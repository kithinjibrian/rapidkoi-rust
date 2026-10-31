//! A window's state as named fields, and which go in its address (contract
//! 4, PRP-036). `#[derive(State)]` writes [`State`] for a struct and one
//! [`Field`] per field; [`Next`] is what [`crate::App::handle`] returns: the
//! new state, what to ask Rapidkoi for, and which fields the address shows.

use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;
use minicbor::Encoder;

use crate::Ask;

/// A state Rapidkoi can read: a CBOR map of named fields. Derive it with
/// `#[derive(State)]` (and `Default`, the start of a fresh run).
pub trait State: Sized + Default {
    /// Writes the state as a map of its field names.
    fn encode_state(&self, e: &mut Encoder<Vec<u8>>);
    /// Reads it back; a missing field is its default, an unknown one skipped.
    fn decode_state(bytes: &[u8]) -> Option<Self>;
    /// What kind each simple field is, by its path (contract 7): `number`,
    /// `text` or `bool`, an `Option` as its inner type's. Sent with `on_init`,
    /// so a link can set a field that starts empty. Written by the derive.
    fn kinds() -> Vec<(String, &'static str)> {
        Vec::new()
    }
}

/// The most steps a link path may take (PRP-039): `a.b.c.d`.
pub const MAX_PATH: usize = 4;

/// A field of state `S` holding a `T`, named for the address: `Counter::count`,
/// or a path into nested state: `App::editor.then(Editor::task)`.
pub struct Field<S, T> {
    steps: [&'static str; MAX_PATH],
    len: usize,
    of: PhantomData<fn(S) -> T>,
}

impl<S, T> Field<S, T> {
    #[doc(hidden)]
    pub const fn new(name: &'static str) -> Self {
        Field { steps: [name, "", "", ""], len: 1, of: PhantomData }
    }

    /// A path one step further in: `App::editor.then(Editor::task)` is
    /// `editor.task`. Panics past four steps, Rapidkoi's limit.
    ///
    /// ```
    /// use rapidkoi::{Next, State};
    ///
    /// #[derive(Default, State)]
    /// struct Editor { task: u32, notes: Vec<String> }
    /// #[derive(Default, State)]
    /// struct App { editor: Editor }
    ///
    /// let next = Next::state(App::default()).link(App::editor.then(Editor::task));
    /// ```
    ///
    /// A path ending on a list can't go in an address:
    ///
    /// ```compile_fail
    /// use rapidkoi::{Next, State};
    ///
    /// #[derive(Default, State)]
    /// struct Editor { task: u32, notes: Vec<String> }
    /// #[derive(Default, State)]
    /// struct App { editor: Editor }
    ///
    /// let next = Next::state(App::default()).link(App::editor.then(Editor::notes));
    /// ```
    pub fn then<U>(self, next: Field<T, U>) -> Field<S, U> {
        assert!(self.len + next.len <= MAX_PATH, "a link path is at most four steps");
        let mut steps = self.steps;
        for i in 0..next.len {
            steps[self.len + i] = next.steps[i];
        }
        Field { steps, len: self.len + next.len, of: PhantomData }
    }

