//! Write Rapidkoi apps in Rust (UI contract v2).
//!
//! An app is three pure functions over its state: [`App::init`],
//! [`App::update`] and [`App::view`]. Rapidkoi holds the state between
//! calls, as CBOR bytes, so the state type derives minicbor's `Encode` and
//! `Decode`. [`export!`] writes the WebAssembly exports the shell calls.
//!
//! ```ignore
//! #[derive(minicbor::Encode, minicbor::Decode)]
//! struct Counter { #[n(0)] count: i64 }
//!
//! impl rapidkoi::App for Counter {
//!     fn init() -> Self { Counter { count: 0 } }
//!     fn update(self, msg: &str, _value: Option<&str>) -> Self {
//!         match msg { "add" => Counter { count: self.count + 1 }, _ => self }
//!     }
//!     fn view(&self) -> rapidkoi::View {
//!         rapidkoi::button("Add").on_press("add")
//!     }
//! }
//!
//! rapidkoi::export!(Counter);
//! ```
//!
//! In this version of the crate, messages are text.
//!
//! Contract v2 adds two things. An app can ask Rapidkoi for its own key
//! for the person ([`Ask::identity`]) or a sign-in signature
//! ([`Ask::sign`]), from [`App::start`] or [`App::step`]; the reply comes
//! to [`App::answer`]. And text can hold [`FIRST_NAME`] and [`LAST_NAME`]:
//! Rapidkoi fills in the person's name while drawing, so the app shows it
//! without ever knowing it.

#![cfg_attr(not(test), no_std)]
extern crate alloc;

mod abi;
mod view;

pub use abi::{Event, Payload, __alloc, __finish, __input, __CONTRACT_VERSION};
pub use view::{button, column, input, row, text, Space, View};
use alloc::string::String;
use alloc::vec::Vec;

/// Shown as the person's first name, filled in by Rapidkoi while drawing.
pub const FIRST_NAME: &str = "{FIRST_NAME}";
/// Shown as the person's last name, filled in by Rapidkoi while drawing.
pub const LAST_NAME: &str = "{LAST_NAME}";

/// Something to ask Rapidkoi for. The reply comes to [`App::answer`] with `msg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// This app's own public key for the person (32 bytes): a stable ID meaning nothing elsewhere.
    Identity(String),
    /// A sign-in signature (64 bytes) over the app's verified host and this challenge (1–1024 bytes).
    Sign(String, Vec<u8>),
}

impl Ask {
    /// Asks for the person's public key for this app.
    pub fn identity(msg: &str) -> Ask {
        Ask::Identity(msg.into())
    }
    /// Asks for a sign-in signature over `challenge`.
    pub fn sign(msg: &str, challenge: &[u8]) -> Ask {
        Ask::Sign(msg.into(), challenge.to_vec())
    }
}

/// A Rapidkoi app: its state, and how messages change it.
pub trait App: Sized + minicbor::Encode<()> + for<'b> minicbor::Decode<'b, ()> {
    /// The first state.
    fn init() -> Self;
    /// The next state after `msg`; `value` is an input's new text.
    fn update(self, msg: &str, value: Option<&str>) -> Self;
    /// What's on screen in this state.
    fn view(&self) -> View;
    /// The first state and what to ask Rapidkoi for; [`App::init`] and nothing by default.
    fn start() -> (Self, Vec<Ask>) {
        (Self::init(), Vec::new())
    }
    /// [`App::update`], able to ask for things too; asks for nothing by default.
    fn step(self, msg: &str, value: Option<&str>) -> (Self, Vec<Ask>) {
        (self.update(msg, value), Vec::new())
    }
    /// Rapidkoi's reply to an [`Ask`] sent with `msg`: the bytes, or `None`
    /// (not signed in, or an unsigned app). Ignored by default.
    fn answer(self, msg: &str, value: Option<&[u8]>) -> (Self, Vec<Ask>) {
        let _ = (msg, value);
        (self, Vec::new())
    }
}

/// Writes the exports contract v2 needs (`rapidkoi_version`, `alloc`,
/// `init`, `update`, `view`) for the app type `$app`.
#[macro_export]
macro_rules! export {
    ($app:ty) => {
        #[no_mangle]
        pub extern "C" fn rapidkoi_version() -> i32 {
            $crate::__CONTRACT_VERSION
        }

        #[no_mangle]
        pub extern "C" fn alloc(len: i32) -> i32 {
            $crate::__alloc(len)
        }

        #[no_mangle]
        pub extern "C" fn init() -> i64 {
            $crate::__finish($crate::abi_init::<$app>())
        }

        #[no_mangle]
        pub extern "C" fn update(sp: i32, sl: i32, ep: i32, el: i32) -> i64 {
            let state = $crate::__input(sp, sl);
            let event = $crate::__input(ep, el);
            $crate::__finish($crate::abi_update::<$app>(state, event))
        }

        #[no_mangle]
        pub extern "C" fn view(sp: i32, sl: i32) -> i64 {
            let state = $crate::__input(sp, sl);
            $crate::__finish($crate::abi_view::<$app>(state))
        }
    };
}

#[doc(hidden)]
pub use abi::{abi_init, abi_update, abi_view};
