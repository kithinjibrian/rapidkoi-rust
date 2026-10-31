//! Single files (contract 9, DECISION-089): the person hands the app a file
//! in Rapidkoi's own dialog, and the app saves back to it or saves a new one.
//!
//! - **Choosing is the permission.** Every file comes from Rapidkoi's chooser
//!   ("Open a file for {app}", "… to modify", "Save a file from {app}"),
//!   never drawn by the app. The app can't name a file, create, rename or
//!   delete one: the worst it can do is damage the one file it was handed.
//! - **Names, never paths.** The app learns a file's name (its last part,
//!   `receipt.pdf`) and size, nothing else: no folder, dates or owner.
//! - **The bytes go through blobs.** [`crate::Ask::file_open`] stores the
//!   file as a blob under the app's key, with no owner (give it one with
//!   [`crate::Step::blob_own`], or [`crate::Ask::attach_blob`]); read it with
//!   [`crate::Ask::blob_get`] after the answer. To save, store the bytes
//!   with [`crate::Step::blob_set`] first, and ask [`crate::Ask::file_save`]
//!   after that batch's answer.
//! - **Handles** (from opening with `.edit()`, or saving as) are opaque
//!   text, valid only in the window that got them, until it closes or
//!   reloads: keep them in the window's state, never in a link (a link's
//!   window is another window), and expect "This file is no longer open here".
//! - **Saving never overwrites another program's change.** If the file
//!   changed on disk since it was opened (or last saved), `file_save`
//!   fails with "This file changed since it was opened" and nothing is
//!   written: offer to open it again, or to save as. A save is safe: a
//!   crash leaves the old file or the new one.
//! - **Cancelling isn't an error:** it's [`crate::Event::FileCancelled`].
//! - Other failures, as [`crate::Event::Failed`]'s `error`: "Rapidkoi
//!   couldn't read this file", "Rapidkoi couldn't save this file", "This
//!   app has no blob with that key" (saving a key that holds nothing).
//! - A file is at most 16 MB either way ("This file is over 16 MB"), and
//!   counts toward the app's blobs (1 GB). One dialog at a time per window:
//!   "A file dialog is already open". The new tab and built-in apps keep no
//!   files: "This app keeps no files here".
//!
//! Which answers were file asks' is kept in the window's state (a field
//! named `rapidkoi_files`) until they arrive, so use a `msg` of their own.
//!
//! ```
//! use rapidkoi::{params, Ask, Event, Next, Step};
//!
//! // Open a picture as blob "photo-17", then attach it to note 17.
//! let open = Ask::file_open("attached", "photo-17").accept(["png", "jpg", "jpeg"]);
//! let event: Event = Event::FileOpened { msg: "attached".into(), key: "photo-17".into(),
//!     name: "beach.jpg".into(), size: 48_213, handle: None };
//! if let Event::FileOpened { key, .. } = event {
//!     let attach = Ask::attach_blob("saved", "UPDATE notes SET photo = ? WHERE id = ?",
//!         params![key.as_str(), 17], "notes", 17, &key);
//! }
//!
//! // Open to modify, save back, save as.
//! let open = Ask::file_open("opened", "doc").edit();
//! let store = Ask::sql_batch_steps("stored", vec![Step::blob_set("doc", b"new text".to_vec())]);
//! let save = Ask::file_save("saved", "the handle from FileOpened", "doc");
//! let export = Ask::file_save_as("exported", "doc").name("notes.txt");
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::{Decoder, Encoder};

use crate::blobs::{field_of, key_ok};
use crate::Event;

/// The most extensions [`FileOpen::accept`] and [`FileSaveAs::accept`] take.
pub const MAX_ACCEPT: usize = 16;

/// A [`crate::Ask::file_open`]: which key the file goes under, and how it's chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOpen {
    pub msg: String,
    pub key: String,
    pub accept: Option<Vec<String>>,
    pub edit: bool,
}

impl FileOpen {
    /// Shows only files with these extensions, without the dot (`["png",
    /// "jpg"]`, at most 16). Only a filter: never trust a file's type by it.
    pub fn accept<S: AsRef<str>>(mut self, exts: impl IntoIterator<Item = S>) -> FileOpen {
        self.accept = Some(extensions(exts));
        self
    }

