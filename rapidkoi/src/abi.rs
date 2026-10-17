//! The calling convention of contract v1: bytes in through `alloc`, bytes
//! out as (pointer, length) packed in an i64, and the CBOR of the event
//! envelope and the update result.
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

use crate::App;

#[doc(hidden)]
pub const __CONTRACT_VERSION: i32 = 1;

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

/// One event, decoded from the envelope `{ "msg": text, "value"?: text }`.
#[derive(Debug, PartialEq, Eq)]
pub struct Event {
    pub msg: String,
    pub value: Option<String>,
}

impl Event {
    /// Decodes an envelope; anything unexpected is `None`.
    pub fn decode(bytes: &[u8]) -> Option<Event> {
        let mut d = Decoder::new(bytes);
        let n = d.map().ok()??;
        let mut msg = None;
        let mut value = None;
        for _ in 0..n {
            match d.str().ok()? {
                "msg" => msg = Some(String::from(d.str().ok()?)),
                "value" => value = Some(String::from(d.str().ok()?)),
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
    save(&A::init())
}

#[doc(hidden)]
pub fn abi_update<A: App>(state: &[u8], event: &[u8]) -> Vec<u8> {
    let app = load::<A>(state);
    let next = match Event::decode(event) {
        Some(e) => app.update(&e.msg, e.value.as_deref()),
        None => app,
    };
    let mut e = Encoder::new(Vec::new());
    // {"state": <bytes>, "effects": []}; writing to a Vec never fails.
    let _ = e.map(2);
    let _ = e.str("state").and_then(|e| e.bytes(&save(&next)));
    let _ = e.str("effects").and_then(|e| e.array(0));
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
        assert_eq!(Event::decode(&bytes), Some(Event { msg: "name".into(), value: Some("Amina".into()) }));
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
}
