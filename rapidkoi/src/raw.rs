//! Contract 5's primitives (PRP-037): boxes, texts and typing boxes, each
//! with a style, a role and what it sends. Everything on screen is one of
//! these; the widgets in this crate ([`crate::button`], [`crate::checkbox`]…)
//! are built from them. Use these directly to make your own.
//!
//! ```
//! use rapidkoi::raw::{boxed, text, Role};
//! use rapidkoi::style::{Background, Color, Spacing, Space};
//!
//! let add = boxed()
//!     .role(Role::Button)
//!     .on_press("add")
//!     .style([Background::color(Color::system("accent")), Spacing::padding(Space::Sm)])
//!     .children([text("Add")]);
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::Encoder;

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

/// One node of a screen: a box, a text or a typing box. Build them with
/// [`boxed`], [`text`] and [`field`], or with the widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    kind: &'static str,
    props: Vec<(&'static str, Val)>,
    /// The style map's keys and values, in order; a later key replaces an earlier one.
    style: Vec<(String, Val)>,
    children: Option<Vec<View>>,
    /// A widget button: disabled until it's given something to send.
    pub(crate) needs_press: bool,
}

impl View {
    pub(crate) fn new(kind: &'static str) -> View {
        View { kind, props: Vec::new(), style: Vec::new(), children: None, needs_press: false }
    }

    pub(crate) fn prop(mut self, name: &'static str, value: Val) -> View {
        self.props.retain(|(n, _)| *n != name);
        self.props.push((name, value));
        self
    }

    pub(crate) fn has(&self, name: &str) -> bool {
        self.props.iter().any(|(n, _)| *n == name)
    }

    /// Styles it: each value replaces the same one given before, so a
    /// widget's look can be changed by styling it again.
    pub fn style(mut self, items: impl IntoIterator<Item = Style>) -> View {
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
    pub fn restyle_text(self, s: Style) -> View {
        self.restyle_first(s)
    }

    /// Styles its first child: how a widget sets the text inside it.
    pub(crate) fn restyle_first(mut self, s: Style) -> View {
        if let Some(kids) = &mut self.children {
            if let Some(k) = kids.first_mut() {
                *k = k.clone().style([s]);
            }
        }
        self
    }

    /// What it is, for the keyboard and screen readers.
    pub fn role(self, r: Role) -> View {
        self.prop("role", Val::Text(String::from(r.name())))
    }

    /// A checkbox's or switch's state.
    pub fn checked(self, on: bool) -> View {
        self.prop("checked", Val::Bool(on))
    }

    /// A tab's or list item's state.
    pub fn selected(self, on: bool) -> View {
        self.prop("selected", Val::Bool(on))
    }

    /// Greyed out: a pressable box sends nothing.
    pub fn disabled(self, on: bool) -> View {
        self.prop("disabled", Val::Bool(on))
    }

    /// What a screen reader says, when the text isn't enough.
    pub fn label(self, s: &str) -> View {
        self.prop("label", Val::Text(String::from(s)))
    }

    /// A box: the message a press (click, Enter, Space) sends.
    pub fn on_press(mut self, msg: &str) -> View {
        self.needs_press = false;
        self.prop("on_press", Val::Text(String::from(msg)))
    }

    /// A box: sends `msg` when `enabled`, else is disabled.
    pub fn on_press_if(self, enabled: bool, msg: &str) -> View {
        if enabled { self.on_press(msg) } else { self }
    }

    /// A typing box: the hint shown while it's empty.
    pub fn placeholder(self, hint: &str) -> View {
        self.prop("placeholder", Val::Text(String::from(hint)))
    }

    /// A typing box: the message each edit sends; `handle` gets the new text as `value`.
    pub fn on_change(self, msg: &str) -> View {
        self.prop("on_change", Val::Text(String::from(msg)))
    }

    /// A one-line typing box: the message Enter sends, with the text as `value`.
    pub fn on_submit(self, msg: &str) -> View {
        self.prop("on_submit", Val::Text(String::from(msg)))
    }

    /// A typing box: how many lines it shows (1 to 20).
    pub fn lines(self, n: u8) -> View {
        self.prop("lines", Val::Int(n.clamp(1, 20) as i64))
    }

    /// The screen's title, on the root: Rapidkoi shows it in the tab, the
    /// window title and "Where you left off".
    pub fn screen_title(self, title: &str) -> View {
        self.prop("title", Val::Text(String::from(title)))
    }

    /// A box: its children.
    pub fn children(mut self, kids: impl IntoIterator<Item = View>) -> View {
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
pub fn boxed() -> View {
    View::new("box").children([])
}

/// A text.
pub fn text(s: &str) -> View {
    View::new("text").prop("text", Val::Text(String::from(s)))
}

/// A typing box showing `value`; give it [`View::on_change`].
pub fn field(value: &str) -> View {
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
        assert_eq!(hex(&text("hi").encode()), expected);
    }

    #[test]
    fn a_styled_pressable_box_encodes_its_style_role_and_message() {
        let v = boxed()
            .role(Role::Button)
            .on_press("add")
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
        let v = boxed().style([Spacing::gap(Space::Sm)]).style([Spacing::gap(Space::Lg)]);
        assert_eq!(v.style, alloc::vec![(String::from("gap"), Val::Text(String::from("lg")))]);
    }
}
