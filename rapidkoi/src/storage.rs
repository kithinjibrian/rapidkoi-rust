//! The app's database (contract 6, PRP-041): every app has its own SQLite
//! database on the device. Ask with [`crate::Ask::sql`] (one statement) or
//! [`crate::Ask::sql_batch`] (several, as one transaction); the answer is an
//! [`crate::Event::Rows`], an [`crate::Event::Done`], or an
//! [`crate::Event::Failed`] saying why. Values always go in as parameters.
//!
//! ```
//! use rapidkoi::{params, Ask};
//!
//! let read = Ask::sql("task", "SELECT title, done FROM tasks WHERE id = ?", params![2]);
//! let write = Ask::sql_batch("saved", vec![
//!     ("UPDATE tasks SET done = ? WHERE id = ?".into(), params![true, 2]),
//! ]);
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::{data::Type, Decoder, Encoder};

/// One SQLite value: a parameter, or a column of a row.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Real(f64),
    Text(String),
    Bytes(Vec<u8>),
    Null,
}

impl Value {
    /// The value as an integer: 0 when it isn't one.
    pub fn int(&self) -> i64 {
        match self {
            Value::Integer(n) => *n,
            _ => 0,
        }
    }

    /// The value as true or false: SQLite keeps them as 1 and 0.
    pub fn bool(&self) -> bool {
        self.int() != 0
    }

    /// The value as text: empty when it isn't text.
    pub fn text(&self) -> String {
        match self {
            Value::Text(s) => s.clone(),
            _ => String::new(),
        }
    }

    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let _ = match self {
            Value::Integer(n) => e.i64(*n),
            Value::Real(x) => e.f64(*x),
            Value::Text(s) => e.str(s),
            Value::Bytes(b) => e.bytes(b),
            Value::Null => e.null(),
        };
    }

    pub(crate) fn read(d: &mut Decoder) -> Option<Value> {
        Some(match d.datatype().ok()? {
            Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::I8 | Type::I16 | Type::I32 | Type::I64 => {
                Value::Integer(d.i64().ok()?)
            }
            Type::F16 | Type::F32 | Type::F64 => Value::Real(d.f64().ok()?),
            Type::String => Value::Text(String::from(d.str().ok()?)),
            Type::Bytes => Value::Bytes(d.bytes().ok()?.to_vec()),
            Type::Null => {
                d.null().ok()?;
                Value::Null
            }
            _ => return None,
        })
    }
}

macro_rules! from_int {
    ($($t:ty),*) => { $( impl From<$t> for Value { fn from(n: $t) -> Value { Value::Integer(n as i64) } } )* };
}

from_int!(i8, i16, i32, i64, u8, u16, u32);

impl From<f64> for Value {
    fn from(x: f64) -> Value {
        Value::Real(x)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Integer(b as i64)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Text(String::from(s))
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Text(s)
    }
}

impl From<Vec<u8>> for Value {
    fn from(b: Vec<u8>) -> Value {
        Value::Bytes(b)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(o: Option<T>) -> Value {
        o.map(Into::into).unwrap_or(Value::Null)
    }
}

/// Values for a statement's `?`s, in order: `params![title, false, 2]`.
#[macro_export]
macro_rules! params {
    ($($v:expr),* $(,)?) => { $crate::__Vec::from([$($crate::Value::from($v)),*]) };
}

/// Reads an answer: `{columns, rows, cut}` or `{changes, last_rowid}`.
pub(crate) enum Answered {
    Rows { columns: Vec<String>, rows: Vec<Vec<Value>>, cut: bool },
    Done { changes: i64, last_rowid: i64 },
}

pub(crate) fn answered(cbor: &[u8]) -> Option<Answered> {
    let mut d = Decoder::new(cbor);
    let n = d.map().ok()??;
    let (mut columns, mut rows, mut cut, mut changes, mut last) = (None, None, false, None, 0);
    for _ in 0..n {
        match d.str().ok()? {
            "columns" => {
                let k = d.array().ok()??;
                let mut out = Vec::new();
                for _ in 0..k {
                    out.push(String::from(d.str().ok()?));
                }
                columns = Some(out);
            }
            "rows" => {
                let k = d.array().ok()??;
                let mut out = Vec::new();
                for _ in 0..k {
                    let w = d.array().ok()??;
                    let mut row = Vec::new();
                    for _ in 0..w {
                        row.push(Value::read(&mut d)?);
                    }
                    out.push(row);
                }
                rows = Some(out);
            }
            "cut" => cut = d.bool().ok()?,
            "changes" => changes = Some(d.i64().ok()?),
            "last_rowid" => last = d.i64().ok()?,
            _ => d.skip().ok()?,
        }
    }
    match (columns, rows, changes) {
        (Some(columns), Some(rows), _) => Some(Answered::Rows { columns, rows, cut }),
        (_, _, Some(changes)) => Some(Answered::Done { changes, last_rowid: last }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ask, Event};

    #[test]
    fn params_make_values() {
        let p = crate::params![2, true, "a", 1.5, Option::<i64>::None, vec![1u8]];
        assert_eq!(
            p,
            vec![Value::Integer(2), Value::Integer(1), Value::Text("a".into()), Value::Real(1.5), Value::Null, Value::Bytes(vec![1])]
        );
    }

    #[test]
    fn rows_and_done_decode() {
        let mut e = Encoder::new(Vec::new());
        e.map(3).unwrap();
        e.str("event").unwrap().str("on_answer").unwrap();
        e.str("msg").unwrap().str("t").unwrap();
        e.str("value").unwrap().map(3).unwrap();
        e.str("columns").unwrap().array(1).unwrap().str("x").unwrap();
        e.str("rows").unwrap().array(1).unwrap().array(1).unwrap().i64(7).unwrap();
        e.str("cut").unwrap().bool(false).unwrap();
        let got = Event::<String>::decode(&e.into_writer());
        assert_eq!(
            got,
            Some(Event::Rows { msg: "t".into(), columns: vec!["x".into()], rows: vec![vec![Value::Integer(7)]], cut: false })
        );
        let mut e = Encoder::new(Vec::new());
        e.map(3).unwrap();
        e.str("event").unwrap().str("on_answer").unwrap();
        e.str("msg").unwrap().str("s").unwrap();
        e.str("value").unwrap().map(2).unwrap();
        e.str("changes").unwrap().i64(1).unwrap();
        e.str("last_rowid").unwrap().i64(9).unwrap();
        assert_eq!(Event::<String>::decode(&e.into_writer()), Some(Event::Done { msg: "s".into(), changes: 1, last_rowid: 9 }));
    }

    #[test]
    fn change_carries_its_tables() {
        let mut e = Encoder::new(Vec::new());
        e.map(2).unwrap();
        e.str("event").unwrap().str("on_change").unwrap();
        e.str("tables").unwrap().array(1).unwrap().str("tasks").unwrap();
        assert_eq!(Event::<String>::decode(&e.into_writer()), Some(Event::Change { tables: vec!["tasks".into()] }));
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("event").unwrap().str("on_change").unwrap();
        assert_eq!(Event::<String>::decode(&e.into_writer()), Some(Event::Change { tables: vec![] }));
    }

    #[test]
    fn asks_encode() {
        assert_eq!(Ask::sql("t", "SELECT ?", crate::params![1]), Ask::Sql("t".into(), "SELECT ?".into(), vec![Value::Integer(1)]));
    }
}
