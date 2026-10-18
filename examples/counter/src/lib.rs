//! The Rapidkoi demo: a counter and a greeting.
//!
//! Build: `cargo build --release --target wasm32-unknown-unknown` in
//! `guest/rust/`, then `./main guest/rust/target/wasm32-unknown-unknown/release/counter.wasm`.

use minicbor::{Decode, Encode};
use rapidkoi::{button, column, input, row, text, App, Ask, Event, Space, View, FIRST_NAME};

#[derive(Default, Encode, Decode)]
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
    fn handle(self, event: Event) -> (Self, Vec<Ask>) {
        match event {
            // Every open, fresh or resumed: ask Rapidkoi who you are to this app.
            Event::Opened => (self, vec![Ask::identity("me")]),
            Event::Answer { msg, value } if msg == "me" => (Counter { me: value, ..self }, vec![]),
            Event::Message { msg, value } => (
                match msg.as_str() {
                    "add" => Counter { count: self.count + 1, ..self },
                    "reset" => Counter { count: 0, ..self },
                    "name" => Counter { name: value.unwrap_or_default(), ..self },
                    _ => self,
                },
                vec![],
            ),
            // Init (Default is the start), and events this version doesn't use.
            _ => (self, vec![]),
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

    fn message(c: Counter, msg: &str, value: Option<&str>) -> Counter {
        c.handle(Event::Message { msg: msg.into(), value: value.map(String::from) }).0
    }

    #[test]
    fn add_reset_and_name() {
        let c = message(message(Counter::default(), "add", None), "add", None);
        assert_eq!(c.count, 2);
        let c = message(message(c, "name", Some("Amina")), "reset", None);
        assert_eq!((c.count, c.name.as_str()), (0, "Amina"));
    }

    #[test]
    fn reset_is_disabled_at_zero() {
        let v = Counter::default().view();
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
    fn it_asks_who_you_are_on_every_open() {
        let (c, asks) = Counter::default().handle(Event::Opened);
        assert_eq!(asks, vec![Ask::identity("me")]);
        let (c, _) = c.handle(Event::Answer { msg: "me".into(), value: Some(vec![0x4f, 0x9a, 1, 2]) });
        assert_eq!(label(c.me.as_deref().unwrap()), "User 4f9a");
        // Resumed later: it asks again.
        assert_eq!(c.handle(Event::Opened).1, vec![Ask::identity("me")]);
    }
}
