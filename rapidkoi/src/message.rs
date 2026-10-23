//! Typed messages (PRP-039): what a screen sends when something is pressed,
//! typed, toggled or picked. Derive [`Message`] for an enum of your own,
//! give its values to `on_press` and friends, and `handle` gets them back
//! as `Event::Message { msg, .. }`. A part of an app keeps its own enum,
//! and its view's messages are wrapped with [`crate::View::map`] on the way
//! up. On the wire a message is still a CBOR value: a unit variant is its
//! name (`"Add"`), one with fields is `["Toggle", 3]`.
//!
//! ```
//! use rapidkoi::{button, Message, View};
//!
//! #[derive(Debug, PartialEq, Message)]
//! enum Msg { Add, Toggle(u32) }
//!
//! let add: View<Msg> = button("Add").on_press(Msg::Add);
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::{Decoder, Encoder};

/// A value a screen can send. `#[derive(Message)]` writes it for an enum;
/// `String`, `bool` and the integer types are messages too.
pub trait Message: Sized {
    /// Writes it as CBOR.
    fn write(&self, e: &mut Encoder<Vec<u8>>);
    /// Reads it back; `None` if the CBOR isn't one.
    fn read(d: &mut Decoder) -> Option<Self>;
}

/// Encodes a message to its bytes.
pub(crate) fn bytes_of<M: Message>(m: &M) -> Vec<u8> {
    let mut e = Encoder::new(Vec::new());
    m.write(&mut e);
    e.into_writer()
}

/// Decodes a message from its bytes; `None` if they aren't one.
pub(crate) fn from_bytes<M: Message>(b: &[u8]) -> Option<M> {
    M::read(&mut Decoder::new(b))
}

impl Message for String {
    fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let _ = e.str(self);
    }
    fn read(d: &mut Decoder) -> Option<Self> {
        d.str().ok().map(String::from)
    }
}

impl Message for bool {
    fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let _ = e.bool(*self);
    }
    fn read(d: &mut Decoder) -> Option<Self> {
        d.bool().ok()
    }
}

macro_rules! numbers {
    ($($t:ty => $get:ident),*) => {
        $(
            impl Message for $t {
                fn write(&self, e: &mut Encoder<Vec<u8>>) {
                    let _ = e.encode(self);
                }
                fn read(d: &mut Decoder) -> Option<Self> {
                    d.$get().ok()
                }
            }
        )*
    };
}

numbers!(u8 => u8, u16 => u16, u32 => u32, u64 => u64, i8 => i8, i16 => i16, i32 => i32, i64 => i64);

#[cfg(test)]
mod tests {
    use super::*;
    use crate as rapidkoi;

    #[derive(Debug, PartialEq, rapidkoi::Message)]
    enum Inner {
        Toggle(u32),
        Rename { id: u32, name: String },
    }

    #[derive(Debug, PartialEq, rapidkoi::Message)]
    enum Outer {
        Add,
        List(Inner),
    }

    #[test]
    fn unit_and_field_variants_round_trip() {
        assert_eq!(bytes_of(&Outer::Add), bytes_of(&String::from("Add")));
        for m in [Outer::Add, Outer::List(Inner::Toggle(3)), Outer::List(Inner::Rename { id: 1, name: "Pay".into() })] {
            let b = bytes_of(&m);
            assert_eq!(from_bytes::<Outer>(&b), Some(m));
        }
        // ["List", ["Toggle", 3]]
        let b = bytes_of(&Outer::List(Inner::Toggle(3)));
        let mut d = Decoder::new(&b);
        assert_eq!(d.array().unwrap(), Some(2));
        assert_eq!(d.str().unwrap(), "List");
        assert_eq!(d.array().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.u32().unwrap()), ("Toggle", 3));
    }

    #[test]
    fn something_else_is_no_message() {
        assert_eq!(from_bytes::<Outer>(&bytes_of(&String::from("Nope"))), None);
        assert_eq!(from_bytes::<Outer>(&[0xf5]), None);
    }
}
