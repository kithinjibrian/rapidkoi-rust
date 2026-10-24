//! Contract 5's styling, shown (PRP-037): a button in the app's own green
//! with round corners, one in Rapidkoi's accent, and a pressable card built
//! from a bare box. Press any of them to count.
//!
//! Build: `cargo build --release --target wasm32-unknown-unknown` in
//! this repository's root, then open `target/wasm32-unknown-unknown/release/styled.wasm` in Rapidkoi.

use rapidkoi::raw::{boxed, Role};
use rapidkoi::style::{Background, Border, Color, Corner, Font, Shadow, Space, Spacing, State, Text, TextSize, Weight, When};
use rapidkoi::{button, column, row, text, App, Event, Message, Next, State as AppState, View};

/// What its screen sends: any press counts.
#[derive(Debug, Clone, PartialEq, Message)]
enum Msg {
    Press,
}

#[derive(Default, AppState)]
struct Styled {
    presses: i64,
}

/// The app's own green: raw, so it stays green whatever the person's theme.
fn green() -> Color {
    Color::rgb(0x2e, 0x7d, 0x32)
}

impl App for Styled {
    type Msg = Msg;

    fn handle(self, event: Event<Msg>) -> Next<Self> {
        match event {
            Event::Message { .. } => Next::state(Styled { presses: self.presses + 1 }),
            _ => Next::state(self),
        }
    }

    fn view(&self) -> View<Msg> {
        let card = boxed()
            .role(Role::Button)
            .on_press(Msg::Press)
            .style([
                Spacing::padding(Space::Lg),
                Background::color(Color::system("surface")),
                Border::all(1, Color::system("border")),
                Border::corner(Corner::Large),
                Shadow::md(),
                When(State::Hover, [Border::all(1, Color::system("accent"))]),
            ])
            .children([
                text("A card").style([Font::size(TextSize::Heading), Font::weight(Weight::Bold)]),
                text("Built from a bare box, in Rapidkoi's colours").dim(),
            ]);
        column([
            text("Styling").title(),
            text(&format!("Pressed {} times", self.presses)),
            row([
                // The library's button, restyled: its own green, round corners.
                button("Mine")
                    .on_press(Msg::Press)
                    .style([
                        Background::color(green()),
                        Border::corner(Corner::Round),
                        When(State::Hover, [Background::color(green().mix(Color::rgb(0, 0, 0), 0.2))]),
                    ])
                    .restyle_text(Text::color(Color::rgb(0xff, 0xff, 0xff))),
                // Rapidkoi's accent: follows the theme.
                button("Rapidkoi's").primary().on_press(Msg::Press),
            ])
            .spacing(Space::Sm),
            card,
        ])
        .spacing(Space::Md)
        .padding(Space::Lg)
        .screen_title("Styling")
    }
}

rapidkoi::export!(Styled);
