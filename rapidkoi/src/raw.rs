//! Contract 5's primitives (PRP-037): boxes, texts and typing boxes, each
//! with a style, a role and what it sends. Everything on screen is one of
//! these; the widgets in this crate ([`crate::button`], [`crate::checkbox`]…)
//! are built from them. Use these directly to make your own.
//!
//! ```
//! use rapidkoi::raw::{boxed, text, Role};
//! use rapidkoi::style::{Background, Color, Spacing, Space};
//! use rapidkoi::View;
//!
//! let add: View = boxed()
//!     .role(Role::Button)
//!     .on_press(String::from("add"))
//!     .style([Background::color(Color::system("accent")), Spacing::padding(Space::Sm)])
//!     .children([text("Add")]);
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use minicbor::Encoder;

use crate::message::{bytes_of, from_bytes, Message};
use crate::style::{Item, Style};

/// A value in a node's props, as CBOR.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Val {
    Text(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
    Map(Vec<(String, Val)>),
    Array(Vec<Val>),
    /** A message, already encoded (PRP-039). */
    Raw(Vec<u8>),
}

impl Val {
    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        // Writing to a Vec never fails, so the results are ignored.
        match self {
            Val::Text(s) => {
                let _ = e.str(s);
            }
            Val::Int(n) => {
                let _ = e.i64(*n);
            }
            Val::Float(x) => {
                let _ = e.f64(*x);
            }
            Val::Bool(b) => {
                let _ = e.bool(*b);
            }
            Val::Null => {
                let _ = e.null();
            }
            Val::Map(entries) => {
                let _ = e.map(entries.len() as u64);
                for (k, v) in entries {
                    let _ = e.str(k);
                    v.write(e);
                }
            }
            Val::Array(items) => {
                let _ = e.array(items.len() as u64);
                for v in items {
                    v.write(e);
                }
            }
            Val::Raw(bytes) => e.writer_mut().extend_from_slice(bytes),
        }
    }
}

/// What a node is, for the keyboard and screen readers: AccessKit's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Button,
    Checkbox,
    Switch,
    Tab,
    TabList,
    Heading,
    List,
    ListItem,
    Group,
    Separator,
}

impl Role {
    fn name(self) -> &'static str {
        match self {
            Role::Button => "button",
            Role::Checkbox => "checkbox",
            Role::Switch => "switch",
            Role::Tab => "tab",
            Role::TabList => "tab_list",
            Role::Heading => "heading",
            Role::List => "list",
            Role::ListItem => "list_item",
            Role::Group => "group",
            Role::Separator => "separator",
        }
    }
}

/// The props that carry a message.
const MESSAGES: [&str; 3] = ["on_press", "on_change", "on_submit"];

/// One node of a screen: a box, a text or a typing box, sending messages of
/// type `M` (PRP-039). Build them with [`boxed`], [`text`] and [`field`], or
/// with the widgets.
pub struct View<M = String> {
    kind: &'static str,
    props: Vec<(&'static str, Val)>,
    /// The style map's keys and values, in order; a later key replaces an earlier one.
    style: Vec<(String, Val)>,
    children: Option<Vec<View<M>>>,
    /// A widget button: disabled until it's given something to send.
    pub(crate) needs_press: bool,
    sends: PhantomData<fn() -> M>,
}

// By hand, so `M` needn't be Clone, Debug or PartialEq: a view holds its
// messages already encoded.
impl<M> Clone for View<M> {
    fn clone(&self) -> Self {
        View {
            kind: self.kind,
            props: self.props.clone(),
            style: self.style.clone(),
            children: self.children.clone(),
            needs_press: self.needs_press,
            sends: PhantomData,
        }
    }
}

impl<M> fmt::Debug for View<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("View")
            .field("kind", &self.kind)
            .field("props", &self.props)
            .field("style", &self.style)
            .field("children", &self.children)
            .finish()
    }
}

impl<M> PartialEq for View<M> {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.props == other.props
            && self.style == other.style
            && self.children == other.children
            && self.needs_press == other.needs_press
    }
}

impl<M: Message> View<M> {
    /// A box: the message a press (click, Enter, Space) sends.
    pub fn on_press(mut self, msg: M) -> View<M> {
        self.needs_press = false;
        self.prop("on_press", Val::Raw(bytes_of(&msg)))
    }

