//! Screen descriptions: a builder for contract v1's components, and their
//! CBOR encoding. Apps say what is on screen, never how it looks: styles and
//! sizes are meaning words that Rapidkoi maps onto the person's theme.

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::Encoder;

/// How much room: a contract spacing or padding word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    None,
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

impl Space {
    fn word(self) -> &'static str {
        match self {
            Space::None => "none",
            Space::Xs => "xs",
            Space::Sm => "sm",
            Space::Md => "md",
            Space::Lg => "lg",
            Space::Xl => "xl",
        }
    }
}

/// One node of a screen description. Build them with [`text`], [`button`],
/// [`input`], [`column`] and [`row`].
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    kind: &'static str,
    props: Vec<(&'static str, String)>,
    children: Option<Vec<View>>,
}

impl View {
    fn new(kind: &'static str) -> View {
        View { kind, props: Vec::new(), children: None }
    }

    fn prop(mut self, name: &'static str, value: &str) -> View {
        self.props.retain(|(n, _)| *n != name);
        self.props.push((name, String::from(value)));
        self
    }

    /// A text: styled as a view's title.
    pub fn title(self) -> View {
        self.prop("style", "title")
    }

    /// The screen's title, on the root node: Rapidkoi shows it in the tab,
    /// the window title and "Where you left off".
    pub fn screen_title(self, title: &str) -> View {
        self.prop("title", title)
    }

    /// A text: a section heading.
    pub fn heading(self) -> View {
        self.prop("style", "heading")
    }

    /// A text: secondary.
    pub fn dim(self) -> View {
        self.prop("style", "dim")
    }

    /// A button: the one main action in a view.
    pub fn primary(self) -> View {
        self.prop("style", "primary")
    }

    /// A button: deletes or discards.
    pub fn danger(self) -> View {
        self.prop("style", "danger")
    }

    /// A button: low emphasis.
    pub fn subtle(self) -> View {
        self.prop("style", "subtle")
    }

    /// A button: the message a press sends. Without one it is disabled.
    pub fn on_press(self, msg: &str) -> View {
        self.prop("on_press", msg)
    }

    /// A button: sends `msg` when `enabled`, else is disabled.
    pub fn on_press_if(self, enabled: bool, msg: &str) -> View {
        if enabled { self.on_press(msg) } else { self }
    }

    /// An input: the hint shown while it's empty.
    pub fn placeholder(self, hint: &str) -> View {
        self.prop("placeholder", hint)
    }

    /// An input: the message each edit sends; `update` gets the new text as `value`.
    pub fn on_change(self, msg: &str) -> View {
        self.prop("on_change", msg)
    }

    /// A column or row: room between its children.
    pub fn spacing(self, s: Space) -> View {
        self.prop("spacing", s.word())
    }

    /// A column or row: room around it.
    pub fn padding(self, s: Space) -> View {
        self.prop("padding", s.word())
    }

    /// Encodes this tree as contract v1 CBOR.
    pub fn encode(&self) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        self.write(&mut e);
        e.into_writer()
    }

    fn write(&self, e: &mut Encoder<Vec<u8>>) {
        // Writing to a Vec never fails, so the results are ignored.
        let fields = if self.children.is_some() { 3 } else { 2 };
        let _ = e.map(fields);
        let _ = e.str("kind").and_then(|e| e.str(self.kind));
        let _ = e.str("props").and_then(|e| e.map(self.props.len() as u64));
        for (name, value) in &self.props {
            let _ = e.str(name).and_then(|e| e.str(value));
        }
        if let Some(kids) = &self.children {
            let _ = e.str("children").and_then(|e| e.array(kids.len() as u64));
            for k in kids {
                k.write(e);
            }
        }
    }
}

/// A line of text.
pub fn text(s: &str) -> View {
    View::new("text").prop("text", s)
}

/// A button labelled `label`; give it [`View::on_press`] to make it work.
pub fn button(label: &str) -> View {
    View::new("button").prop("label", label)
}

/// A one-line text field showing `value`; give it [`View::on_change`].
pub fn input(value: &str) -> View {
    View::new("input").prop("value", value)
}

/// Children top to bottom.
pub fn column(children: impl IntoIterator<Item = View>) -> View {
    View { children: Some(children.into_iter().collect()), ..View::new("column") }
}

/// Children left to right.
pub fn row(children: impl IntoIterator<Item = View>) -> View {
    View { children: Some(children.into_iter().collect()), ..View::new("row") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| alloc::format!("{b:02x}")).collect()
    }

    #[test]
    fn a_text_encodes_exactly() {
        // {"kind": "text", "props": {"text": "hi"}}
        let expected = [
            "a2",                     // map, 2 entries
            "64", "6b696e64",         // "kind"
            "64", "74657874",         // "text"
            "65", "70726f7073",       // "props"
            "a1",                     // map, 1 entry
            "64", "74657874",         // "text"
            "62", "6869",             // "hi"
        ]
        .concat();
        assert_eq!(hex(&text("hi").encode()), expected);
    }

    #[test]
    fn a_column_carries_its_children_and_words() {
        // {"kind": "column", "props": {"spacing": "md"}, "children": [<text "a">]}
        let expected = [
            "a3",
            "64", "6b696e64", "66", "636f6c756d6e",               // "kind": "column"
            "65", "70726f7073", "a1", "67", "73706163696e67", "62", "6d64", // "props": {"spacing": "md"}
            "68", "6368696c6472656e", "81",                       // "children": [
            "a2", "64", "6b696e64", "64", "74657874",
            "65", "70726f7073", "a1", "64", "74657874", "61", "61", // {"kind":"text","props":{"text":"a"}}
        ]
        .concat();
        assert_eq!(hex(&column([text("a")]).spacing(Space::Md).encode()), expected);
    }

    #[test]
    fn a_screen_title_is_a_root_prop() {
        assert_eq!(column([]).screen_title("Inbox"), View { props: alloc::vec![("title", String::from("Inbox"))], ..column([]) });
    }

    #[test]
    fn a_disabled_button_has_no_message() {
        assert_eq!(button("Reset").on_press_if(false, "reset"), button("Reset"));
        assert_eq!(button("Reset").on_press_if(true, "reset"), button("Reset").on_press("reset"));
    }
}
