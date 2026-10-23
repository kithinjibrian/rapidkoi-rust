//! The Rapidkoi demo: a counter and a greeting.
//!
//! Build: `cargo build --release --target wasm32-unknown-unknown` in
//! `guest/rust/`, then `./main guest/rust/target/wasm32-unknown-unknown/release/counter.wasm`.

use rapidkoi::{button, column, input, row, text, App, Ask, Event, Message, Next, Space, State, View, FIRST_NAME};

#[derive(Default, State)]
struct Counter {
    /// In the window's address: a link to it opens a new window at this count.
    count: i64,
    name: String,
    /// Its own key for you, from Rapidkoi; none until it answers, or when not signed in.
    me: Option<Vec<u8>>,
}

/// What the counter's screen sends.
#[derive(Debug, Clone, PartialEq, Message)]
enum Msg {
    Add,
    Reset,
    /// The name box changed: the text comes as the event's `value`.
    Name,
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
    type Msg = Msg;

    fn handle(self, event: Event<Msg>) -> Next<Self> {
        match event {
            // A fresh start (Default is the start): the count goes in the address.
            Event::Init => Next::state(self).link(Counter::count),
            // Every open, fresh, resumed or from a link: ask Rapidkoi who you are to this app.
            Event::Opened => Next::state(self).ask(Ask::identity("me")),
            Event::Answer { msg, value } if msg == "me" => Next::state(Counter { me: value, ..self }),
            Event::Message { msg, value } => match msg {
                Msg::Add => Next::state(Counter { count: self.count + 1, ..self }).link(Counter::count),
                Msg::Reset => Next::state(Counter { count: 0, ..self }).link(Counter::count),
                Msg::Name => Next::state(Counter { name: value.unwrap_or_default(), ..self }),
            },
            // Events this version doesn't use.
            _ => Next::state(self),
        }
    }

    fn view(&self) -> View<Msg> {
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
                button("Add").primary().on_press(Msg::Add),
                button("Reset").on_press_if(self.count != 0, Msg::Reset),
            ])
            .spacing(Space::Sm),
            input(&self.name).placeholder("Your name").on_change(Msg::Name),
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

    fn message(c: Counter, msg: Msg, value: Option<&str>) -> Next<Counter> {
        c.handle(Event::Message { msg, value: value.map(String::from) })
    }

    #[test]
    fn add_reset_and_name() {
        let c = message(message(Counter::default(), Msg::Add, None).into_state(), Msg::Add, None).into_state();
        assert_eq!(c.count, 2);
        let c = message(message(c, Msg::Name, Some("Amina")).into_state(), Msg::Reset, None).into_state();
        assert_eq!((c.count, c.name.as_str()), (0, "Amina"));
    }

    #[test]
    fn the_address_follows_the_count() {
        // PRP-036: the count goes in the address from the start, and stays there.
        assert_eq!(Counter::default().handle(Event::Init).linked(), Some(vec![String::from("count")]));
        assert_eq!(message(Counter::default(), Msg::Add, None).linked(), Some(vec![String::from("count")]));
        assert_eq!(message(Counter::default(), Msg::Name, Some("Amina")).linked(), None);
    }

    #[test]
    fn reset_is_disabled_at_zero() {
        let v = Counter::default().view();
        assert_eq!(v, column([
            text("Counter").title(),
            text("Count: 0"),
            row([button("Add").primary().on_press(Msg::Add), button("Reset")]).spacing(Space::Sm),
            input("").placeholder("Your name").on_change(Msg::Name),
            text("Hello, {FIRST_NAME}!"),
            text("Not signed in").dim(),
        ]).spacing(Space::Md).screen_title("Counter"));
    }

    #[test]
    fn it_asks_who_you_are_on_every_open() {
        let opened = Counter::default().handle(Event::Opened);
        assert_eq!(opened.asks(), &[Ask::identity("me")]);
        let c = opened.into_state().handle(Event::Answer { msg: "me".into(), value: Some(vec![0x4f, 0x9a, 1, 2]) }).into_state();
        assert_eq!(label(c.me.as_deref().unwrap()), "User 4f9a");
        // Resumed later: it asks again.
        assert_eq!(c.handle(Event::Opened).asks(), &[Ask::identity("me")]);
    }
}
