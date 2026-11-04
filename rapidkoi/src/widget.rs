//! Widgets (contract 11 addition, Rapidkoi's PRP-073): small cards an app
//! keeps on the person's new tab, only where they put it. Declare each in
//! `rapidkoi.toml` (`[[widgets]] name = "count" label = "Count"`), then fill
//! it while the app runs with [`crate::Ask::widget`]. Rapidkoi draws the
//! card's frame and top line (your icon, name and when it was sent); you
//! give the view under it and the place clicking it opens.
//!
//! A card is boxes, texts (with parts) and styles: no buttons, typing,
//! pictures or scrolling; at most 64 nodes, 8 deep, 16 KB. The ready-made
//! cards here keep to that.
//!
//! ```
//! use rapidkoi::{widget, Ask};
//! let ask: Ask = Ask::widget(widget::Widget::new("count", widget::number::<()>("12", "Your count")).field("count", 12));
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::Encoder;

use crate::notify::FieldValue;
use crate::raw::{Val, View};
use crate::style::{Color, Font, Spacing, Space, Text, TextSize, Weight};
use crate::view::{column, text};

/// A widget's card to send: build it, then [`crate::Ask::widget`] it.
#[derive(Debug, Clone, PartialEq)]
pub struct Widget {
    pub(crate) name: String,
    pub(crate) view: Vec<u8>,
    pub(crate) fields: Vec<(String, FieldValue)>,
}

impl Widget {
    /// The card for widget `name` (one your manifest declares; another
    /// stops the app), showing `view`. Clicking it opens your app as it
    /// starts, unless you give its place with [`Widget::field`].
    pub fn new<M>(name: &str, view: View<M>) -> Widget {
        Widget { name: name.into(), view: view.encode(), fields: Vec::new() }
    }

    /// Clicking it opens your app with this link field set, as a link would (at most 16).
    pub fn field(mut self, name: &str, value: impl Into<FieldValue>) -> Widget {
        self.fields.push((name.into(), value.into()));
        self
    }

    pub(crate) fn write(&self, e: &mut Encoder<Vec<u8>>) {
        let n = 4 + u64::from(!self.fields.is_empty());
        let _ = e.map(n).and_then(|e| e.str("kind")).and_then(|e| e.str("widget"));
        // A card isn't answered: its `msg` is only the ask's name.
        let _ = e.str("msg").and_then(|e| e.str("widget"));
        let _ = e.str("name").and_then(|e| e.str(&self.name));
        let _ = e.str("view");
        e.writer_mut().extend_from_slice(&self.view);
        if !self.fields.is_empty() {
            let _ = e.str("fields").and_then(|e| e.map(self.fields.len() as u64));
            for (name, v) in &self.fields {
                let _ = e.str(name);
                let _ = match v {
                    FieldValue::Number(n) => e.i64(*n),
                    FieldValue::Real(n) => e.f64(*n),
                    FieldValue::Text(s) => e.str(s),
                    FieldValue::Bool(b) => e.bool(*b),
                };
            }
        }
    }
}

/// A title and a line under it, dim: the note you last edited.
pub fn headline<M>(title: &str, line: &str) -> View<M> {
    column([
        text(title).style([Font::size(TextSize::Heading), Font::weight(Weight::Bold), Text::truncate()]),
        text(line).style([Text::color(Color::system("text_dim"))]),
    ])
    .style([Spacing::gap(Space::Xs)])
}

/// A large number and what it counts, dim: your count.
pub fn number<M>(value: &str, label: &str) -> View<M> {
    column([
        text(value).style([Font::size(TextSize::Title), Font::weight(Weight::Bold)]),
        text(label).style([Text::color(Color::system("text_dim"))]),
    ])
    .style([Spacing::gap(Space::Xs)])
}

/// How far along: "4 of 7" large, and what it's of, dim.
pub fn progress<M>(done: u32, of: u32, label: &str) -> View<M> {
    number(&alloc::format!("{done} of {of}"), label)
}

/// A title and up to four lines under it: today's events.
pub fn list<M>(title: &str, lines: &[&str]) -> View<M> {
    let mut kids = alloc::vec![text(title).style([Font::weight(Weight::Bold)])];
    for l in lines.iter().take(4) {
        kids.push(text(l).style([Text::truncate()]));
    }
    column(kids).style([Spacing::gap(Space::Xs)])
}

/// Where Rapidkoi draws the person's widgets: only Rapidkoi's own new tab
/// may place it; any other app is stopped.
#[doc(hidden)]
pub fn __slot<M>() -> View<M> {
    View::new("box").prop("slot", Val::Text(String::from("widgets")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ask;

    #[test]
    fn a_widget_ask_carries_its_name_card_and_place() {
        let w = Widget::new("count", number::<()>("12", "Your count")).field("count", 12);
        let mut e = Encoder::new(Vec::new());
        w.write(&mut e);
        let bytes = e.into_writer();
        let mut d = minicbor::Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(5));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "widget"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("msg", "widget"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("name", "count"));
        assert_eq!(d.str().unwrap(), "view");
        assert_eq!(d.map().unwrap(), Some(3));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "box"));
        // The card's props and children, each a key and its value.
        for _ in 0..4 {
            d.skip().unwrap();
        }
        assert_eq!(d.str().unwrap(), "fields");
        assert_eq!(d.map().unwrap(), Some(1));
        assert_eq!((d.str().unwrap(), d.i64().unwrap()), ("count", 12));
        assert_eq!(Ask::widget(w.clone()), Ask::Widget(w));
    }

    #[test]
    fn the_slot_is_a_box_with_no_children() {
        let bytes = __slot::<()>().encode();
        let mut d = minicbor::Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "box"));
        assert_eq!(d.str().unwrap(), "props");
        assert_eq!(d.map().unwrap(), Some(1));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("slot", "widgets"));
    }

    #[test]
    fn a_list_shows_at_most_four_lines() {
        let v = list::<()>("Today", &["a", "b", "c", "d", "e"]);
        let bytes = v.encode();
        // The title and four lines: five texts.
        assert_eq!(bytes.windows(4).filter(|w| *w == b"text").count(), 5 + 5);
    }
}
