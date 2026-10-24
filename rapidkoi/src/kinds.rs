//! What kind each link field is (contract 7, `kinds`): written for the app
//! by `#[derive(State)]`, so Rapidkoi can set a field that starts empty (an
//! `Option`) from a link. Not for apps to use directly.
//!
//! The derive can't see from a field's type whether it's a simple value, a
//! nested state or neither, so it calls `Probe::<T>::new().kinds_into(..)`
//! through three references and lets method resolution pick the first that
//! fits ("autoref specialization"): a simple value (`&&Probe`), a nested
//! state (`&Probe`), anything else (`Probe`, nothing).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::state::{Simple, State, MAX_PATH};

/// Stands for a field's type while the derive asks what kind it is.
pub struct Probe<T>(PhantomData<T>);

impl<T> Probe<T> {
    /// A probe for type `T`.
    pub const fn new() -> Self {
        Probe(PhantomData)
    }
}

impl<T> Default for Probe<T> {
    fn default() -> Self {
        Probe::new()
    }
}

/// A simple field: its own kind.
pub trait SimpleKind {
    /// Adds `(name, kind)`.
    fn kinds_into(&self, name: &str, out: &mut Vec<(String, &'static str)>);
}

impl<T: Simple> SimpleKind for &&Probe<T> {
    fn kinds_into(&self, name: &str, out: &mut Vec<(String, &'static str)>) {
        out.push((String::from(name), T::KIND));
    }
}

/// A nested state: its own fields' kinds, under its name.
pub trait NestedKind {
    /// Adds the nested state's kinds as `name.path`, within `MAX_PATH` steps.
    fn kinds_into(&self, name: &str, out: &mut Vec<(String, &'static str)>);
}

impl<T: State> NestedKind for &Probe<T> {
    fn kinds_into(&self, name: &str, out: &mut Vec<(String, &'static str)>) {
        for (path, kind) in T::kinds() {
            let full = format!("{name}.{path}");
            if full.split('.').count() <= MAX_PATH {
                out.push((full, kind));
            }
        }
    }
}

/// Anything else (a list, bytes, a map): no kind, never in an address.
pub trait NoKind {
    /// Adds nothing.
    fn kinds_into(&self, _name: &str, _out: &mut Vec<(String, &'static str)>) {}
}

impl<T> NoKind for Probe<T> {}

#[cfg(test)]
mod tests {
    use crate::State;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    #[derive(Default, State)]
    struct Editor {
        task: u32,
        draft: Option<String>,
    }

    #[derive(Default, State)]
    struct Notes {
        count: i64,
        note: Option<u32>,
        title: String,
        done: bool,
        editor: Editor,
        notes: Vec<String>,
        me: Option<Vec<u8>>,
    }

    #[test]
    fn simple_fields_say_their_kind_and_nested_ones_their_path() {
        let got: Vec<(String, &str)> = Notes::kinds();
        assert_eq!(
            got,
            vec![
                (String::from("count"), "number"),
                (String::from("note"), "number"),
                (String::from("title"), "text"),
                (String::from("done"), "bool"),
                (String::from("editor.task"), "number"),
                (String::from("editor.draft"), "text"),
            ]
        );
    }
}
