//! The calling convention of the contract (version 3): bytes in through `alloc`, bytes
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
pub const __CONTRACT_VERSION: i32 = 3;

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
    /// Every time the app is opened, fresh or from saved state: ask here for what you need.
    Opened,
    /// Back at the front while running (its tab, or the new tab, shown again):
    /// ask again for what may have changed while it was away.
    Resume,
    /// Something it may read changed while it's in front (your pins, where
    /// you left off, one of its permission settings): ask again.
    Change,
    /// One of your own messages (a click, typing); `value` is an input's text.
    Message { msg: String, value: Option<String> },
    /// Rapidkoi's reply to an identity or sign [`crate::Ask`] sent with
    /// `msg`: the bytes, or `None` (not signed in, or an unsigned app, or
    /// nothing left off).
    Answer { msg: String, value: Option<Vec<u8>> },
    /// Rapidkoi's reply to a read ([`crate::Ask::pins`], [`crate::Ask::left_off`])
    /// sent with `msg`, as CBOR: read it with [`crate::Place`].
    Data { msg: String, cbor: Vec<u8> },
    /// A fetch's response (any status, a redirect included): header names in lower case.
    Response { msg: String, status: u16, headers: Vec<(String, String)>, body: Vec<u8> },
    /// A fetch that wasn't sent or didn't come back, and Rapidkoi's sentence why.
    Failed { msg: String, error: String },
}

/// A fetch's answer value: `{status, headers, body}` or `{error}`.
enum Reply {
    Got(u16, Vec<(String, String)>, Vec<u8>),
    Error(String),
}

fn reply(d: &mut Decoder) -> Option<Reply> {
    let n = d.map().ok()??;
    let (mut status, mut headers, mut body, mut error) = (None, Vec::new(), Vec::new(), None);
    for _ in 0..n {
        match d.str().ok()? {
            "status" => status = Some(d.u16().ok()?),
            "headers" => {
                let h = d.map().ok()??;
                for _ in 0..h {
                    let k = String::from(d.str().ok()?);
                    headers.push((k, String::from(d.str().ok()?)));
                }
            }
            "body" => body = d.bytes().ok()?.to_vec(),
            "error" => error = Some(String::from(d.str().ok()?)),
            _ => d.skip().ok()?,
        }
    }
    match (error, status) {
        (Some(e), _) => Some(Reply::Error(e)),
        (None, Some(s)) => Some(Reply::Got(s, headers, body)),
        _ => None,
    }
}

impl Event {
    /// Decodes an event `{ "event": …, … }` (contract 3's `on_` names);
    /// anything unexpected (or a kind this version doesn't know) is `None`.
    pub fn decode(bytes: &[u8]) -> Option<Event> {
        let mut d = Decoder::new(bytes);
        let n = d.map().ok()??;
        let mut kind = None;
        let mut msg = None;
        let mut value: Option<(minicbor::data::Type, usize, usize)> = None;
        for _ in 0..n {
            match d.str().ok()? {
                "event" => kind = Some(String::from(d.str().ok()?)),
                "msg" => msg = Some(String::from(d.str().ok()?)),
                "value" => {
                    // Kept as its span: what it means depends on the event.
                    let t = d.datatype().ok()?;
                    let start = d.position();
                    d.skip().ok()?;
                    value = Some((t, start, d.position()));
                }
                _ => d.skip().ok()?,
            }
        }
        let raw = |span: (usize, usize)| &bytes[span.0..span.1];
        match kind?.as_str() {
            "on_init" => Some(Event::Init),
            "on_open" => Some(Event::Opened),
            "on_resume" => Some(Event::Resume),
            "on_change" => Some(Event::Change),
            "on_message" => {
                let text = match value {
                    Some((minicbor::data::Type::String, a, b)) => Some(String::from(Decoder::new(raw((a, b))).str().ok()?)),
                    None => None,
                    Some(_) => return None,
                };
                Some(Event::Message { msg: msg?, value: text })
            }
            "on_answer" => match value {
                Some((minicbor::data::Type::Bytes, a, b)) => {
                    Some(Event::Answer { msg: msg?, value: Some(Decoder::new(raw((a, b))).bytes().ok()?.to_vec()) })
                }
                Some((minicbor::data::Type::Null, _, _)) | None => Some(Event::Answer { msg: msg?, value: None }),
                Some((_, a, b)) => Some(Event::Data { msg: msg?, cbor: raw((a, b)).to_vec() }),
            },
            "on_response" | "on_fail" => {
                let (_, a, b) = value?;
                match reply(&mut Decoder::new(raw((a, b))))? {
                    Reply::Got(status, headers, body) => Some(Event::Response { msg: msg?, status, headers, body }),
                    Reply::Error(error) => Some(Event::Failed { msg: msg?, error }),
                }
            }
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
            Ask::Fetch(msg, r) => r.encode_effect(msg, &mut e),
            Ask::Pins(msg) => plain(&mut e, "pins", msg),
            Ask::LeftOff(msg) => plain(&mut e, "left_off", msg),
            Ask::ContinueLeftOff(msg) => plain(&mut e, "continue", msg),
            Ask::Session(msg) => plain(&mut e, "session", msg),
            Ask::RestoreSession(msg) => plain(&mut e, "restore_session", msg),
            Ask::OpenPin(msg, index) => {
                let _ = e.map(3).and_then(|e| e.str("kind")).and_then(|e| e.str("open_pin"));
                let _ = e.str("msg").and_then(|e| e.str(msg));
                let _ = e.str("index").and_then(|e| e.u32(*index));
            }
        }
    }
    e.into_writer()
}