    /// Opens it to modify ("Open a file for {app} to modify"): the answer
    /// carries a handle for [`crate::Ask::file_save`].
    pub fn edit(mut self) -> FileOpen {
        self.edit = true;
        self
    }

    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let n = 3 + self.accept.is_some() as u64 + self.edit as u64;
        let _ = e.map(n).and_then(|e| e.str("kind")).and_then(|e| e.str("file_open"));
        let _ = e.str("msg").and_then(|e| e.str(&self.msg));
        let _ = e.str("key").and_then(|e| e.str(&self.key));
        if let Some(a) = &self.accept {
            write_accept(e, a);
        }
        if self.edit {
            let _ = e.str("edit").and_then(|e| e.bool(true));
        }
    }
}

/// A [`crate::Ask::file_save`]: a blob's bytes over the file a handle names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSave {
    pub msg: String,
    pub handle: String,
    pub key: String,
}

impl FileSave {
    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let _ = e.map(4).and_then(|e| e.str("kind")).and_then(|e| e.str("file_save"));
        let _ = e.str("msg").and_then(|e| e.str(&self.msg));
        let _ = e.str("handle").and_then(|e| e.str(&self.handle));
        let _ = e.str("key").and_then(|e| e.str(&self.key));
    }
}

/// A [`crate::Ask::file_save_as`]: a blob's bytes to a file the person chooses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSaveAs {
    pub msg: String,
    pub key: String,
    pub name: Option<String>,
    pub accept: Option<Vec<String>>,
}

impl FileSaveAs {
    /// The name the dialog suggests (`notes.txt`).
    pub fn name(mut self, name: &str) -> FileSaveAs {
        self.name = Some(String::from(name));
        self
    }

    /// The extensions the dialog offers, without the dot (at most 16).
    pub fn accept<S: AsRef<str>>(mut self, exts: impl IntoIterator<Item = S>) -> FileSaveAs {
        self.accept = Some(extensions(exts));
        self
    }

    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let n = 3 + self.name.is_some() as u64 + self.accept.is_some() as u64;
        let _ = e.map(n).and_then(|e| e.str("kind")).and_then(|e| e.str("file_save_as"));
        let _ = e.str("msg").and_then(|e| e.str(&self.msg));
        let _ = e.str("key").and_then(|e| e.str(&self.key));
        if let Some(name) = &self.name {
            let _ = e.str("name").and_then(|e| e.str(name));
        }
        if let Some(a) = &self.accept {
            write_accept(e, a);
        }
    }
}

pub(crate) fn file_open(msg: &str, key: &str) -> FileOpen {
    debug_assert!(key_ok(key), "a blob's key is 1 to 256 bytes, no control characters");
    FileOpen { msg: msg.into(), key: key.into(), accept: None, edit: false }
}

pub(crate) fn file_save(msg: &str, handle: &str, key: &str) -> FileSave {
    debug_assert!(key_ok(key), "a blob's key is 1 to 256 bytes, no control characters");
    FileSave { msg: msg.into(), handle: handle.into(), key: key.into() }
}

pub(crate) fn file_save_as(msg: &str, key: &str) -> FileSaveAs {
    debug_assert!(key_ok(key), "a blob's key is 1 to 256 bytes, no control characters");
    FileSaveAs { msg: msg.into(), key: key.into(), name: None, accept: None }
}

fn extensions<S: AsRef<str>>(exts: impl IntoIterator<Item = S>) -> Vec<String> {
    let out: Vec<String> = exts.into_iter().map(|s| String::from(s.as_ref())).collect();
    // More stops the app, as any malformed ask does.
    debug_assert!(out.len() <= MAX_ACCEPT, "at most 16 extensions");
    out
}

fn write_accept(e: &mut Encoder<Vec<u8>>, exts: &[String]) {
    let _ = e.str("accept").and_then(|e| e.array(exts.len() as u64));
    for x in exts {
        let _ = e.str(x);
    }
}

/// The state's field where file asks wait for their answers.
pub(crate) const PENDING: &str = "rapidkoi_files";

/// The most file asks kept waiting: an answer lost to a reload never
/// arrives, and the oldest go first.
const MAX_PENDING: usize = 32;

