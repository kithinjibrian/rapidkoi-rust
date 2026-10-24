//! Widgets (PRP-037): ready-made pieces built from contract 5's primitives
//! ([`crate::raw`]), in Rapidkoi's look by default. Each is an ordinary
//! [`View`], so `.style([...])` changes any part of its look, and the
//! keyboard and screen-reader facts come with it.
//!
//! ```
//! use rapidkoi::{button, column, text, Message, Space, View};
//!
//! #[derive(Message)]
//! enum Msg { Add }
//!
//! let screen: View<Msg> = column([
//!     text("Counter").title(),
//!     button("Add").primary().on_press(Msg::Add),
//! ])
//! .spacing(Space::Md);
//! ```


use crate::message::Message;
use crate::raw::{boxed, field, Role, View};
use crate::style::{
    Align, Background, Border, Color, Corner, Effects, Font, Layout, Outline, Sizing, Space, Spacing, State, Text,
    TextSize, Weight, When,
};

impl<M: Message> View<M> {
    /// A checkbox or switch: the message a click (or Space) sends.
    pub fn on_toggle(self, msg: M) -> View<M> {
        self.on_press(msg)
    }
}

impl<M> View<M> {
    /// A text: a view's title.
    pub fn title(self) -> View<M> {
        self.style([Font::size(TextSize::Title), Font::weight(Weight::Bold)])
    }

    /// A text: a section heading.
    pub fn heading(self) -> View<M> {
        self.style([Font::size(TextSize::Heading), Font::weight(Weight::Bold)])
    }

    /// A text: secondary.
    pub fn dim(self) -> View<M> {
        self.style([Text::color(Color::system("text_dim"))])
    }

    /// A button: the one main action in a view.
    pub fn primary(self) -> View<M> {
        self.painted("accent", "on_accent")
    }

    /// A button: deletes or discards.
    pub fn danger(self) -> View<M> {
        self.painted("danger", "on_danger")
    }

    /// A button: low emphasis, no fill until hovered.
    pub fn subtle(self) -> View<M> {
        self.style([
            Background::color(Color::rgba(0, 0, 0, 0)),
            When(State::Hover, [Background::color(Color::system("surface"))]),
            When(State::Pressed, [Background::color(Color::system("surface").mix(Color::system("text"), 0.1))]),
        ])
    }

    /// A box: room between its children.
    pub fn spacing(self, s: Space) -> View<M> {
        self.style([Spacing::gap(s)])
    }

    /// A box: room inside its edge.
    pub fn padding(self, s: Space) -> View<M> {
        self.style([Spacing::padding(s)])
    }

    /// A button's fill, its text, and hover and press a step towards the text.
    fn painted(self, fill: &str, ink: &str) -> View<M> {
        self.style([
            Background::color(Color::system(fill)),
            When(State::Hover, [Background::color(Color::system(fill).mix(Color::system("text"), 0.1))]),
            When(State::Pressed, [Background::color(Color::system(fill).mix(Color::system("text"), 0.2))]),
        ])
        .ink(ink)
    }

    /// The colour of the text inside a button: its first child's.
    fn ink(self, colour: &str) -> View<M> {
        self.restyle_first(Text::color(Color::system(colour)))
    }
}

/// A line of text.
pub fn text<M>(s: &str) -> View<M> {
    crate::raw::text(s)
}

/// Children top to bottom, each at its own width unless it stretches.
pub fn column<M>(children: impl IntoIterator<Item = View<M>>) -> View<M> {
    boxed().children(children)
}

/// Children left to right.
pub fn row<M>(children: impl IntoIterator<Item = View<M>>) -> View<M> {
    boxed().style([Layout::row()]).children(children)
}

/// A button labelled `label`; give it [`View::on_press`] to make it work,
/// or it's shown disabled.
pub fn button<M>(label: &str) -> View<M> {
    let mut b = boxed()
        .role(Role::Button)
        .style([
            Layout::align(Align::Start),
            Spacing::padding_xy(Space::Xs, Space::Md),
            Border::corner(Corner::Medium),
            Background::color(Color::system("surface")),
            When(State::Hover, [Background::color(Color::system("surface").mix(Color::system("text"), 0.1))]),
            When(State::Pressed, [Background::color(Color::system("surface").mix(Color::system("text"), 0.2))]),
            When(State::Disabled, [Effects::opacity(0.5)]),
        ])
        .children([text(label).style([Text::color(Color::system("text"))])]);
    b.needs_press = true;
    b
}

/// A one-line typing box showing `value`; give it [`View::on_change`],
/// and [`View::on_submit`] for Enter.
pub fn input<M>(value: &str) -> View<M> {
    field(value)
}

/// A typing box of several lines showing `value`.
pub fn text_area<M>(value: &str) -> View<M> {
    field(value).lines(5)
}

