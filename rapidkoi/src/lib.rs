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
//!
//! An app can reach its own server and the servers it declares with
//! [`Ask::fetch`]; the reply is an [`Event::Response`] or [`Event::Failed`].
//! [`SignIn`] signs in to the app's own server with the person's key.

#![cfg_attr(not(test), no_std)]
extern crate alloc;

mod abi;
mod fetch;
mod home;
mod signin;
mod view;

pub use abi::{Event, __alloc, __finish, __input, __CONTRACT_VERSION};
pub use fetch::{header, Request};
pub use home::{Place, Session};
pub use signin::{Progress, SignIn};
pub use view::{button, column, input, row, text, Space, View};
use alloc::string::String;
use alloc::vec::Vec;

/// Shown as the person's first name, filled in by Rapidkoi while drawing.
pub const FIRST_NAME: &str = "{FIRST_NAME}";
/// Shown as the person's last name, filled in by Rapidkoi while drawing.
pub const LAST_NAME: &str = "{LAST_NAME}";

/// Something to ask Rapidkoi for. The reply comes to [`App::handle`] as an event with `msg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// This app's own public key for the person (32 bytes): a stable ID meaning nothing elsewhere.
    Identity(String),
    /// A sign-in signature (64 bytes) over the app's verified host and this challenge (1–1024 bytes).
    Sign(String, Vec<u8>),
    /// A request to the app's own server or one it declares.
    Fetch(String, Request),
    /// The person's pins (needs the built-in `read_pins` permission).
    Pins(String),
    /// Where the person left off (needs the built-in `read_left_off`).
    LeftOff(String),
    /// Open the person's pin at this index (needs the built-in `open_apps`); not answered.
    OpenPin(String, u32),
    /// Open where the person left off (needs the built-in `open_apps`); not answered.
    ContinueLeftOff(String),
    /// The person's last session, if there's one to restore (needs the built-in `restore_session`).
    Session(String),
    /// Restore the person's last session (needs the built-in `restore_session`); not answered.
    RestoreSession(String),
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
    /// Asks Rapidkoi to send `request`.
    pub fn fetch(msg: &str, request: Request) -> Ask {
        Ask::Fetch(msg.into(), request)
    }
    /// Asks for the person's pins: an [`Event::Data`] to read with [`Place::list`].
    pub fn pins(msg: &str) -> Ask {
        Ask::Pins(msg.into())
    }
    /// Asks where the person left off: an [`Event::Data`] to read with
    /// [`Place::one`], or an [`Event::Answer`] with nothing.
    pub fn left_off(msg: &str) -> Ask {
        Ask::LeftOff(msg.into())
    }
    /// Asks Rapidkoi to open pin `index` of the person's pins as they are now.
    pub fn open_pin(msg: &str, index: u32) -> Ask {
        Ask::OpenPin(msg.into(), index)
    }
    /// Asks Rapidkoi to open where the person left off.
    pub fn continue_left_off(msg: &str) -> Ask {
        Ask::ContinueLeftOff(msg.into())
    }
    /// Asks whether there's a last session to restore: an [`Event::Data`]
    /// to read with [`Session::read`], or an [`Event::Answer`] with nothing.
    pub fn session(msg: &str) -> Ask {
        Ask::Session(msg.into())
    }
    /// Asks Rapidkoi to restore the last session.
    pub fn restore_session(msg: &str) -> Ask {
        Ask::RestoreSession(msg.into())
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
