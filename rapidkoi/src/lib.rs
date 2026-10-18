//! Write Rapidkoi apps in Rust.
//!
//! An app is two functions over its state: [`App::handle`], which answers
//! everything that happens (an [`Event`]: a fresh start, being opened, its
//! own messages, Rapidkoi's answers), and [`App::view`], what's on screen.
//! Rapidkoi holds the state between calls, as CBOR bytes, so the state type
//! derives minicbor's `Encode` and `Decode`, and `Default` (the start of a
//! fresh run). [`export!`] writes the WebAssembly exports the shell calls.
//!
//! ```ignore
//! #[derive(Default, minicbor::Encode, minicbor::Decode)]
//! struct Counter { #[n(0)] count: i64 }
//!
//! impl rapidkoi::App for Counter {
//!     fn handle(self, event: rapidkoi::Event) -> (Self, Vec<rapidkoi::Ask>) {
//!         match event {
//!             rapidkoi::Event::Message { msg, .. } if msg == "add" => (Counter { count: self.count + 1 }, vec![]),
//!             _ => (self, vec![]),
//!         }
//!     }
//!     fn view(&self) -> rapidkoi::View {
//!         rapidkoi::button("Add").on_press("add")
//!     }
//! }
//!
//! rapidkoi::export!(Counter);
//! ```
//!
//! An app can ask Rapidkoi for its own key for the person
//! ([`Ask::identity`]) or a sign-in signature ([`Ask::sign`]) from any
//! event, usually [`Event::Opened`]; the reply is an [`Event::Answer`]. Text
//! can hold [`FIRST_NAME`] and [`LAST_NAME`]: Rapidkoi fills in the
//! person's name while drawing, so the app shows it without ever knowing it.

#![cfg_attr(not(test), no_std)]
extern crate alloc;

mod abi;
mod view;

pub use abi::{Event, __alloc, __finish, __input, __CONTRACT_VERSION};
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

/// A Rapidkoi app: its state, how events change it, and what it shows.
pub trait App: Sized + Default + minicbor::Encode<()> + for<'b> minicbor::Decode<'b, ()> {
    /// The next state after `event`, and what to ask Rapidkoi for.
    fn handle(self, event: Event) -> (Self, Vec<Ask>);
    /// What's on screen in this state.
    fn view(&self) -> View;
}

/// Writes the exports the contract needs (`rapidkoi_version`, `alloc`,
/// `handle`, `view`) for the app type `$app`.
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
        pub extern "C" fn handle(sp: i32, sl: i32, ep: i32, el: i32) -> i64 {
            let state = $crate::__input(sp, sl);
            let event = $crate::__input(ep, el);
            $crate::__finish($crate::abi_handle::<$app>(state, event))
        }

        #[no_mangle]
        pub extern "C" fn view(sp: i32, sl: i32) -> i64 {
            let state = $crate::__input(sp, sl);
            $crate::__finish($crate::abi_view::<$app>(state))
        }
    };
}

#[doc(hidden)]
pub use abi::{abi_handle, abi_view};
