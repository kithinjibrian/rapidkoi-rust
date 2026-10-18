//! The calling convention of the contract (version 2): bytes in through `alloc`, bytes
//! out as (pointer, length) packed in an i64, and the CBOR of the event
//! events and the `{state, effects}` result of `handle`.
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

/// Something that happened, for [`App::handle`]. More kinds will come
/// (closing, going to the background), so match with a `_` arm.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    /// A fresh start: make the first state from `Default`.
    Init,
    /// Every time the app is opened, fresh or resumed: ask here for what you need.
    Opened,
    /// One of your own messages (a click, typing); `value` is an input's text.
    Message { msg: String, value: Option<String> },
    /// Rapidkoi's reply to an [`crate::Ask`] sent with `msg`: the bytes, or
    /// `None` (not signed in, or an unsigned app).
    Answer { msg: String, value: Option<Vec<u8>> },
}

impl Event {
    /// Decodes an event `{ "event": …, … }`; anything unexpected (or a kind
    /// this version doesn't know) is `None`.
    pub fn decode(bytes: &[u8]) -> Option<Event> {
        let mut d = Decoder::new(bytes);
        let n = d.map().ok()??;
        let mut kind = None;
        let mut msg = None;
        let mut text = None;
        let mut blob = None;
        for _ in 0..n {
            match d.str().ok()? {
                "event" => kind = Some(String::from(d.str().ok()?)),
                "msg" => msg = Some(String::from(d.str().ok()?)),
                "value" => match d.datatype().ok()? {
                    minicbor::data::Type::String => text = Some(String::from(d.str().ok()?)),
                    minicbor::data::Type::Bytes => blob = Some(d.bytes().ok()?.to_vec()),
                    minicbor::data::Type::Null => d.null().ok()?,
                    _ => return None,
                },
                _ => d.skip().ok()?,
            }
        }
        match kind?.as_str() {
            "init" => Some(Event::Init),
            "opened" => Some(Event::Opened),
            "message" => Some(Event::Message { msg: msg?, value: text }),
            "answer" => Some(Event::Answer { msg: msg?, value: blob }),
            _ => None,
        }
    }
}

fn save<A: App>(app: &A) -> Vec<u8> {
    minicbor::to_vec(app).unwrap_or_default()
}

fn load<A: App>(state: &[u8]) -> A {
    // Rapidkoi only ever hands back state this app made; if it can't be
    // read, starting over is the safest answer.
    minicbor::decode(state).unwrap_or_default()
}

#[doc(hidden)]
pub fn abi_handle<A: App>(state: &[u8], event: &[u8]) -> Vec<u8> {
    match Event::decode(event) {
        // A fresh start has no state yet: begin from Default.
        Some(Event::Init) => {
            let (next, asks) = A::default().handle(Event::Init);
            result(&next, &asks)
        }
        Some(e) => {
            let (next, asks) = load::<A>(state).handle(e);
            result(&next, &asks)
        }
        // A kind of event this app doesn't know: nothing changes.
        None => result(&load::<A>(state), &[]),
    }
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
    fn events_decode() {
        let enc = |pairs: &[(&str, &dyn Fn(&mut Encoder<Vec<u8>>))]| {
            let mut e = Encoder::new(Vec::new());
            e.map(pairs.len() as u64).unwrap();
            for (k, f) in pairs {
                e.str(k).unwrap();
                f(&mut e);
            }
            e.into_writer()
        };
        let init = enc(&[("event", &|e| { e.str("init").unwrap(); })]);
        assert_eq!(Event::decode(&init), Some(Event::Init));
        let msg = enc(&[("event", &|e| { e.str("message").unwrap(); }), ("msg", &|e| { e.str("name").unwrap(); }), ("value", &|e| { e.str("Amina").unwrap(); })]);
        assert_eq!(Event::decode(&msg), Some(Event::Message { msg: "name".into(), value: Some("Amina".into()) }));
        let ans = enc(&[("event", &|e| { e.str("answer").unwrap(); }), ("msg", &|e| { e.str("me").unwrap(); }), ("value", &|e| { e.bytes(&[1, 2]).unwrap(); })]);
        assert_eq!(Event::decode(&ans), Some(Event::Answer { msg: "me".into(), value: Some(alloc::vec![1, 2]) }));
        let none = enc(&[("event", &|e| { e.str("answer").unwrap(); }), ("msg", &|e| { e.str("me").unwrap(); }), ("value", &|e| { e.null().unwrap(); })]);
        assert_eq!(Event::decode(&none), Some(Event::Answer { msg: "me".into(), value: None }));
        let later = enc(&[("event", &|e| { e.str("closing").unwrap(); })]);
        assert_eq!(Event::decode(&later), None);
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
    fn asks_encode_as_v2_effects() {
        // Decode what `result` wrote and read the effects back.
        #[derive(Default, minicbor::Encode, minicbor::Decode)]
        struct S {
            #[n(0)]
            n: i64,
        }
        impl App for S {
            fn handle(self, _: Event) -> (Self, Vec<Ask>) { (self, Vec::new()) }
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
