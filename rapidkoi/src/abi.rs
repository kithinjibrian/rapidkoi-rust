//! The calling convention of contract v2: bytes in through `alloc`, bytes
//! out as (pointer, length) packed in an i64, and the CBOR of the event
//! envelope and the `{state, effects}` result of `init` and `update`.
//!
//! Memory: every buffer handed to or returned from Rapidkoi lives in `LIVE`
//! until the first `alloc` after an export has returned. By then Rapidkoi
//! has read the last output, so it is safe to free everything at once.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;
use minicbor::{Decoder, Encoder};

use crate::{App, Ask};

#[doc(hidden)]
pub const __CONTRACT_VERSION: i32 = 2;

struct Arena {
    live: Vec<Box<[u8]>>,
    done: bool,
}

// WebAssembly here is single-threaded; this wrapper only lets the arena
// be a `static` without `static mut`.
struct Single(RefCell<Arena>);
unsafe impl Sync for Single {}

static ARENA: Single = Single(RefCell::new(Arena { live: Vec::new(), done: true }));

#[doc(hidden)]
pub fn __alloc(len: i32) -> i32 {
    let mut a = ARENA.0.borrow_mut();
    if a.done {
        a.live.clear();
        a.done = false;
    }
    let buf = vec![0u8; len.max(0) as usize].into_boxed_slice();
    let ptr = buf.as_ptr() as usize as i32;
    a.live.push(buf);
    ptr
}

#[doc(hidden)]
pub fn __input(ptr: i32, len: i32) -> &'static [u8] {
    if len <= 0 {
        return &[];
    }
    // Rapidkoi wrote these bytes into a buffer from `__alloc`, which stays
    // alive until the next call's first `alloc`.
    unsafe { core::slice::from_raw_parts(ptr as usize as *const u8, len as usize) }
}

#[doc(hidden)]
pub fn __finish(out: Vec<u8>) -> i64 {
    let out = out.into_boxed_slice();
    let packed = ((out.as_ptr() as usize as u64) << 32) | out.len() as u64;
    let mut a = ARENA.0.borrow_mut();
    a.live.push(out);
    a.done = true;
    packed as i64
}

/// What came with a message: an input's text, or Rapidkoi's answer to an
/// [`Ask`] (bytes, or nothing when you aren't signed in).
#[derive(Debug, PartialEq, Eq)]
pub enum Payload {
    /// A plain event: a button's press.
    None,
    /// An input's new text.
    Text(String),
    /// Rapidkoi's answer: a public key or a signature.
    Bytes(Vec<u8>),
    /// Rapidkoi's answer when there's nothing to give (not signed in, an unsigned app).
    Null,
}

/// One message, decoded from the envelope `{ "msg": text, "value"?: … }`.
#[derive(Debug, PartialEq, Eq)]
pub struct Event {
    pub msg: String,
    pub value: Payload,
}

impl Event {
    /// Decodes an envelope; anything unexpected is `None`.
    pub fn decode(bytes: &[u8]) -> Option<Event> {
        let mut d = Decoder::new(bytes);
        let n = d.map().ok()??;
        let mut msg = None;
        let mut value = Payload::None;
        for _ in 0..n {
            match d.str().ok()? {
                "msg" => msg = Some(String::from(d.str().ok()?)),
                "value" => {
                    value = match d.datatype().ok()? {
                        minicbor::data::Type::String => Payload::Text(String::from(d.str().ok()?)),
                        minicbor::data::Type::Bytes => Payload::Bytes(d.bytes().ok()?.to_vec()),
                        minicbor::data::Type::Null => {
                            d.null().ok()?;
                            Payload::Null
                        }
                        _ => return None,
                    }
                }
                _ => d.skip().ok()?,
            }
        }
        Some(Event { msg: msg?, value })
    }
}

fn save<A: App>(app: &A) -> Vec<u8> {
    minicbor::to_vec(app).unwrap_or_default()
}

fn load<A: App>(state: &[u8]) -> A {
    // Rapidkoi only ever hands back state this app made; if it can't be
    // read, starting over is the safest answer.
    minicbor::decode(state).unwrap_or_else(|_| A::init())
}

#[doc(hidden)]
pub fn abi_init<A: App>() -> Vec<u8> {
    let (first, asks) = A::start();
    result(&first, &asks)
}

