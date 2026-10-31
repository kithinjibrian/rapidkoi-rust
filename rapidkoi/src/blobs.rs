//! Blobs (contract 9, PRP-054): an app's large bytes, kept beside its
//! database under keys it names. Rapidkoi stores them by their hash, never
//! looks inside them, and never gives them to another app.
//!
//! Storing and removing are steps of a batch, committed with the rows:
//!
//! ```
//! use rapidkoi::{params, Ask, Step};
//!
//! let id = 17;
//! let photo: Vec<u8> = vec![0x89, b'P', b'N', b'G'];
//! let save = Ask::sql_batch_steps("saved", vec![
//!     Step::blob_set("photo-17", photo).owner("notes", id),
//!     Step::sql("INSERT INTO notes (id, photo) VALUES (?, ?)", params![id, "photo-17"]),
//! ]);
//! let gone = Ask::sql_batch_steps("removed", vec![Step::blob_delete("photo-17")]);
//! ```
//!
//! Reading: [`crate::Ask::blob_get`] answers [`crate::Event::Blob`], at most
//! 4 MB a piece; [`crate::Ask::blob_list`] answers [`crate::Event::Blobs`];
//! [`crate::Ask::blob_lines`] reads a blob of any size line by line as
//! [`crate::Event::Lines`].
//!
//! - **Read after the write's answer.** An app's asks run side by side, so
//!   a read sent with the batch that writes the blob can run first.
//! - **Handle pieces as they arrive** and keep the result, not the bytes:
//!   what an app holds lives in its window's state (16 MB).
//! - **Owners:** with [`Step::owner`] a blob belongs to a row (its table
//!   must exist before the batch, with a primary key of one column).
//!   Deleting the row, or dropping the table, deletes the blob in the same
//!   write. **Changing the row's primary key drops its blobs too.**
//! - **Never name anything `rapidkoi_…`**: those tables, indexes, triggers
//!   and views are Rapidkoi's, and your SQL is refused them.
//! - Limits: a key 1–256 bytes, no control characters; a blob 16 MB; an
//!   app's blobs 1 GB and 10,000. A batch over one fails whole.

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::{data::Type, Decoder, Encoder};

use crate::storage::Value;

/// Whether `key` can name a blob: 1 to 256 bytes, no control characters.
pub(crate) fn key_ok(key: &str) -> bool {
    (1..=256).contains(&key.len()) && !key.bytes().any(|b| b < 32 || b == 127)
}

/// One step of [`crate::Ask::sql_batch_steps`]: a statement, or storing or
/// removing a blob. All of a batch's steps commit together, or none do.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// A statement and its parameters.
    Sql(String, Vec<Value>),
    /// Store or replace a key's bytes, owned by a row (table, primary key) or by no one.
    BlobSet { key: String, bytes: Vec<u8>, owner: Option<(String, Value)> },
    /// Remove a key; no such key is nothing to do.
    BlobDelete(String),
}

impl Step {
    /// A statement: values go in `params` (`params![…]`), one per `?`.
    pub fn sql(sql: &str, params: Vec<Value>) -> Step {
        Step::Sql(String::from(sql), params)
    }

    /// Stores `bytes` (at most 16 MB) under `key`, replacing what it held.
    pub fn blob_set(key: &str, bytes: impl Into<Vec<u8>>) -> Step {
        Step::BlobSet { key: String::from(key), bytes: bytes.into(), owner: None }
    }

    /// A [`Step::blob_set`]'s owner: the row of `table` whose primary key
    /// is `key`. Deleting that row deletes the blob. Nothing on other steps.
    pub fn owner(self, table: &str, key: impl Into<Value>) -> Step {
        match self {
            Step::BlobSet { key: k, bytes, .. } => Step::BlobSet { key: k, bytes, owner: Some((String::from(table), key.into())) },
            other => other,
        }
    }