    /// A box: sends `msg` when `enabled`, else is disabled.
    pub fn on_press_if(self, enabled: bool, msg: M) -> View<M> {
        if enabled { self.on_press(msg) } else { self }
    }

    /// A typing box: the message each edit sends; `handle` gets the new text as `value`.
    pub fn on_change(self, msg: M) -> View<M> {
        self.prop("on_change", Val::Raw(bytes_of(&msg)))
    }

    /// A one-line typing box: the message Enter sends, with the text as `value`.
    pub fn on_submit(self, msg: M) -> View<M> {
        self.prop("on_submit", Val::Raw(bytes_of(&msg)))
    }

    /// The same view sending `wrap(m)` for each of its messages `m`: how a
    /// part's view joins its parent's (`self.list.view().map(AppMsg::List)`).
    pub fn map<N: Message>(self, wrap: impl Fn(M) -> N + Clone) -> View<N> {
        let props = self
            .props
            .into_iter()
            .map(|(name, v)| match v {
                Val::Raw(b) if MESSAGES.contains(&name) => match from_bytes::<M>(&b) {
                    Some(m) => (name, Val::Raw(bytes_of(&wrap(m)))),
                    None => (name, Val::Raw(b)),
                },
                other => (name, other),
            })
            .collect();
        View {
            kind: self.kind,
            props,
            style: self.style,
            children: self.children.map(|kids| kids.into_iter().map(|k| k.map(wrap.clone())).collect()),
            needs_press: self.needs_press,
            sends: PhantomData,
        }
    }
}

impl<M> View<M> {
    pub(crate) fn new(kind: &'static str) -> View<M> {
        View { kind, props: Vec::new(), style: Vec::new(), children: None, needs_press: false, sends: PhantomData }
    }

    pub(crate) fn prop(mut self, name: &'static str, value: Val) -> View<M> {
        self.props.retain(|(n, _)| *n != name);
        self.props.push((name, value));
        self
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.props.iter().any(|(n, _)| *n == name)
    }

    /// Styles it: each value replaces the same one given before, so a
    /// widget's look can be changed by styling it again.
    pub fn style(mut self, items: impl IntoIterator<Item = Style>) -> View<M> {
        for item in items {
            match item.0 {
                Item::Set(key, value) => set(&mut self.style, key, value),
                Item::When(state, inner) => {
                    let name = state.name();
                    let mut look = match self.style.iter().find(|(k, _)| k == name) {
                        Some((_, Val::Map(es))) => es.clone(),
                        _ => Vec::new(),
                    };
                    for i in inner {
                        if let Item::Set(key, value) = i.0 {
                            set(&mut look, key, value);
                        }
                    }
                    set(&mut self.style, String::from(name), Val::Map(look));
                }
            }
        }
        self
    }

    /// Styles the text inside a widget (its first child): a button's label colour.
    pub fn restyle_text(self, s: Style) -> View<M> {
        self.restyle_first(s)
    }

    /// Styles its first child: how a widget sets the text inside it.
    pub(crate) fn restyle_first(mut self, s: Style) -> View<M> {
        if let Some(kids) = &mut self.children {
            if let Some(k) = kids.first_mut() {
                *k = k.clone().style([s]);
            }
        }
        self
    }

    /// What it is, for the keyboard and screen readers.
    pub fn role(self, r: Role) -> View<M> {
        self.prop("role", Val::Text(String::from(r.name())))
    }

    /// A checkbox's or switch's state.
    pub fn checked(self, on: bool) -> View<M> {
        self.prop("checked", Val::Bool(on))
    }

    /// A tab's or list item's state.
    pub fn selected(self, on: bool) -> View<M> {
        self.prop("selected", Val::Bool(on))
    }

    /// Greyed out: a pressable box sends nothing.
    pub fn disabled(self, on: bool) -> View<M> {
        self.prop("disabled", Val::Bool(on))
    }

    /// What a screen reader says, when the text isn't enough.
    pub fn label(self, s: &str) -> View<M> {
        self.prop("label", Val::Text(String::from(s)))
    }