#[doc(hidden)]
pub fn abi_update<A: App>(state: &[u8], event: &[u8]) -> Vec<u8> {
    let app = load::<A>(state);
    let (next, asks) = match Event::decode(event) {
        Some(Event { msg, value: Payload::Bytes(b) }) => app.answer(&msg, Some(&b)),
        Some(Event { msg, value: Payload::Null }) => app.answer(&msg, None),
        Some(Event { msg, value: Payload::Text(t) }) => app.step(&msg, Some(&t)),
        Some(Event { msg, value: Payload::None }) => app.step(&msg, None),
        None => (app, Vec::new()),
    };
    result(&next, &asks)
}

/// `{"state": <bytes>, "effects": [...]}`; writing to a Vec never fails.
fn result<A: App>(app: &A, asks: &[Ask]) -> Vec<u8> {
    let mut e = Encoder::new(Vec::new());
    let _ = e.map(2);
    let _ = e.str("state").and_then(|e| e.bytes(&save(app)));
    let _ = e.str("effects").and_then(|e| e.array(asks.len() as u64));
    for a in asks {
        match a {
            Ask::Identity(msg) => {
                let _ = e.map(2).and_then(|e| e.str("kind")).and_then(|e| e.str("identity"));
                let _ = e.str("msg").and_then(|e| e.str(msg));
            }
            Ask::Sign(msg, challenge) => {
                let _ = e.map(3).and_then(|e| e.str("kind")).and_then(|e| e.str("sign"));
                let _ = e.str("msg").and_then(|e| e.str(msg));
                let _ = e.str("challenge").and_then(|e| e.bytes(challenge));
            }
        }
    }
    e.into_writer()
}

#[doc(hidden)]
pub fn abi_view<A: App>(state: &[u8]) -> Vec<u8> {
    load::<A>(state).view().encode()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_envelope_decodes() {
        // {"msg": "name", "value": "Amina"}
        let bytes = [0xa2, 0x63, b'm', b's', b'g', 0x64, b'n', b'a', b'm', b'e', 0x65, b'v', b'a', b'l', b'u', b'e', 0x65, b'A', b'm', b'i', b'n', b'a'];
        assert_eq!(Event::decode(&bytes), Some(Event { msg: "name".into(), value: Payload::Text("Amina".into()) }));
        assert_eq!(Event::decode(&[0x80]), None);
    }

    #[test]
    fn outputs_survive_until_the_next_calls_first_alloc() {
        __alloc(4);
        let packed = __finish(vec![1, 2, 3]);
        let ptr = (packed as u64 >> 32) as usize;
        // Still alive: nothing has called alloc since.
        assert_eq!(ARENA.0.borrow().live.len(), 2);
        assert_eq!(ARENA.0.borrow().live[1].as_ptr() as usize & 0xffff_ffff, ptr & 0xffff_ffff);
        __alloc(1);
        assert_eq!(ARENA.0.borrow().live.len(), 1);
    }

    #[test]
    fn answers_decode_as_bytes_or_null() {
        // {"msg": "me", "value": h'0102'}
        let bytes = [0xa2, 0x63, b'm', b's', b'g', 0x62, b'm', b'e', 0x65, b'v', b'a', b'l', b'u', b'e', 0x42, 1, 2];
        assert_eq!(Event::decode(&bytes), Some(Event { msg: "me".into(), value: Payload::Bytes(alloc::vec![1, 2]) }));
        // {"msg": "me", "value": null}
        let null = [0xa2, 0x63, b'm', b's', b'g', 0x62, b'm', b'e', 0x65, b'v', b'a', b'l', b'u', b'e', 0xf6];
        assert_eq!(Event::decode(&null), Some(Event { msg: "me".into(), value: Payload::Null }));
    }

    #[test]
    fn asks_encode_as_v2_effects() {
        // Decode what `result` wrote and read the effects back.
        #[derive(minicbor::Encode, minicbor::Decode)]
        struct S {
            #[n(0)]
            n: i64,
        }
        impl App for S {
            fn init() -> Self { S { n: 0 } }
            fn update(self, _: &str, _: Option<&str>) -> Self { self }
            fn view(&self) -> crate::View { crate::text("") }
        }
        let bytes = result(&S { n: 1 }, &[Ask::identity("me"), Ask::sign("go", &[7, 8])]);
        let mut d = Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!(d.str().unwrap(), "state");
        d.bytes().unwrap();
        assert_eq!(d.str().unwrap(), "effects");
        assert_eq!(d.array().unwrap(), Some(2));
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "identity"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", "me"));
        assert_eq!(d.map().unwrap(), Some(3));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "sign"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", "go"));
        assert_eq!((d.str().unwrap(), d.bytes().unwrap()), ("challenge", &[7u8, 8][..]));
    }
}
