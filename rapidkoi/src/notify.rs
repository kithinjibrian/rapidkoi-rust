//! Notifications (contract 11 addition, Rapidkoi's PRP-065): an app that
//! declares `notifications` sends the person a sentence and a place in
//! itself, while it runs. Rapidkoi asks the person the first time, keeps
//! them, and draws them with the app's own name and icon: an app gives
//! values, never a look. The new tab reads them with the built-in
//! `read_notifications`.

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::{Decoder, Encoder};

/// A value in a notification's place: a link field's text, number or true/false.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Number(i64),
    Real(f64),
    Text(String),
    Bool(bool),
}

macro_rules! numbers {
    ($($t:ty),*) => { $( impl From<$t> for FieldValue { fn from(n: $t) -> Self { FieldValue::Number(n as i64) } } )* };
}
numbers!(i8, i16, i32, i64, u8, u16, u32);

impl From<f64> for FieldValue {
    fn from(n: f64) -> Self {
        FieldValue::Real(n)
    }
}

impl From<&str> for FieldValue {
    fn from(s: &str) -> Self {
        FieldValue::Text(s.into())
    }
}

impl From<String> for FieldValue {
    fn from(s: String) -> Self {
        FieldValue::Text(s)
    }
}

impl From<bool> for FieldValue {
    fn from(b: bool) -> Self {
        FieldValue::Bool(b)
    }
}

/// A notification to send: build it, then [`crate::Ask::notify`] it.
///
/// ```ignore
/// Ask::notify(Notify::new("sent", "Your count reached 10").field("count", 10).key("reached-10"))
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Notify {
    pub(crate) msg: String,
    pub(crate) text: String,
    pub(crate) fields: Vec<(String, FieldValue)>,
    pub(crate) key: Option<String>,
}

impl Notify {
    /// A notification of `text`, 1–200 characters, that opens your app as
    /// it starts. Not answered: `msg` is only the ask's name.
    pub fn new(msg: &str, text: &str) -> Notify {
        Notify { msg: msg.into(), text: text.into(), fields: Vec::new(), key: None }
    }

    /// Opening it opens your app with this link field set, as a link would
    /// (at most 16).
    pub fn field(mut self, name: &str, value: impl Into<FieldValue>) -> Notify {
        self.fields.push((name.into(), value.into()));
        self
    }

    /// Its key, 1–64 of `a–z`, `0–9`, `_` and `-`: sending again with it
    /// replaces this one, and [`crate::Ask::withdraw`] takes it back.
    pub fn key(mut self, key: &str) -> Notify {
        self.key = Some(key.into());
        self
    }

    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let n = 3 + u64::from(!self.fields.is_empty()) + u64::from(self.key.is_some());
        let _ = e.map(n).and_then(|e| e.str("kind")).and_then(|e| e.str("notify"));
        let _ = e.str("msg").and_then(|e| e.str(&self.msg));
        let _ = e.str("text").and_then(|e| e.str(&self.text));
        if !self.fields.is_empty() {
            let _ = e.str("fields").and_then(|e| e.map(self.fields.len() as u64));
            for (name, v) in &self.fields {
                let _ = e.str(name);
                let _ = match v {
                    FieldValue::Number(n) => e.i64(*n),
                    FieldValue::Real(n) => e.f64(*n),
                    FieldValue::Text(s) => e.str(s),
                    FieldValue::Bool(b) => e.bool(*b),
                };
            }
        }
        if let Some(k) = &self.key {
            let _ = e.str("key").and_then(|e| e.str(k));
        }
    }
}

/// One of the person's notifications, as Rapidkoi answers [`crate::Ask::notifications`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Who it's from: `"app"`, an app's own. Later kinds come from Rapidkoi.
    pub kind: String,
    /// Its app's name.
    pub name: String,
    /// Where its app lives: a site's host, or a local app's file name.
    pub place: String,
    /// Its app's icon, as a source to show: `image(&n.icon)`.
    pub icon: String,
    /// The sentence.
    pub text: String,
    /// Its place, as the address box shows it.
    pub address: String,
    /// When it was sent, in milliseconds since 1970.
    pub sent: i64,
    /// Whether the person opened it.
    pub read: bool,
}