    /// Removes `key` and its bytes.
    pub fn blob_delete(key: &str) -> Step {
        Step::BlobDelete(String::from(key))
    }

    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        match self {
            Step::Sql(sql, params) => {
                let _ = e.map(2).and_then(|e| e.str("sql")).and_then(|e| e.str(sql));
                let _ = e.str("params").and_then(|e| e.array(params.len() as u64));
                for p in params {
                    p.write(e);
                }
            }
            Step::BlobSet { key, bytes, owner } => {
                let _ = e.map(2 + owner.is_some() as u64).and_then(|e| e.str("blob_set")).and_then(|e| e.str(key));
                let _ = e.str("bytes").and_then(|e| e.bytes(bytes));
                if let Some((table, k)) = owner {
                    let _ = e.str("owner").and_then(|e| e.map(2)).and_then(|e| e.str("table")).and_then(|e| e.str(table));
                    let _ = e.str("key");
                    k.write(e);
                }
            }
            Step::BlobDelete(key) => {
                let _ = e.map(1).and_then(|e| e.str("blob_delete")).and_then(|e| e.str(key));
            }
        }
    }
}

impl From<(String, Vec<Value>)> for Step {
    fn from((sql, params): (String, Vec<Value>)) -> Step {
        Step::Sql(sql, params)
    }
}

/// A [`crate::Ask::blob_get`]: which key, and optionally where to start,
/// how much, and the hash an earlier piece gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobGet {
    pub msg: String,
    pub key: String,
    pub from: Option<u64>,
    pub len: Option<u64>,
    pub hash: Option<Vec<u8>>,
}

impl BlobGet {
    /// Starts at byte `n` (default 0).
    pub fn from(mut self, n: u64) -> BlobGet {
        self.from = Some(n);
        self
    }

    /// At most `n` bytes (default, and at most, 4 MB).
    pub fn len(mut self, n: u64) -> BlobGet {
        self.len = Some(n);
        self
    }

    /// The hash the first piece's answer gave: if the blob changed since,
    /// the answer is [`crate::Event::Failed`] and the reading starts over.
    pub fn hash(mut self, h: &[u8]) -> BlobGet {
        self.hash = Some(h.to_vec());
        self
    }

    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let n = 3 + self.from.is_some() as u64 + self.len.is_some() as u64 + self.hash.is_some() as u64;
        let _ = e.map(n).and_then(|e| e.str("kind")).and_then(|e| e.str("blob_get"));
        let _ = e.str("msg").and_then(|e| e.str(&self.msg));
        let _ = e.str("key").and_then(|e| e.str(&self.key));
        if let Some(n) = self.from {
            let _ = e.str("from").and_then(|e| e.u64(n));
        }
        if let Some(n) = self.len {
            let _ = e.str("len").and_then(|e| e.u64(n));
        }
        if let Some(h) = &self.hash {
            let _ = e.str("hash").and_then(|e| e.bytes(h));
        }
    }
}

/// A [`crate::Ask::blob_list`]: the app's keys, or those starting with a prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobList {
    pub msg: String,
    pub prefix: Option<String>,
}

impl BlobList {
    /// Only the keys starting with `p`.
    pub fn prefix(mut self, p: &str) -> BlobList {
        self.prefix = Some(String::from(p));
        self
    }

    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let _ = e.map(2 + self.prefix.is_some() as u64).and_then(|e| e.str("kind")).and_then(|e| e.str("blob_list"));
        let _ = e.str("msg").and_then(|e| e.str(&self.msg));
        if let Some(p) = &self.prefix {
            let _ = e.str("prefix").and_then(|e| e.str(p));
        }
    }
}

/// One of the app's blobs, as [`crate::Event::Blobs`] lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct BlobInfo {
    pub key: String,
    /// Its size in bytes.
    pub size: u64,
    /// The row that owns it: its table and primary key.
    pub owner: Option<(String, Value)>,
}

/// A blob answer: a piece, or a list.
pub(crate) enum BlobAnswer {
    Piece { bytes: Vec<u8>, from: u64, size: u64, hash: Vec<u8> },
    List { blobs: Vec<BlobInfo>, cut: u64 },
}

