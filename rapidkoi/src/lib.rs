//! Write Rapidkoi apps in Rust.
//!
//! An app is two functions over its state: [`App::handle`], which answers
//! everything that happens (an [`Event`]: a fresh start, being opened, its
//! own messages, Rapidkoi's answers), and [`App::view`], what's on screen.
//! Rapidkoi holds the state between calls as named fields (contract 9), so
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
//! event, usually [`Event::Opened`], with [`Next::ask`]; the reply is an [`Event::Answer`].
//!
//! A text can be made of parts ([`text_of`]): the person's name and moments
//! in time, which Rapidkoi fills in while drawing, in the person's own zone,
//! so the app shows them without ever learning them:
//!
//! ```ignore
//! text_of(["Hello, ".into(), first_name(), "!".into()])
//! text_of(["Edited ".into(), time(note.updated).relative()])   // "Edited 5 minutes ago"
//! ```
//!
//! A time is milliseconds since 1970 UTC: what [`Ask::now`] answers
//! (read with [`Now::read`]) and SQLite's `unixepoch() * 1000` gives.
//!
//! Pictures come from the app's bundle, its servers or its blobs with [`image`] and [`image_blob`]
//! (`image("asset:trash.svg")`, `.fit(Fit::Cover)`, `.color(...)`); a box
//! scrolls with `Layout::scroll()`: Rapidkoi never scrolls an app's page.
//!
//! Every app has its own SQLite database ([`Ask::sql`], [`Ask::sql_batch`])
//! and, beside it, blobs (contract 9): large bytes under keys it names,
//! stored with [`Step::blob_set`] in a batch, read with [`Ask::blob_get`]
//! or line by line with [`Ask::blob_lines`]. Read a blob after its write's
//! answer (asks run side by side), handle pieces as they arrive, and never
//! name a table, index, trigger or view `rapidkoi_…`: those are Rapidkoi's.
//! Changing a row's primary key drops the blobs it owns. A blob shows as a
//! picture with [`image_blob`] and goes as a request's body with
//! [`Request::body_blob`], its bytes never passing through the app.
//!
//! An app can reach its own server and the servers it declares with
//! [`Ask::fetch`]; the reply is an [`Event::Response`] or [`Event::Failed`].
//! [`SignIn`] signs in to the app's own server with the person's key.

#![cfg_attr(not(test), no_std)]
extern crate alloc;
// `#[derive(State)]` names this crate `::rapidkoi`, inside it too.
extern crate self as rapidkoi;

mod abi;
mod blobs;
mod fetch;
mod home;
#[doc(hidden)]
#[path = "kinds.rs"]
pub mod __kinds;
mod message;
mod parts;
mod picture;
mod prefs;
mod signin;
pub mod raw;
mod state;
mod storage;
pub mod style;
mod view;

pub use blobs::{BlobGet, BlobInfo, BlobList, Step};
pub use abi::{Event, __alloc, __finish, __input, __CONTRACT_VERSION};
pub use fetch::{header, Request};
pub use home::{Place, Session};
pub use prefs::{Preference, Prefs};
pub use storage::Value;
pub use signin::{Progress, SignIn};
pub use rapidkoi_derive::{Message, State};
pub use message::Message;
pub use parts::{first_name, last_name, time, Now, Part, Show, Time, MAX_PARTS};
pub use picture::{image, image_blob, Fit};
pub use state::{Field, Next, Simple, State};
#[doc(hidden)]
pub use minicbor as __minicbor;
#[doc(hidden)]
pub use alloc::vec::Vec as __Vec;
#[doc(hidden)]
pub use alloc::string::String as __String;
pub use raw::View;
pub use style::Space;
pub use view::{button, checkbox, column, input, row, spacer, tabs, text, text_area, text_of};
use alloc::string::String;
use alloc::vec::Vec;

