//! The Rapidkoi demo: a counter and a greeting.
//!
//! Build: `cargo build --release --target wasm32-unknown-unknown` in
//! `guest/rust/`, then `./main guest/rust/target/wasm32-unknown-unknown/release/counter.wasm`.

use minicbor::{Decode, Encode};
use rapidkoi::{button, column, input, row, text, App, Ask, Space, View, FIRST_NAME};

#[derive(Encode, Decode)]
struct Counter {
    #[n(0)]
    count: i64,
    #[n(1)]
    name: String,
    /// Its own key for you, from Rapidkoi (contract v2); none until it answers, or when not signed in.
    #[n(2)]
    me: Option<Vec<u8>>,
}

/// "User " and a key's first four hex digits: the label Rapidkoi shows too.
fn label(key: &[u8]) -> String {
    let mut s = String::from("User ");
    for b in key.iter().take(2) {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

impl App for Counter {
    fn init() -> Self {
        Counter { count: 0, name: String::new(), me: None }
    }

    // Ask Rapidkoi who you are to this app, as soon as it starts.
    fn start() -> (Self, Vec<Ask>) {
        (Self::init(), vec![Ask::identity("me")])
    }

    fn answer(self, msg: &str, value: Option<&[u8]>) -> (Self, Vec<Ask>) {
        match msg {
            "me" => (Counter { me: value.map(|k| k.to_vec()), ..self }, Vec::new()),
            _ => (self, Vec::new()),
        }
    }

    fn update(self, msg: &str, value: Option<&str>) -> Self {
        match msg {
            "add" => Counter { count: self.count + 1, ..self },
            "reset" => Counter { count: 0, ..self },
            "name" => Counter { name: value.unwrap_or("").to_string(), ..self },
            _ => self,
        }
    }

    fn view(&self) -> View {
        // With no name typed here, Rapidkoi fills in the one you gave it: the
        // app shows it without ever learning it.
        let greeting = if self.name.is_empty() {
            text(&format!("Hello, {}!", FIRST_NAME))
        } else {
            text(&format!("Hello, {}!", self.name))
        };
        column([
            text("Counter").title(),
            text(&format!("Count: {}", self.count)),
            row([
                button("Add").primary().on_press("add"),
                button("Reset").on_press_if(self.count != 0, "reset"),
            ])
            .spacing(Space::Sm),
            input(&self.name).placeholder("Your name").on_change("name"),
            greeting,
            text(&match &self.me {
                Some(k) => format!("You are {}", label(k)),
                None => String::from("Not signed in"),
            })
            .dim(),
        ])
        .spacing(Space::Md)
        .screen_title("Counter")
    }
}

rapidkoi::export!(Counter);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_reset_and_name() {
        let c = Counter::init().update("add", None).update("add", None);
        assert_eq!(c.count, 2);
        let c = c.update("name", Some("Amina")).update("reset", None);
        assert_eq!((c.count, c.name.as_str()), (0, "Amina"));
    }

    #[test]
    fn reset_is_disabled_at_zero() {
        let v = Counter::init().view();
        assert_eq!(v, column([
            text("Counter").title(),
            text("Count: 0"),
            row([button("Add").primary().on_press("add"), button("Reset")]).spacing(Space::Sm),
            input("").placeholder("Your name").on_change("name"),
            text("Hello, {FIRST_NAME}!"),
            text("Not signed in").dim(),
        ]).spacing(Space::Md).screen_title("Counter"));
    }

    #[test]
    fn it_asks_who_you_are_and_shows_the_answer() {
        let (c, asks) = Counter::start();
        assert_eq!(asks, vec![Ask::identity("me")]);
        let (c, _) = c.answer("me", Some(&[0x4f, 0x9a, 1, 2]));
        assert_eq!(label(c.me.as_deref().unwrap()), "User 4f9a");
        let (c, _) = c.answer("me", None);
        assert_eq!(c.me, None);
    }
}