/// Reads `{bytes, from, size, hash}` or `{blobs, cut}`; anything else is `None`.
pub(crate) fn blob_answer(cbor: &[u8]) -> Option<BlobAnswer> {
    let mut d = Decoder::new(cbor);
    let n = d.map().ok()??;
    let (mut bytes, mut from, mut size, mut hash, mut blobs, mut cut) = (None, 0, None, None, None, 0);
    for _ in 0..n {
        match d.str().ok()? {
            "bytes" => bytes = Some(d.bytes().ok()?.to_vec()),
            "from" => from = d.u64().ok()?,
            "size" => size = Some(d.u64().ok()?),
            "hash" => hash = Some(d.bytes().ok()?.to_vec()),
            "blobs" => {
                let k = d.array().ok()??;
                let mut out = Vec::new();
                for _ in 0..k {
                    out.push(info(&mut d)?);
                }
                blobs = Some(out);
            }
            // A list's `cut` is a count (a rows answer's is true/false: not this one).
            "cut" if d.datatype().ok()? != Type::Bool => cut = d.u64().ok()?,
            _ => d.skip().ok()?,
        }
    }
    match (bytes, size, hash, blobs) {
        (Some(bytes), Some(size), Some(hash), _) => Some(BlobAnswer::Piece { bytes, from, size, hash }),
        (_, _, _, Some(blobs)) => Some(BlobAnswer::List { blobs, cut }),
        _ => None,
    }
}

fn info(d: &mut Decoder) -> Option<BlobInfo> {
    let n = d.map().ok()??;
    let (mut key, mut size, mut owner) = (None, 0, None);
    for _ in 0..n {
        match d.str().ok()? {
            "key" => key = Some(String::from(d.str().ok()?)),
            "size" => size = d.u64().ok()?,
            "owner" => {
                let m = d.map().ok()??;
                let (mut table, mut k) = (None, None);
                for _ in 0..m {
                    match d.str().ok()? {
                        "table" => table = Some(String::from(d.str().ok()?)),
                        "key" => k = Some(Value::read(d)?),
                        _ => d.skip().ok()?,
                    }
                }
                owner = Some((table?, k?));
            }
            _ => d.skip().ok()?,
        }
    }
    Some(BlobInfo { key: key?, size, owner })
}

/// The state's field where the line readers wait between pieces: the app
/// keeps nothing between calls, so their place rides in the window's state.
pub(crate) const READERS: &str = "rapidkoi_lines";

/// A [`crate::Ask::blob_lines`] between pieces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reader {
    pub msg: String,
    pub key: String,
    /// Where the next piece starts.
    pub from: u64,
    /// The first piece's hash, passed back with the rest.
    pub hash: Option<Vec<u8>>,
    /// An unfinished last line, waiting for the rest of it.
    pub carry: Vec<u8>,
}

impl Reader {
    pub(crate) fn start(msg: &str, key: &str) -> Reader {
        Reader { msg: String::from(msg), key: String::from(key), from: 0, hash: None, carry: Vec::new() }
    }

    /// The ask for its next piece.
    pub(crate) fn next(&self) -> BlobGet {
        BlobGet { msg: self.msg.clone(), key: self.key.clone(), from: Some(self.from), len: None, hash: self.hash.clone() }
    }

    /// Takes a piece: the lines it finishes, and whether that was the last.
    /// A line ends at `\n` (a `\r` before it is dropped); bytes that aren't
    /// UTF-8 become U+FFFD; the end of the blob finishes its last line.
    pub(crate) fn take(&mut self, bytes: &[u8], size: u64, hash: Vec<u8>) -> (Vec<String>, bool) {
        self.hash = Some(hash);
        self.from += bytes.len() as u64;
        self.carry.extend_from_slice(bytes);
        // An empty piece before the end would ask for the same piece forever.
        let done = self.from >= size || bytes.is_empty();
        let mut lines = Vec::new();
        let mut start = 0;
        for (i, b) in self.carry.iter().enumerate() {
            if *b == b'\n' {
                lines.push(line(&self.carry[start..i]));
                start = i + 1;
            }
        }
        self.carry.drain(..start);
        if done && !self.carry.is_empty() {
            lines.push(line(&self.carry));
            self.carry.clear();
        }
        (lines, done)
    }

    fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let _ = e.array(5).and_then(|e| e.str(&self.msg)).and_then(|e| e.str(&self.key)).and_then(|e| e.u64(self.from));
        let _ = match &self.hash {
            Some(h) => e.bytes(h),
            None => e.null(),
        };
        let _ = e.bytes(&self.carry);
    }

    fn read(d: &mut Decoder) -> Option<Reader> {
        if d.array().ok()? != Some(5) {
            return None;
        }
        let msg = String::from(d.str().ok()?);
        let key = String::from(d.str().ok()?);
        let from = d.u64().ok()?;
        let hash = if d.datatype().ok()? == Type::Null {
            d.null().ok()?;
            None
        } else {
            Some(d.bytes().ok()?.to_vec())
        };
        Some(Reader { msg, key, from, hash, carry: d.bytes().ok()?.to_vec() })
    }
}

fn line(b: &[u8]) -> String {
    let b = b.strip_suffix(b"\r").unwrap_or(b);
    String::from_utf8_lossy(b).into_owned()
}

/// The readers kept in a state's bytes; none when there are none, or the
/// field can't be read.
pub(crate) fn readers_in(state: &[u8]) -> Vec<Reader> {
    let mut d = Decoder::new(state);
    let read = |d: &mut Decoder| -> Option<Vec<Reader>> {
        let n = d.map().ok()??;
        for _ in 0..n {
            if d.str().ok()? == READERS {
                let k = d.array().ok()??;
                return (0..k).map(|_| Reader::read(d)).collect();
            }
            d.skip().ok()?;
        }
        Some(Vec::new())
    };
    read(&mut d).unwrap_or_default()
}

/// The app's encoded state with the readers added as one more field.
pub(crate) fn with_readers(state: Vec<u8>, readers: &[Reader]) -> Vec<u8> {
    if readers.is_empty() {
        return state;
    }
    let mut d = Decoder::new(&state);
    let n = match d.map() {
        Ok(Some(n)) => n,
        _ => return state,
    };
    let mut e = Encoder::new(Vec::new());
    let _ = e.map(n + 1);
    let mut out = e.into_writer();
    out.extend_from_slice(&state[d.position()..]);
    let mut e = Encoder::new(out);
    let _ = e.str(READERS).and_then(|e| e.array(readers.len() as u64));
    for r in readers {
        r.write(&mut e);
    }
    e.into_writer()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> String {
        String::from(x)
    }

    #[test]
    fn lines_carry_across_a_split_and_finish_at_the_end() {
        let mut r = Reader::start("log", "log.jsonl");
        let h = alloc::vec![7u8; 32];
        // "a\nbc" | "d\r\ne" | "f": 9 bytes, no newline at the end.
        assert_eq!(r.take(b"a\nbc", 9, h.clone()), (alloc::vec![s("a")], false));
        assert_eq!((r.from, r.carry.as_slice()), (4, &b"bc"[..]));
        assert_eq!(r.next().hash, Some(h.clone()));
        assert_eq!(r.next().from, Some(4));
        assert_eq!(r.take(b"d\r\ne", 9, h.clone()), (alloc::vec![s("bcd")], false));
        assert_eq!(r.take(b"f", 9, h.clone()), (alloc::vec![s("ef")], true));
    }

    #[test]
    fn a_trailing_newline_makes_no_empty_line_and_an_empty_blob_none() {
        let mut r = Reader::start("m", "k");
        assert_eq!(r.take(b"x\ny\n", 4, alloc::vec![1]), (alloc::vec![s("x"), s("y")], true));
        let mut r = Reader::start("m", "k");
        assert_eq!(r.take(b"", 0, alloc::vec![1]), (alloc::vec![], true));
    }

    #[test]
    fn a_split_character_is_whole_in_its_line() {
        let mut r = Reader::start("m", "k");
        let word = "café\n".as_bytes();
        assert_eq!(r.take(&word[..4], 6, alloc::vec![1]), (alloc::vec![], false));
        assert_eq!(r.take(&word[4..], 6, alloc::vec![1]), (alloc::vec![s("café")], true));
    }

    #[test]
    fn readers_ride_in_the_state() {
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("n").unwrap().i64(3).unwrap();
        let mut r = Reader::start("log", "k");
        r.take(b"ab", 10, alloc::vec![9; 32]);
        let state = with_readers(e.into_writer(), &[r.clone()]);
        assert_eq!(readers_in(&state), alloc::vec![r]);
        let mut d = Decoder::new(&state);
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.i64().unwrap()), ("n", 3));
        assert_eq!(readers_in(&[0xa0]), alloc::vec![]);
    }
}