    /// A typing box: the hint shown while it's empty.
    pub fn placeholder(self, hint: &str) -> View<M> {
        self.prop("placeholder", Val::Text(String::from(hint)))
    }

    /// A typing box: how many lines it shows (1 to 20).
    pub fn lines(self, n: u8) -> View<M> {
        self.prop("lines", Val::Int(n.clamp(1, 20) as i64))
    }

    /// The screen's title, on the root: Rapidkoi shows it in the tab, the
    /// window title and "Where you left off".
    pub fn screen_title(self, title: &str) -> View<M> {
        self.prop("title", Val::Text(String::from(title)))
    }

    /// A box: its children.
    pub fn children(mut self, kids: impl IntoIterator<Item = View<M>>) -> View<M> {
        self.children = Some(kids.into_iter().collect());
        self
    }

    /// Encodes this tree as contract 5 CBOR.
    pub fn encode(&self) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        self.write(&mut e);
        e.into_writer()
    }

    fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let mut props = self.props.clone();
        // A widget button with nothing to send is shown disabled.
        if self.needs_press && !self.has("on_press") {
            props.push(("on_press", Val::Null));
            props.retain(|(n, _)| *n != "disabled");
            props.push(("disabled", Val::Bool(true)));
        }
        let styled = !self.style.is_empty();
        let fields = if self.children.is_some() { 3 } else { 2 };
        let _ = e.map(fields);
        let _ = e.str("kind").and_then(|e| e.str(self.kind));
        let _ = e.str("props").and_then(|e| e.map(props.len() as u64 + styled as u64));
        for (name, value) in &props {
            let _ = e.str(name);
            value.write(e);
        }
        if styled {
            let _ = e.str("style");
            Val::Map(self.style.clone()).write(e);
        }
        if let Some(kids) = &self.children {
            let _ = e.str("children").and_then(|e| e.array(kids.len() as u64));
            for k in kids {
                k.write(e);
            }
        }
    }
}

fn set(map: &mut Vec<(String, Val)>, key: String, value: Val) {
    map.retain(|(k, _)| *k != key);
    map.push((key, value));
}

/// A box: lays out its children (a column, unless styled as a row).
pub fn boxed<M>() -> View<M> {
    View::new("box").children([])
}

/// A text.
pub fn text<M>(s: &str) -> View<M> {
    View::new("text").prop("text", Val::Text(String::from(s)))
}

/// A text made of parts (contract 8): words, the person's name and times,
/// put together by Rapidkoi while drawing ([`crate::Part`]). At most 64.
pub fn text_of<M>(parts: impl IntoIterator<Item = crate::Part>) -> View<M> {
    View::new("text").prop("parts", Val::Array(crate::parts::vals(parts)))
}

/// A typing box showing `value`; give it [`View::on_change`].
pub fn field<M>(value: &str) -> View<M> {
    View::new("input").prop("value", Val::Text(String::from(value)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Background, Color, Spacing, Space, State, When};

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| alloc::format!("{b:02x}")).collect()
    }

    #[test]
    fn a_text_encodes_exactly() {
        // {"kind": "text", "props": {"text": "hi"}}
        let expected = ["a2", "64", "6b696e64", "64", "74657874", "65", "70726f7073", "a1", "64", "74657874", "62", "6869"].concat();
        assert_eq!(hex(&text::<String>("hi").encode()), expected);
    }

    #[test]
    fn a_styled_pressable_box_encodes_its_style_role_and_message() {
        let v = boxed()
            .role(Role::Button)
            .on_press(String::from("add"))
            .style([Background::color(Color::system("accent")), Spacing::padding(Space::Sm)])
            .style([When(State::Hover, [Background::color(Color::rgb(1, 2, 3))])]);
        let bytes = v.encode();
        let mut d = minicbor::Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(3));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "box"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("props", Some(3)));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("role", "button"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("on_press", "add"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("style", Some(3)));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("background", "accent"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("padding", "sm"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("hover", Some(1)));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("background", "#010203"));
    }

    #[test]
    fn styling_again_replaces_the_same_value() {
        let v = boxed::<String>().style([Spacing::gap(Space::Sm)]).style([Spacing::gap(Space::Lg)]);
        assert_eq!(v.style, alloc::vec![(String::from("gap"), Val::Text(String::from("lg")))]);
    }
}