/// Which file ask a `msg` was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Open,
    Save,
    SaveAs,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Open => "open",
            Kind::Save => "save",
            Kind::SaveAs => "save_as",
        }
    }

    fn from_name(s: &str) -> Option<Kind> {
        match s {
            "open" => Some(Kind::Open),
            "save" => Some(Kind::Save),
            "save_as" => Some(Kind::SaveAs),
            _ => None,
        }
    }
}

/// A file ask sent and not yet answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pending {
    pub msg: String,
    pub kind: Kind,
}

/// Adds an ask the app just sent.
pub(crate) fn wait(pending: &mut Vec<Pending>, msg: &str, kind: Kind) {
    pending.push(Pending { msg: msg.into(), kind });
    if pending.len() > MAX_PENDING {
        pending.remove(0);
    }
}

/// The file asks kept in a state's bytes; none when the field is missing or unreadable.
pub(crate) fn pending_in(state: &[u8]) -> Vec<Pending> {
    let read = |d: &mut Decoder| -> Option<Vec<Pending>> {
        let k = d.array().ok()??;
        (0..k)
            .map(|_| {
                if d.array().ok()? != Some(2) {
                    return None;
                }
                let msg = String::from(d.str().ok()?);
                Some(Pending { msg, kind: Kind::from_name(d.str().ok()?)? })
            })
            .collect()
    };
    field_of(state, PENDING).and_then(|mut d| read(&mut d)).unwrap_or_default()
}

/// The pending asks as the value of their field: none is no field.
pub(crate) fn pending_field(pending: &[Pending]) -> Option<(&'static str, Vec<u8>)> {
    if pending.is_empty() {
        return None;
    }
    let mut e = Encoder::new(Vec::new());
    let _ = e.array(pending.len() as u64);
    for p in pending {
        let _ = e.array(2).and_then(|e| e.str(&p.msg)).and_then(|e| e.str(p.kind.name()));
    }
    Some((PENDING, e.into_writer()))
}

/// An answer to a file ask becomes its event, and the ask stops waiting:
/// `null` is [`Event::FileCancelled`], a failure stays [`Event::Failed`].
/// Answers to other asks pass through as they are.
pub(crate) fn answer_of<M>(e: Event<M>, pending: &mut Vec<Pending>) -> Event<M> {
    let msg = match &e {
        Event::Answer { msg, value: None } | Event::Data { msg, .. } | Event::Failed { msg, .. } => msg,
        _ => return e,
    };
    let Some(i) = pending.iter().position(|p| p.msg == *msg) else {
        return e;
    };
    let kind = pending.remove(i).kind;
    match e {
        Event::Answer { msg, .. } => Event::FileCancelled { msg },
        Event::Data { msg, cbor } => match decode(kind, &msg, &cbor) {
            Some(file) => file,
            None => Event::Data { msg, cbor },
        },
        e => e,
    }
}

