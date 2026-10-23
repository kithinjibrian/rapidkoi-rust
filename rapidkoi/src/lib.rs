//! Write Rapidkoi apps in Rust.
//!
//! An app is two functions over its state: [`App::handle`], which answers
//! everything that happens (an [`Event`]: a fresh start, being opened, its
//! own messages, Rapidkoi's answers), and [`App::view`], what's on screen.
//! Rapidkoi holds the state between calls as named fields (contract 4), so
//! the state type derives [`State`] and `Default` (the start of a fresh
//! run). [`export!`] writes the WebAssembly exports the shell calls.
//!
//! `handle` returns a [`Next`]: the new state, what to ask Rapidkoi for,
//! and which fields go in the window's address. A link to the window
//! carries them, and opening it starts a new window with them set.
//!
//! ```ignore
//! #[derive(Default, rapidkoi::State)]
//! struct Counter { count: i64 }
//!
//! impl rapidkoi::App for Counter {
//!     fn handle(self, event: rapidkoi::Event) -> rapidkoi::Next<Self> {
//!         match event {
//!             rapidkoi::Event::Message { msg, .. } if msg == "add" => {
//!                 rapidkoi::Next::state(Counter { count: self.count + 1 }).link(Counter::count)
//!             }
//!             _ => rapidkoi::Next::state(self),
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
//! event, usually [`Event::Opened`], with [`Next::ask`]; the reply is an [`Event::Answer`]. Text
//! can hold [`FIRST_NAME`] and [`LAST_NAME`]: Rapidkoi fills in the
//! person's name while drawing, so the app shows it without ever knowing it.
//!
//! An app can reach its own server and the servers it declares with
//! [`Ask::fetch`]; the reply is an [`Event::Response`] or [`Event::Failed`].
//! [`SignIn`] signs in to the app's own server with the person's key.

#![cfg_attr(not(test), no_std)]
extern crate alloc;
// `#[derive(State)]` names this crate `::rapidkoi`, inside it too.
extern crate self as rapidkoi;

mod abi;
mod fetch;
mod home;
mod message;
mod prefs;
mod signin;
pub mod raw;
mod state;
pub mod style;
mod view;

pub use abi::{Event, __alloc, __finish, __input, __CONTRACT_VERSION};
pub use fetch::{header, Request};
pub use home::{Place, Session};
pub use prefs::{Preference, Prefs};
pub use signin::{Progress, SignIn};
pub use rapidkoi_derive::{Message, State};
pub use message::Message;
pub use state::{Field, Next, Simple, State};
#[doc(hidden)]
pub use minicbor as __minicbor;
#[doc(hidden)]
pub use alloc::vec::Vec as __Vec;
pub use raw::View;
pub use style::Space;
pub use view::{button, checkbox, column, input, row, spacer, tabs, text, text_area};
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
    /// The person's choices: light or dark, more contrast, text size (any app).
    Preferences(String),
    /// Change one of them (needs the built-in `change_settings`); not answered.
    SetPreference(String, Preference),
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
    /// Asks for the person's choices: an [`Event::Data`] to read with [`Prefs::read`].
    pub fn preferences(msg: &str) -> Ask {
        Ask::Preferences(msg.into())
    }
    /// Asks Rapidkoi to change one of the person's choices (Rapidkoi's own Settings app only).
    pub fn set_preference(msg: &str, p: Preference) -> Ask {
        Ask::SetPreference(msg.into(), p)
    }
}

/// A Rapidkoi app: its state, its messages, how events change it, and what it shows.
pub trait App: State {
    /// What its screen sends (PRP-039): an enum deriving [`Message`], or `String`.
    type Msg: Message;
    /// The next state after `event`, what to ask Rapidkoi for, and which
    /// fields go in the window's address.
    fn handle(self, event: Event<Self::Msg>) -> Next<Self>;
    /// What's on screen in this state.
    fn view(&self) -> View<Self::Msg>;
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
