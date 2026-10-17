//! The Rapidkoi demo: a counter and a greeting.
//!
//! Build: `cargo build --release --target wasm32-unknown-unknown` in
//! `guest/rust/`, then `./main guest/rust/target/wasm32-unknown-unknown/release/counter.wasm`.

use minicbor::{Decode, Encode};
use rapidkoi::{button, column, input, row, text, App, Space, View};

#[derive(Encode, Decode)]
struct Counter {
    #[n(0)]
    count: i64,
    #[n(1)]
    name: String,
}

impl App for Counter {
    fn init() -> Self {
        Counter { count: 0, name: String::new() }
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
        let greeting = if self.name.is_empty() {
            text("What's your name?").dim()
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
        ])
        .spacing(Space::Md)
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
            text("What's your name?").dim(),
        ]).spacing(Space::Md));
    }
}