/// Something to ask Rapidkoi for. The reply comes to [`App::handle`] as an event with `msg`.
#[derive(Debug, Clone, PartialEq)]
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
    /// The time now in UTC, in milliseconds since 1970, rounded down to a second (any app).
    Now(String),
    /// Change one of them (needs the built-in `change_settings`); not answered.
    SetPreference(String, Preference),
    /// One statement against the app's database: rows, or why not.
    Sql(String, String, Vec<Value>),
    /// Steps as one transaction (statements, blobs stored or removed): what changed, or why not (nothing kept).
    SqlBatch(String, Vec<Step>),
    /// A blob's bytes, whole or a piece (contract 9).
    BlobGet(BlobGet),
    /// The app's blobs' keys (contract 9).
    BlobList(BlobList),
    /// A blob read line by line: [`Event::Lines`] until `done` (the SDK's own, over `blob_get`).
    BlobLines(String, String),
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
    /// Asks for the time now: an [`Event::Data`] to read with [`Now::read`].
    pub fn now(msg: &str) -> Ask {
        Ask::Now(msg.into())
    }
    /// Runs one statement on the app's database: an [`Event::Rows`] or an
    /// [`Event::Failed`]. Values go in `params` (`params![…]`), one per `?`.
    pub fn sql(msg: &str, sql: &str, params: Vec<Value>) -> Ask {
        Ask::Sql(msg.into(), sql.into(), params)
    }
    /// Runs statements as one transaction: an [`Event::Done`], or an
    /// [`Event::Failed`] with nothing kept.
    pub fn sql_batch(msg: &str, steps: Vec<(String, Vec<Value>)>) -> Ask {
        Ask::SqlBatch(msg.into(), steps.into_iter().map(Step::from).collect())
    }
    /// Runs steps as one transaction (contract 9): statements, and blobs
    /// stored ([`Step::blob_set`]) or removed ([`Step::blob_delete`]). An
    /// [`Event::Done`], `changes` summed over the statements, or an
    /// [`Event::Failed`] with nothing kept.
    pub fn sql_batch_steps(msg: &str, steps: Vec<Step>) -> Ask {
        Ask::SqlBatch(msg.into(), steps)
    }
    /// Writes a row and its blob in one batch, the blob owned by the row:
    /// deleting the row deletes it. `sql` writes the row (usually naming
    /// `key` in a column); the row is `table`'s whose primary key is `id`.
    ///
    /// ```
    /// use rapidkoi::{params, Ask};
    /// let photo = vec![0xff, 0xd8];
    /// let save = Ask::row_with_blob("saved", "INSERT INTO notes (id, photo) VALUES (?, ?)", params![17, "photo-17"],
    ///     "notes", 17, "photo-17", photo);
    /// ```
    pub fn row_with_blob(
        msg: &str,
        sql: &str,
        params: Vec<Value>,
        table: &str,
        id: impl Into<Value>,
        key: &str,
        bytes: impl Into<Vec<u8>>,
    ) -> Ask {
        Ask::sql_batch_steps(msg, alloc::vec![Step::blob_set(key, bytes).owner(table, id), Step::sql(sql, params)])
    }
    /// Reads `key`'s bytes, at most 4 MB from `.from(n)`: an [`Event::Blob`],
    /// or an [`Event::Failed`] (no such key; changed since `.hash(h)`).
    pub fn blob_get(msg: &str, key: &str) -> BlobGet {
        BlobGet { msg: msg.into(), key: key.into(), from: None, len: None, hash: None }
    }
    /// Lists the app's blobs, in key order (`.prefix(p)` for some): an [`Event::Blobs`].
    pub fn blob_list(msg: &str) -> BlobList {
        BlobList { msg: msg.into(), prefix: None }
    }
    /// Reads `key` line by line, piece by piece, without holding it whole
    /// (JSONL, CSV, a log): [`Event::Lines`] with each piece's finished
    /// lines, the last with `done`. If the blob changes while it's read,
    /// an [`Event::Failed`]: ask again to start over. Its place between
    /// pieces is kept in the window's state (a field named `rapidkoi_lines`).
    pub fn blob_lines(msg: &str, key: &str) -> Ask {
        Ask::BlobLines(msg.into(), key.into())
    }
    /// Asks Rapidkoi to change one of the person's choices (Rapidkoi's own Settings app only).
    pub fn set_preference(msg: &str, p: Preference) -> Ask {
        Ask::SetPreference(msg.into(), p)
    }
}

impl From<BlobGet> for Ask {
    fn from(g: BlobGet) -> Ask {
        Ask::BlobGet(g)
    }
}

impl From<BlobList> for Ask {
    fn from(l: BlobList) -> Ask {
        Ask::BlobList(l)
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
