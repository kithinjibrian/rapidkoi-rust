//! The person's choices (PRP-038): light or dark, more contrast, text size,
//! and what Rapidkoi opens with (Rapidkoi's PRP-075), as Rapidkoi answers
//! [`crate::Ask::preferences`]. Any app may ask, to decide something (a
//! darker picture, a roomier layout); styling with Rapidkoi's names follows
//! them without asking. When one changes, the app gets
//! [`crate::Event::Change`]: ask again.

use minicbor::Decoder;

/// What Rapidkoi opens with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Start {
    /// A new tab.
    #[default]
    NewTab,
    /// The last session, when there's one.
    Restore,
}

/// What the person chose in Rapidkoi's Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prefs {
    /// Light rather than dark.
    pub light: bool,
    /// More contrast.
    pub more_contrast: bool,
    /// The base text size, in percent: 80 to 200.
    pub text_size: u16,
    /// What Rapidkoi opens with; a Rapidkoi that doesn't say: a new tab.
    pub start: Start,
}

impl Prefs {
    /// Reads a preferences answer; `None` if it isn't one.
    pub fn read(cbor: &[u8]) -> Option<Prefs> {
        let mut d = Decoder::new(cbor);
        let n = d.map().ok()??;
        let (mut light, mut more, mut size, mut start) = (None, None, None, Start::NewTab);
        for _ in 0..n {
            match d.str().ok()? {
                "theme" => light = Some(d.str().ok()? == "light"),
                "more_contrast" => more = Some(d.bool().ok()?),
                "text_size" => size = Some(d.u16().ok()?),
                "start" => start = if d.str().ok()? == "restore" { Start::Restore } else { Start::NewTab },
                _ => d.skip().ok()?,
            }
        }
        Some(Prefs { light: light?, more_contrast: more?, text_size: size?, start })
    }
}

/// A setting Rapidkoi's own Settings app may change ([`crate::Ask::set_preference`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preference {
    /// Light (`true`) or dark.
    Light(bool),
    /// More contrast on or off.
    MoreContrast(bool),
    /// The text size, in percent: 80 to 200, in steps of 5 (Rapidkoi's PRP-075; 10 before it).
    TextSize(u16),
    /// What Rapidkoi opens with (contract 11 addition, Rapidkoi's PRP-075).
    Start(Start),
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::Encoder;

    #[test]
    fn an_answer_reads() {
        let mut e = Encoder::new(alloc::vec::Vec::new());
        e.map(4).unwrap();
        e.str("theme").unwrap().str("light").unwrap();
        e.str("more_contrast").unwrap().bool(true).unwrap();
        e.str("text_size").unwrap().u16(150).unwrap();
        e.str("start").unwrap().str("restore").unwrap();
        let read = Prefs::read(&e.into_writer());
        assert_eq!(read, Some(Prefs { light: true, more_contrast: true, text_size: 150, start: Start::Restore }));
        assert_eq!(Prefs::read(&[0x80]), None);
    }

    #[test]
    fn an_answer_without_start_opens_a_new_tab() {
        let mut e = Encoder::new(alloc::vec::Vec::new());
        e.map(3).unwrap();
        e.str("theme").unwrap().str("dark").unwrap();
        e.str("more_contrast").unwrap().bool(false).unwrap();
        e.str("text_size").unwrap().u16(100).unwrap();
        assert_eq!(Prefs::read(&e.into_writer()).map(|p| p.start), Some(Start::NewTab));
    }
}