/// An effect of only a kind and a msg.
fn plain(e: &mut Encoder<Vec<u8>>, kind: &str, msg: &str) {
    let _ = e.map(2).and_then(|e| e.str("kind")).and_then(|e| e.str(kind));
    let _ = e.str("msg").and_then(|e| e.str(msg));
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
        let init = enc(&[("event", &|e| { e.str("on_init").unwrap(); })]);
        assert_eq!(Event::decode(&init), Some(Event::Init));
        let msg = enc(&[("event", &|e| { e.str("on_message").unwrap(); }), ("msg", &|e| { e.str("name").unwrap(); }), ("value", &|e| { e.str("Amina").unwrap(); })]);
        assert_eq!(Event::decode(&msg), Some(Event::Message { msg: "name".into(), value: Some("Amina".into()) }));
        let ans = enc(&[("event", &|e| { e.str("on_answer").unwrap(); }), ("msg", &|e| { e.str("me").unwrap(); }), ("value", &|e| { e.bytes(&[1, 2]).unwrap(); })]);
        assert_eq!(Event::decode(&ans), Some(Event::Answer { msg: "me".into(), value: Some(alloc::vec![1, 2]) }));
        let none = enc(&[("event", &|e| { e.str("on_answer").unwrap(); }), ("msg", &|e| { e.str("me").unwrap(); }), ("value", &|e| { e.null().unwrap(); })]);
        assert_eq!(Event::decode(&none), Some(Event::Answer { msg: "me".into(), value: None }));
        let got = enc(&[("event", &|e| { e.str("on_response").unwrap(); }), ("msg", &|e| { e.str("q").unwrap(); }), ("value", &|e| {
            e.map(3).unwrap().str("status").unwrap().u16(301).unwrap();
            e.str("headers").unwrap().map(1).unwrap().str("location").unwrap().str("/x").unwrap();
            e.str("body").unwrap().bytes(b"hi").unwrap();
        })]);
        assert_eq!(Event::decode(&got), Some(Event::Response { msg: "q".into(), status: 301, headers: alloc::vec![("location".into(), "/x".into())], body: b"hi".to_vec() }));
        let failed = enc(&[("event", &|e| { e.str("on_fail").unwrap(); }), ("msg", &|e| { e.str("q").unwrap(); }), ("value", &|e| {
            e.map(1).unwrap().str("error").unwrap().str("You turned off x for Notes").unwrap();
        })]);
        assert_eq!(Event::decode(&failed), Some(Event::Failed { msg: "q".into(), error: "You turned off x for Notes".into() }));
        let resume = enc(&[("event", &|e| { e.str("on_resume").unwrap(); })]);
        assert_eq!(Event::decode(&resume), Some(Event::Resume));
        let change = enc(&[("event", &|e| { e.str("on_change").unwrap(); })]);
        assert_eq!(Event::decode(&change), Some(Event::Change));
        let open = enc(&[("event", &|e| { e.str("on_open").unwrap(); })]);
        assert_eq!(Event::decode(&open), Some(Event::Opened));
        let data = enc(&[("event", &|e| { e.str("on_answer").unwrap(); }), ("msg", &|e| { e.str("pins").unwrap(); }), ("value", &|e| { e.array(0).unwrap(); })]);
        assert_eq!(Event::decode(&data), Some(Event::Data { msg: "pins".into(), cbor: alloc::vec![0x80] }));
        // Contract 2's names are another contract's: unknown here.
        let old = enc(&[("event", &|e| { e.str("init").unwrap(); })]);
        assert_eq!(Event::decode(&old), None);
        let later = enc(&[("event", &|e| { e.str("on_pause").unwrap(); })]);
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
    fn the_new_tabs_asks_encode() {
        #[derive(Default, minicbor::Encode, minicbor::Decode)]
        struct S {
            #[n(0)]
            n: i64,
        }
        impl App for S {
            fn handle(self, _: Event) -> (Self, Vec<Ask>) { (self, Vec::new()) }
            fn view(&self) -> crate::View { crate::text("") }
        }
        let asks = [Ask::pins("p"), Ask::left_off("l"), Ask::open_pin("o", 2), Ask::continue_left_off("c")];
        let bytes = result(&S { n: 1 }, &asks);
        let mut d = Decoder::new(&bytes);
        d.map().unwrap();
        d.str().unwrap();
        d.bytes().unwrap();
        d.str().unwrap();
        assert_eq!(d.array().unwrap(), Some(4));
        for (kind, msg) in [("pins", "p"), ("left_off", "l")] {
            assert_eq!(d.map().unwrap(), Some(2));
            assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", kind));
            assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", msg));
        }
        assert_eq!(d.map().unwrap(), Some(3));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "open_pin"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", "o"));
        assert_eq!((d.str().unwrap(), d.u32().unwrap()), ("index", 2));
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "continue"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", "c"));
    }

    #[test]
    fn asks_encode_as_effects() {
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
        let post = crate::Request::post("https://api.acme.com/n", &[1]).header("x-a", "b");
        let bytes = result(&S { n: 1 }, &[Ask::identity("me"), Ask::sign("go", &[7, 8]), Ask::fetch("f", post)]);
        let mut d = Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!(d.str().unwrap(), "state");
        d.bytes().unwrap();
        assert_eq!(d.str().unwrap(), "effects");
        assert_eq!(d.array().unwrap(), Some(3));
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "identity"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", "me"));
        assert_eq!(d.map().unwrap(), Some(3));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "sign"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", "go"));
        assert_eq!((d.str().unwrap(), d.bytes().unwrap()), ("challenge", &[7u8, 8][..]));
        assert_eq!(d.map().unwrap(), Some(6));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "fetch"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", "f"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("method", "POST"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("url", "https://api.acme.com/n"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("headers", Some(1)));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("x-a", "b"));
        assert_eq!((d.str().unwrap(), d.bytes().unwrap()), ("body", &[1u8][..]));
    }
}
