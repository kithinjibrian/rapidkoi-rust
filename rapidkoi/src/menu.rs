//! A box's right-click menu (contract 11 addition, Rapidkoi's PRP-071):
//! Rapidkoi draws it as a native menu at the pointer, and picking an item
//! sends the app that item's message.
//!
//! ```ignore
//! boxed().on_press(Msg::Open(i)).menu([
//!     item("Open", Msg::Open(i)),
//!     separator(),
//!     item("Forget", Msg::Forget(i)).disabled(true),
//! ])
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::message::{bytes_of, from_bytes, Message};
use crate::raw::{Val, View};

/// The most entries a menu may have, separators included: Rapidkoi refuses more.
pub const MAX_MENU: usize = 32;

/// One entry of a box's menu: an [`item`] or a [`separator`].
pub struct Choice<M> {
    val: Val,
    sends: PhantomData<fn() -> M>,
}

/// An item: its label (1 to 200 bytes) and the message picking it sends.
pub fn item<M: Message>(label: &str, msg: M) -> Choice<M> {
    let entries = alloc::vec![
        (String::from("label"), Val::Text(String::from(label))),
        (String::from("on_pick"), Val::Raw(bytes_of(&msg))),
    ];
    Choice { val: Val::Map(entries), sends: PhantomData }
}

/// A line between groups of items.
pub fn separator<M>() -> Choice<M> {
    Choice { val: Val::Map(alloc::vec![(String::from("separator"), Val::Bool(true))]), sends: PhantomData }
}

impl<M> Choice<M> {
    /// Greyed out when `off`: picking it sends nothing. A separator is unchanged.
    pub fn disabled(mut self, off: bool) -> Choice<M> {
        if let Val::Map(es) = &mut self.val {
            if es.iter().any(|(k, _)| k == "label") {
                es.retain(|(k, _)| k != "disabled");
                es.push((String::from("disabled"), Val::Bool(off)));
            }
        }
        self
    }
}

impl<M: Message> View<M> {
    /// A box: the menu a right-click, a long press or the Menu key
    /// (Shift+F10) opens. It replaces [`View::on_context`], since both
    /// answer the same right-click; an empty menu takes it away.
    pub fn menu(self, entries: impl IntoIterator<Item = Choice<M>>) -> View<M> {
        let vals: Vec<Val> = entries.into_iter().map(|c| c.val).collect();
        let v = self.without("on_context").without("menu");
        if vals.is_empty() { v } else { v.prop("menu", Val::Array(vals)) }
    }
}

/// A menu's entries sending `wrap(m)` for each item's message `m`, for [`View::map`].
pub(crate) fn rewrapped<M: Message, N: Message>(menu: Val, wrap: &impl Fn(M) -> N) -> Val {
    let entry = |e: Val| match e {
        Val::Map(es) => Val::Map(
            es.into_iter()
                .map(|(k, v)| match v {
                    Val::Raw(b) if k == "on_pick" => match from_bytes::<M>(&b) {
                        Some(m) => (k, Val::Raw(bytes_of(&wrap(m)))),
                        None => (k, Val::Raw(b)),
                    },
                    other => (k, other),
                })
                .collect(),
        ),
        other => other,
    };
    match menu {
        Val::Array(items) => Val::Array(items.into_iter().map(entry).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::boxed;

    #[test]
    fn a_box_s_menu_encodes_its_items_a_separator_and_a_disabled_item() {
        let v = boxed()
            .on_press(String::from("open"))
            .menu([item("Open", String::from("open")), separator(), item("Forget", String::from("forget")).disabled(true)]);
        let bytes = v.encode();
        let mut d = minicbor::Decoder::new(&bytes);
        d.map().unwrap();
        d.str().unwrap();
        d.str().unwrap();
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("props", Some(2)));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("on_press", "open"));
        assert_eq!((d.str().unwrap(), d.array().unwrap()), ("menu", Some(3)));
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("label", "Open"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("on_pick", "open"));
        assert_eq!(d.map().unwrap(), Some(1));
        assert_eq!((d.str().unwrap(), d.bool().unwrap()), ("separator", true));
        assert_eq!(d.map().unwrap(), Some(3));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("label", "Forget"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("on_pick", "forget"));
        assert_eq!((d.str().unwrap(), d.bool().unwrap()), ("disabled", true));
    }

    #[test]
    fn a_menu_replaces_on_context_and_the_other_way_round() {
        let menu = boxed().on_context(String::from("details")).menu([item("Open", String::from("open"))]);
        assert_eq!(menu, boxed().menu([item("Open", String::from("open"))]));
        let back = boxed().menu([item("Open", String::from("open"))]).on_context(String::from("details"));
        assert_eq!(back, boxed().on_context(String::from("details")));
        let gone = boxed().menu([item("Open", String::from("open"))]).menu(Vec::<Choice<String>>::new());
        assert_eq!(gone, boxed::<String>());
    }

    #[test]
    fn a_mapped_view_wraps_its_menu_items_and_right_click() {
        let part = boxed()
            .on_context(String::from("details"))
            .map(|m: String| alloc::format!("list:{m}"));
        assert_eq!(part, boxed().on_context(String::from("list:details")));
        let part = boxed().menu([item("Open", String::from("open"))]).map(|m: String| alloc::format!("list:{m}"));
        assert_eq!(part, boxed().menu([item("Open", String::from("list:open"))]));
    }
}