/// `{key, name, size, handle?}`, `{size}` or `{name, size, handle}`, by the ask's kind.
fn decode<M>(kind: Kind, msg: &str, cbor: &[u8]) -> Option<Event<M>> {
    let mut d = Decoder::new(cbor);
    let n = d.map().ok()??;
    let (mut key, mut name, mut size, mut handle) = (None, None, None, None);
    for _ in 0..n {
        match d.str().ok()? {
            "key" => key = Some(String::from(d.str().ok()?)),
            "name" => name = Some(String::from(d.str().ok()?)),
            "size" => size = Some(d.u64().ok()?),
            "handle" => handle = Some(String::from(d.str().ok()?)),
            _ => d.skip().ok()?,
        }
    }
    let msg = String::from(msg);
    Some(match kind {
        Kind::Open => Event::FileOpened { msg, key: key?, name: name?, size: size?, handle },
        Kind::Save => Event::FileSaved { msg, size: size? },
        Kind::SaveAs => Event::FileSavedAs { msg, name: name?, size: size?, handle: handle? },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ask, Step};

    fn cbor(f: impl FnOnce(&mut Encoder<Vec<u8>>)) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        f(&mut e);
        e.into_writer()
    }

    fn encoded(a: &Ask) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        match a {
            Ask::FileOpen(o) => o.write(&mut e),
            Ask::FileSave(s) => s.write(&mut e),
            Ask::FileSaveAs(s) => s.write(&mut e),
            other => panic!("{other:?}"),
        }
        e.into_writer()
    }

    #[test]
    fn file_open_encodes_accept_and_edit_only_when_given() {
        assert_eq!(
            encoded(&Ask::file_open("opened", "doc").into()),
            cbor(|e| {
                e.map(3).unwrap().str("kind").unwrap().str("file_open").unwrap();
                e.str("msg").unwrap().str("opened").unwrap().str("key").unwrap().str("doc").unwrap();
            })
        );
        assert_eq!(
            encoded(&Ask::file_open("attached", "photo-3").accept(["png", "jpg"]).edit().into()),
            cbor(|e| {
                e.map(5).unwrap().str("kind").unwrap().str("file_open").unwrap();
                e.str("msg").unwrap().str("attached").unwrap().str("key").unwrap().str("photo-3").unwrap();
                e.str("accept").unwrap().array(2).unwrap().str("png").unwrap().str("jpg").unwrap();
                e.str("edit").unwrap().bool(true).unwrap();
            })
        );
    }

    #[test]
    fn file_save_and_save_as_encode() {
        assert_eq!(
            encoded(&Ask::file_save("saved", "h1", "doc")),
            cbor(|e| {
                e.map(4).unwrap().str("kind").unwrap().str("file_save").unwrap().str("msg").unwrap().str("saved").unwrap();
                e.str("handle").unwrap().str("h1").unwrap().str("key").unwrap().str("doc").unwrap();
            })
        );
        assert_eq!(
            encoded(&Ask::file_save_as("exported", "doc").into()),
            cbor(|e| {
                e.map(3).unwrap().str("kind").unwrap().str("file_save_as").unwrap();
                e.str("msg").unwrap().str("exported").unwrap().str("key").unwrap().str("doc").unwrap();
            })
        );
        assert_eq!(
            encoded(&Ask::file_save_as("exported", "doc").name("notes.txt").accept(["txt"]).into()),
            cbor(|e| {
                e.map(5).unwrap().str("kind").unwrap().str("file_save_as").unwrap();
                e.str("msg").unwrap().str("exported").unwrap().str("key").unwrap().str("doc").unwrap();
                e.str("name").unwrap().str("notes.txt").unwrap();
                e.str("accept").unwrap().array(1).unwrap().str("txt").unwrap();
            })
        );
    }

    #[test]
    fn blob_own_encodes_with_its_owner() {
        let mut e = Encoder::new(Vec::new());
        Step::blob_own("photo-3", "notes", 3).write(&mut e);
        assert_eq!(
            e.into_writer(),
            cbor(|e| {
                e.map(2).unwrap().str("blob_own").unwrap().str("photo-3").unwrap();
                e.str("owner").unwrap().map(2).unwrap().str("table").unwrap().str("notes").unwrap();
                e.str("key").unwrap().i64(3).unwrap();
            })
        );
        // `.owner` changes a blob_own's owner too.
        assert_eq!(Step::blob_own("k", "a", 1).owner("b", 2), Step::blob_own("k", "b", 2));
    }

    #[test]
    fn attach_blob_owns_then_writes_the_row() {
        assert_eq!(
            Ask::attach_blob("saved", "UPDATE notes SET photo = ? WHERE id = ?", crate::params!["photo-3", 3], "notes", 3, "photo-3"),
            Ask::sql_batch_steps("saved", alloc::vec![
                Step::blob_own("photo-3", "notes", 3),
                Step::sql("UPDATE notes SET photo = ? WHERE id = ?", crate::params!["photo-3", 3]),
            ])
        );
    }

    #[test]
    fn pending_asks_ride_in_the_state() {
        let mut p = Vec::new();
        wait(&mut p, "opened", Kind::Open);
        wait(&mut p, "saved", Kind::Save);
        let (name, value) = pending_field(&p).unwrap();
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("n").unwrap().i64(3).unwrap();
        let state = crate::blobs::with_fields(e.into_writer(), &[(name, value)]);
        assert_eq!(pending_in(&state), p);
        assert_eq!(pending_in(&[0xa0]), alloc::vec![]);
        assert!(pending_field(&[]).is_none());
        for i in 0..40 {
            wait(&mut p, &alloc::format!("m{i}"), Kind::Save);
        }
        assert_eq!((p.len(), p[0].msg.as_str()), (MAX_PENDING, "m8"));
    }
}