/// A checkbox labelled `label`: a box that ticks, and its words; clicking
/// either, or Space, sends [`View::on_toggle`]'s message.
pub fn checkbox<M>(label: &str, checked: bool) -> View<M> {
    let mark = boxed()
        .style([
            Sizing::width(18),
            Sizing::height(18),
            Border::all(1, Color::system("border")),
            Border::corner(Corner::Small),
            Layout::align(Align::Center),
        ])
        .children(if checked { alloc::vec![text("✓").style([Text::color(Color::system("on_accent"))])] } else { alloc::vec![] });
    let mark = if checked { mark.style([Background::color(Color::system("accent")), Border::all(1, Color::system("accent"))]) } else { mark };
    boxed()
        .role(Role::Checkbox)
        .checked(checked)
        .label(label)
        .style([
            Layout::row(),
            Layout::align(Align::Start),
            Spacing::gap(Space::Sm),
            When(State::Focus, [Outline::ring(1, Color::system("accent"))]),
        ])
        .children([mark, text(label).style([Text::color(Color::system("text"))])])
}

/// Tabs: one per `(key, label)`, `selected` marked. Picking one sends
/// `pick(key)`.
pub fn tabs<M: Message>(items: &[(&str, &str)], selected: &str, pick: impl Fn(&str) -> M) -> View<M> {
    boxed()
        .role(Role::TabList)
        .style([Layout::row(), Spacing::gap(Space::Xs), Layout::align(Align::Start)])
        .children(items.iter().map(|(key, label)| {
            let on = *key == selected;
            let tab = boxed()
                .role(Role::Tab)
                .selected(on)
                .on_press(pick(key))
                .style([
                    Spacing::padding_xy(Space::Xs, Space::Md),
                    Border::corner(Corner::Medium),
                    When(State::Hover, [Background::color(Color::system("surface"))]),
                ])
                .children([text(label).style([Text::color(Color::system(if on { "on_accent" } else { "text" }))])]);
            if on { tab.style([Background::color(Color::system("accent"))]) } else { tab }
        }))
}

/// Room that grows: pushes what follows it in a row to the end.
pub fn spacer<M>() -> View<M> {
    boxed().style([Layout::grow()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use minicbor::Decoder;

    /// The props map's keys, in order.
    fn keys<M>(v: &View<M>) -> alloc::vec::Vec<String> {
        let bytes = v.encode();
        let mut d = Decoder::new(&bytes);
        d.map().unwrap();
        d.str().unwrap();
        d.str().unwrap();
        d.str().unwrap();
        let n = d.map().unwrap().unwrap();
        let mut out = alloc::vec::Vec::new();
        for _ in 0..n {
            out.push(String::from(d.str().unwrap()));
            d.skip().unwrap();
        }
        out
    }

    fn s(x: &str) -> String {
        String::from(x)
    }

    #[test]
    fn a_button_is_a_pressable_box_with_a_role() {
        let k = keys(&button("Add").primary().on_press(s("add")));
        assert_eq!(k, ["role", "on_press", "style"]);
    }

    #[test]
    fn a_button_with_nothing_to_send_is_disabled() {
        let k = keys(&button::<String>("Reset"));
        assert_eq!(k, ["role", "on_press", "disabled", "style"]);
        assert_eq!(button("Reset").on_press_if(false, s("reset")), button("Reset"));
    }

    #[test]
    fn scrolling_styles_say_which_way() {
        // Contract 7 (PRP-046): `scroll` is vertical, horizontal or both.
        use crate::style::Layout;
        let has = |v: &View<String>, word: &str| {
            let bytes = v.encode();
            bytes.windows(word.len()).any(|w| w == word.as_bytes())
        };
        assert!(has(&column::<String>([]).style([Layout::scroll()]), "vertical"));
        assert!(has(&column::<String>([]).style([Layout::scroll_x()]), "horizontal"));
        assert!(has(&column::<String>([]).style([Layout::scroll_both()]), "both"));
        assert!(!has(&column::<String>([]), "scroll"));
    }

    #[test]
    fn a_typing_box_carries_its_label() {
        // Contract 7: Rapidkoi names the box by it.
        let k = keys(&input::<String>("").placeholder("Search").label("Search notes").on_change(s("search")));
        assert!(k.iter().any(|x| x == "label"));
    }

    #[test]
    fn a_checkbox_says_what_it_is() {
        let k = keys(&checkbox("Buy milk", true).on_toggle(s("toggle:3")));
        assert_eq!(k, ["role", "checked", "label", "on_press", "style"]);
    }

    #[test]
    fn restyling_a_widget_keeps_the_rest_of_its_look() {
        let mine = button::<String>("Go").style([Background::color(Color::rgb(0x2e, 0x7d, 0x32))]);
        assert_ne!(mine, button("Go"));
        assert_eq!(keys(&mine), keys(&button::<String>("Go")));
    }

    #[test]
    fn tabs_send_what_pick_makes() {
        let t = tabs(&[("all", "All"), ("open", "Open")], "open", |k| alloc::format!("filter:{k}"));
        let b = t.encode();
        assert!(b.windows(11).any(|w| w == b"filter:open"));
    }
}