    /// The field's name, as the address writes it: its steps joined by `.`.
    pub fn name(&self) -> String {
        self.steps[..self.len].join(".")
    }
}

impl<S, T> Clone for Field<S, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, T> Copy for Field<S, T> {}

/// A value that can go in an address: a number, text, true/false, or an
/// optional one of those (none leaves it out). Lists and maps can't.
pub trait Simple {
    /// Its kind in `kinds` (contract 7): `number`, `text` or `bool`.
    const KIND: &'static str;
}

macro_rules! simple {
    ($kind:literal: $($t:ty),*) => { $( impl Simple for $t { const KIND: &'static str = $kind; } )* };
}

simple!("number": i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
simple!("text": alloc::string::String);
simple!("bool": bool);

impl<T: Simple> Simple for Option<T> {
    const KIND: &'static str = T::KIND;
}

/// What the address names after an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Link {
    /// As it was.
    Keep,
    /// No fields.
    Clear,
    /// These fields, in order.
    Set(Vec<String>),
}

/// What [`crate::App::handle`] returns: the new state, what it asks for,
/// and which of its fields go in the window's address.
///
/// ```
/// use rapidkoi::{Next, State};
///
/// #[derive(Default, State)]
/// struct Counter { count: i64, notes: Vec<i64> }
///
/// let next = Next::state(Counter { count: 3, notes: vec![] }).link(Counter::count);
/// ```
///
/// Only a simple field can go in an address, checked as it's built:
///
/// ```compile_fail
/// use rapidkoi::{Next, State};
///
/// #[derive(Default, State)]
/// struct Counter { count: i64, notes: Vec<i64> }
///
/// let next = Next::state(Counter { count: 3, notes: vec![] }).link(Counter::notes);
/// ```
pub struct Next<S> {
    pub(crate) state: S,
    pub(crate) asks: Vec<Ask>,
    pub(crate) link: Link,
}

impl<S> Next<S> {
    /// The new state; the address keeps the fields it named.
    pub fn state(state: S) -> Self {
        Next { state, asks: Vec::new(), link: Link::Keep }
    }

    /// Puts `field` in the address, after any named before it; its value is
    /// always the state's own.
    pub fn link<T: Simple>(mut self, field: Field<S, T>) -> Self {
        let mut names = match self.link {
            Link::Set(names) => names,
            _ => Vec::new(),
        };
        let name = field.name();
        if !names.contains(&name) {
            names.push(name);
        }
        self.link = Link::Set(names);
        self
    }

    /// The address names no fields.
    pub fn no_link(mut self) -> Self {
        self.link = Link::Clear;
        self
    }

    /// Asks Rapidkoi for something; the reply comes as an event.
    pub fn ask(mut self, ask: impl Into<Ask>) -> Self {
        self.asks.push(ask.into());
        self
    }

    /// The new state, as it'll be kept.
    pub fn new_state(&self) -> &S {
        &self.state
    }

    /// The new state, taken out.
    pub fn into_state(self) -> S {
        self.state
    }

    /// What it asks for, in order.
    pub fn asks(&self) -> &[Ask] {
        &self.asks
    }

    /// The fields the address names, when this changes them: `Some([])` for none.
    pub fn linked(&self) -> Option<Vec<String>> {
        match &self.link {
            Link::Keep => None,
            Link::Clear => Some(Vec::new()),
            Link::Set(names) => Some(names.clone()),
        }
    }
}

/// The pair form, `(state, asks)`: the address keeps the fields it named.
impl<S> From<(S, Vec<Ask>)> for Next<S> {
    fn from((state, asks): (S, Vec<Ask>)) -> Self {
        Next { state, asks, link: Link::Keep }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;

    #[derive(Debug, Default, PartialEq, crate::State)]
    struct Notes {
        count: i64,
        filter: String,
        done: bool,
        tags: Vec<String>,
        due: Option<u32>,
    }

    fn encoded(s: &Notes) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        s.encode_state(&mut e);
        e.into_writer()
    }

    #[test]
    fn a_state_round_trips_as_named_fields() {
        let s = Notes { count: -3, filter: "work".into(), done: true, tags: vec!["a".into()], due: Some(4) };
        let bytes = encoded(&s);
        let mut d = minicbor::Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(5));
        assert_eq!((d.str().unwrap(), d.i64().unwrap()), ("count", -3));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("filter", "work"));
        assert_eq!(Notes::decode_state(&bytes), Some(s));
    }

    #[test]
    fn missing_fields_are_their_default_and_unknown_ones_skipped() {
        let mut e = Encoder::new(Vec::new());
        e.map(2).unwrap().str("count").unwrap().i64(7).unwrap().str("gone").unwrap().array(0).unwrap();
        let s = Notes::decode_state(&e.into_writer()).unwrap();
        assert_eq!(s, Notes { count: 7, ..Notes::default() });
        assert_eq!(Notes::decode_state(&[0x80]), None);
    }

    #[test]
    fn next_names_the_address_fields() {
        let n = Next::state(Notes::default()).link(Notes::count).link(Notes::due).link(Notes::count);
        assert_eq!(n.linked(), Some(vec![String::from("count"), String::from("due")]));
        assert_eq!(Next::state(Notes::default()).linked(), None);
        assert_eq!(Next::state(Notes::default()).link(Notes::filter).no_link().linked(), Some(Vec::<String>::new()));
        let asked = Next::state(Notes::default()).ask(Ask::identity("me"));
        assert_eq!(asked.asks(), &[Ask::identity("me")]);
    }

    #[derive(Debug, Default, PartialEq, crate::State)]
    struct Editor {
        task: u32,
        notes: Vec<String>,
    }

    #[derive(Debug, Default, PartialEq, crate::State)]
    struct Whole {
        page: String,
        editor: Editor,
    }

    #[test]
    fn nested_state_is_named_fields_too() {
        // PRP-039: so a path like editor.task finds its way.
        let w = Whole { page: "task".into(), editor: Editor { task: 42, notes: vec![] } };
        let mut e = Encoder::new(Vec::new());
        w.encode_state(&mut e);
        let bytes = e.into_writer();
        let mut d = minicbor::Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("page", "task"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("editor", Some(2)));
        assert_eq!((d.str().unwrap(), d.u32().unwrap()), ("task", 42));
        assert_eq!(Whole::decode_state(&bytes), Some(w));
    }

    #[test]
    fn then_names_a_path() {
        let n = Next::state(Whole::default()).link(Whole::page).link(Whole::editor.then(Editor::task));
        assert_eq!(n.linked(), Some(vec![String::from("page"), String::from("editor.task")]));
    }
}
