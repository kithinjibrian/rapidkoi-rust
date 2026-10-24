//! Styling (PRP-037): one door, [`crate::View::style`], taking typed values
//! grouped as kivuli groups them. Colours, sizes and spacing can be
//! Rapidkoi's names (`Color::system("accent")`, `TextSize::Title`,
//! `Space::Md`), which follow the person's theme and settings without the
//! app ever learning their values, or raw ones, which stay as written.
//!
//! ```
//! use rapidkoi::raw::boxed;
//! use rapidkoi::style::{Background, Border, Color, Corner, State, When};
//! use rapidkoi::View;
//!
//! // A box that sends nothing: its message type is the default, `String`.
//! let card: View = boxed().style([
//!     Background::color(Color::system("surface")),
//!     Border::corner(Corner::Large),
//!     When(State::Hover, [Background::color(Color::system("surface").mix(Color::system("text"), 0.1))]),
//! ]);
//! ```

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::raw::Val;

/// One style value, for [`crate::View::style`].
#[derive(Clone, Debug, PartialEq)]
pub struct Style(pub(crate) Item);

/// A style value's insides: a key and its value, or one state's look.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Item {
    Set(String, Val),
    When(State, Vec<Style>),
}

fn set(key: &str, value: Val) -> Style {
    Style(Item::Set(String::from(key), value))
}

/// A state a box can be in, each with its own look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// The pointer is over it.
    Hover,
    /// It's being pressed.
    Pressed,
    /// It has the keyboard.
    Focus,
    /// It sends nothing.
    Disabled,
    /// A checkbox or switch that's on.
    Checked,
}

impl State {
    pub(crate) fn name(self) -> &'static str {
        match self {
            State::Hover => "hover",
            State::Pressed => "pressed",
            State::Focus => "focus",
            State::Disabled => "disabled",
            State::Checked => "checked",
        }
    }
}

/// A look for one state: `When(State::Hover, [Background::color(…)])`.
#[allow(non_snake_case)]
pub fn When(state: State, items: impl IntoIterator<Item = Style>) -> Style {
    Style(Item::When(state, items.into_iter().collect()))
}

/// A colour: one of Rapidkoi's, the app's own, or a mix.
#[derive(Clone, Debug, PartialEq)]
pub struct Color(Val);

impl Color {
    /// One of Rapidkoi's colours: `background`, `surface`, `text`,
    /// `text_dim`, `accent`, `on_accent`, `border`, `danger`, `on_danger`.
    /// It follows the person's theme; the app never learns its value.
    pub fn system(name: &str) -> Color {
        Color(Val::Text(String::from(name)))
    }

    /// The app's own colour, opaque.
    pub fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color(Val::Text(format!("#{r:02x}{g:02x}{b:02x}")))
    }

    /// The app's own colour, with transparency (`a` 255 is opaque).
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color(Val::Text(format!("#{r:02x}{g:02x}{b:02x}{a:02x}")))
    }

    /// This colour moved towards `other` by `amount` (0 to 1): a hover a
    /// little darker than the accent, whatever the accent is.
    pub fn mix(self, other: Color, amount: f64) -> Color {
        let mut entries = Vec::new();
        entries.push((String::from("mix"), Val::Array(alloc::vec![self.0, other.0, Val::Float(amount.clamp(0.0, 1.0))])));
        Color(Val::Map(entries))
    }
}

/// A gap or padding: one of Rapidkoi's spacing sizes, or pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    None,
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
    Px(u16),
}

impl Space {
    pub(crate) fn val(self) -> Val {
        match self {
            Space::None => Val::Int(0),
            Space::Xs => Val::Text(String::from("xs")),
            Space::Sm => Val::Text(String::from("sm")),
            Space::Md => Val::Text(String::from("md")),
            Space::Lg => Val::Text(String::from("lg")),
            Space::Xl => Val::Text(String::from("xl")),
            Space::Px(n) => Val::Int(n.min(256) as i64),
        }
    }
}

/// Corners: Rapidkoi's sizes, or pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    Small,
    Medium,
    Large,
    Round,
    Px(u16),
}

/// A text size: Rapidkoi's (follow the person's text size), a multiple of
/// the body size (follows it too), or points (stays).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextSize {
    Title,
    Heading,
    Body,
    Small,
    Rem(f64),
    Pt(u8),
}

/// How heavy text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Normal,
    Medium,
    Bold,
}

/// A typeface: the body's, monospaced, serif or sans-serif.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Body,
    Mono,
    Serif,
    Sans,
}

/// Where a node sits across its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
}

fn word(s: &str) -> Val {
    Val::Text(String::from(s))
}

fn line(width: u8, color: Color) -> Val {
    Val::Map(alloc::vec![(String::from("width"), Val::Int(width as i64)), (String::from("color"), color.0)])
}

/// Behind a box.
pub struct Background;

impl Background {
    pub fn color(c: Color) -> Style {
        set("background", c.0)
    }
}

/// Text's own look.
pub struct Text;

impl Text {
    pub fn color(c: Color) -> Style {
        set("color", c.0)
    }
}