/// The person's notifications: unread first, then newest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Notifications {
    /// How many are unread.
    pub unread: u32,
    pub items: Vec<Notification>,
}

impl Notifications {
    /// Reads a notifications answer; `None` if it isn't one.
    pub fn read(cbor: &[u8]) -> Option<Notifications> {
        let mut d = Decoder::new(cbor);
        let n = d.map().ok()??;
        let mut out = Notifications::default();
        for _ in 0..n {
            match d.str().ok()? {
                "unread" => out.unread = d.u32().ok()?,
                "items" => {
                    let k = d.array().ok()??;
                    for _ in 0..k {
                        out.items.push(read_one(&mut d)?);
                    }
                }
                _ => d.skip().ok()?,
            }
        }
        Some(out)
    }
}

fn read_one(d: &mut Decoder) -> Option<Notification> {
    let n = d.map().ok()??;
    let (mut kind, mut name, mut place, mut icon) = (None, None, None, None);
    let (mut text, mut address, mut sent, mut read) = (None, None, None, None);
    for _ in 0..n {
        match d.str().ok()? {
            "kind" => kind = Some(String::from(d.str().ok()?)),
            "name" => name = Some(String::from(d.str().ok()?)),
            "place" => place = Some(String::from(d.str().ok()?)),
            "icon" => icon = Some(String::from(d.str().ok()?)),
            "text" => text = Some(String::from(d.str().ok()?)),
            "address" => address = Some(String::from(d.str().ok()?)),
            "sent" => sent = Some(d.i64().ok()?),
            "read" => read = Some(d.bool().ok()?),
            _ => d.skip().ok()?,
        }
    }
    Some(Notification {
        kind: kind?,
        name: name?,
        place: place?,
        icon: icon?,
        text: text?,
        address: address?,
        sent: sent?,
        read: read?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn a_notify_writes_its_text_fields_and_key() {
        let mut e = Encoder::new(Vec::new());
        Notify::new("sent", "Hi").field("count", 10).field("on", true).key("k").write(&mut e);
        let bytes = e.into_writer();
        let mut d = Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(5));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "notify"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", "sent"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("text", "Hi"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("fields", Some(2)));
        assert_eq!((d.str().unwrap(), d.i64().unwrap()), ("count", 10));
        assert_eq!((d.str().unwrap(), d.bool().unwrap()), ("on", true));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("key", "k"));
        // Without fields or a key, neither is written.
        let mut plain = Encoder::new(Vec::new());
        Notify::new("sent", "Hi").write(&mut plain);
        assert_eq!(Decoder::new(&plain.into_writer()).map().unwrap(), Some(3));
    }

    #[test]
    fn the_notifications_answer_reads() {
        let mut e = Encoder::new(Vec::new());
        let _ = e.map(2).and_then(|e| e.str("unread")).and_then(|e| e.u32(1));
        let _ = e.str("items").and_then(|e| e.array(1)).and_then(|e| e.map(8));
        for (k, v) in [("kind", "app"), ("name", "Counter"), ("place", "counter.rkb"), ("icon", "icon:counter.rkb")] {
            let _ = e.str(k).and_then(|e| e.str(v));
        }
        let _ = e.str("text").and_then(|e| e.str("Hi")).and_then(|e| e.str("address")).and_then(|e| e.str("counter.rkb"));
        let _ = e.str("sent").and_then(|e| e.i64(5)).and_then(|e| e.str("read")).and_then(|e| e.bool(false));
        let got = Notifications::read(&e.into_writer()).unwrap();
        assert_eq!(
            got,
            Notifications {
                unread: 1,
                items: vec![Notification {
                    kind: "app".into(),
                    name: "Counter".into(),
                    place: "counter.rkb".into(),
                    icon: "icon:counter.rkb".into(),
                    text: "Hi".into(),
                    address: "counter.rkb".into(),
                    sent: 5,
                    read: false,
                }],
            }
        );
        assert_eq!(Notifications::read(&[0x01]), None);
    }
}
