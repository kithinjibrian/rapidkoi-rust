//! Write Rapidkoi apps in Rust (UI contract v1).
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

#![cfg_attr(not(test), no_std)]
extern crate alloc;

mod abi;
mod view;

pub use abi::{Event, __alloc, __finish, __input, __CONTRACT_VERSION};
pub use view::{button, column, input, row, text, Space, View};

/// A Rapidkoi app: its state, and how messages change it.
pub trait App: Sized + minicbor::Encode<()> + for<'b> minicbor::Decode<'b, ()> {
    /// The first state.
    fn init() -> Self;
    /// The next state after `msg`; `value` is an input's new text.
    fn update(self, msg: &str, value: Option<&str>) -> Self;
    /// What's on screen in this state.
    fn view(&self) -> View;
}

/// Writes the exports contract v1 needs (`rapidkoi_version`, `alloc`,
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