/// Text's typeface, size and weight.
pub struct Font;

impl Font {
    pub fn size(s: TextSize) -> Style {
        set(
            "size",
            match s {
                TextSize::Title => word("title"),
                TextSize::Heading => word("heading"),
                TextSize::Body => word("body"),
                TextSize::Small => word("small"),
                TextSize::Rem(x) => Val::Map(alloc::vec![(String::from("rem"), Val::Float(x.clamp(0.5, 6.0)))]),
                TextSize::Pt(n) => Val::Map(alloc::vec![(String::from("pt"), Val::Int(n.clamp(6, 72) as i64))]),
            },
        )
    }

    pub fn weight(w: Weight) -> Style {
        set(
            "weight",
            word(match w {
                Weight::Normal => "normal",
                Weight::Medium => "medium",
                Weight::Bold => "bold",
            }),
        )
    }

    pub fn family(f: Family) -> Style {
        set(
            "font",
            word(match f {
                Family::Body => "body",
                Family::Mono => "mono",
                Family::Serif => "serif",
                Family::Sans => "sans",
            }),
        )
    }

    pub fn italic() -> Style {
        set("italic", Val::Bool(true))
    }
}

/// A box's edge and corners.
pub struct Border;

impl Border {
    /// A line all round, `width` pixels (at most 16).
    pub fn all(width: u8, c: Color) -> Style {
        set("border", line(width.min(16), c))
    }

    pub fn corner(c: Corner) -> Style {
        set(
            "corner",
            match c {
                Corner::Small => word("small"),
                Corner::Medium => word("medium"),
                Corner::Large => word("large"),
                Corner::Round => word("round"),
                Corner::Px(n) => Val::Int(n.min(256) as i64),
            },
        )
    }
}

/// A ring outside a box that takes no room.
pub struct Outline;

impl Outline {
    pub fn ring(width: u8, c: Color) -> Style {
        set("outline", line(width.min(8), c))
    }
}

/// Room inside a box and between its children.
pub struct Spacing;

impl Spacing {
    pub fn padding(s: Space) -> Style {
        set("padding", s.val())
    }

    /// Room above and below, and either side.
    pub fn padding_xy(vertical: Space, horizontal: Space) -> Style {
        set("padding", Val::Array(alloc::vec![vertical.val(), horizontal.val()]))
    }

    pub fn gap(s: Space) -> Style {
        set("gap", s.val())
    }
}

/// A box's size, in pixels (at most 4096).
pub struct Sizing;

impl Sizing {
    pub fn width(px: u16) -> Style {
        set("width", Val::Int(px.min(4096) as i64))
    }
    pub fn height(px: u16) -> Style {
        set("height", Val::Int(px.min(4096) as i64))
    }
    pub fn min_width(px: u16) -> Style {
        set("min_width", Val::Int(px.min(4096) as i64))
    }
    pub fn max_width(px: u16) -> Style {
        set("max_width", Val::Int(px.min(4096) as i64))
    }
    pub fn min_height(px: u16) -> Style {
        set("min_height", Val::Int(px.min(4096) as i64))
    }
    pub fn max_height(px: u16) -> Style {
        set("max_height", Val::Int(px.min(4096) as i64))
    }
}

/// A box's shadow.
pub struct Shadow;

impl Shadow {
    pub fn none() -> Style {
        set("shadow", word("none"))
    }
    pub fn sm() -> Style {
        set("shadow", word("sm"))
    }
    pub fn md() -> Style {
        set("shadow", word("md"))
    }
    pub fn lg() -> Style {
        set("shadow", word("lg"))
    }
}

/// See-through.
pub struct Effects;

impl Effects {
    /// 0 is invisible, 1 solid.
    pub fn opacity(x: f64) -> Style {
        set("opacity", Val::Float(x.clamp(0.0, 1.0)))
    }
}

/// How a box lays out its children, and where it sits in its parent.
pub struct Layout;

impl Layout {
    /// Children left to right (a box is a column otherwise).
    pub fn row() -> Style {
        set("direction", word("row"))
    }
    /// Children top to bottom.
    pub fn column() -> Style {
        set("direction", word("column"))
    }
    pub fn align(a: Align) -> Style {
        set(
            "align",
            word(match a {
                Align::Start => "start",
                Align::Center => "center",
                Align::End => "end",
                Align::Stretch => "stretch",
            }),
        )
    }
    /// Take the spare room along the parent's direction.
    pub fn grow() -> Style {
        set("grow", Val::Bool(true))
    }
    /// Scroll up and down: as tall as its content until the room runs out,
    /// then it scrolls; with [`Layout::grow`], it fills the room left.
    /// Rapidkoi never scrolls an app's page, so whatever can grow past the
    /// window goes in one of these. Not inside another that scrolls the same way.
    pub fn scroll() -> Style {
        set("scroll", word("vertical"))
    }
    /// Scroll side to side, as [`Layout::scroll`] does up and down.
    pub fn scroll_x() -> Style {
        set("scroll", word("horizontal"))
    }
    /// Scroll both ways.
    pub fn scroll_both() -> Style {
        set("scroll", word("both"))
    }
}
