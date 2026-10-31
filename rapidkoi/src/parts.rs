//! Texts made of parts (contract 8, PRP-053): words of the app's own, the
//! person's name and moments in time, put together by Rapidkoi while it
//! draws. The app never learns what's drawn: not the name, not the person's
//! zone, not how "5 minutes ago" came out.
//!
//! ```
//! use rapidkoi::{first_name, text_of, time, View};
//!
//! let updated: i64 = 1_793_450_000_000; // milliseconds since 1970, UTC
//! let hello: View = text_of(["Hello, ".into(), first_name(), "!".into()]);
//! let edited: View = text_of(["Edited ".into(), time(updated).relative()]);
//! ```
//!
//! The time now comes from [`crate::Ask::now`], read with [`Now::read`];
//! SQLite's `unixepoch() * 1000` gives the same shape.

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::Decoder;

use crate::raw::Val;

/// The most parts one text may have.
pub const MAX_PARTS: usize = 64;

/// The first and last moments a part may name: 0001-01-01 00:00 and
/// 9999-12-31 23:59:59.999 UTC, in milliseconds since 1970.
const EARLIEST: i64 = -62_135_596_800_000;
const LATEST: i64 = 253_402_300_799_999;

/// One piece of a text from [`crate::text_of`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    /// Shown as written; braces are only braces.
    Text(String),
    /// The person's first name for this app; nothing when unset.
    FirstName,
    /// The person's last name for this app; nothing when unset.
    LastName,
    /// A moment, in milliseconds since 1970-01-01 00:00 UTC, shown in the person's zone.
    Time(i64, Show),
}

/// How a moment is shown: Rapidkoi's 24-hour clock, day before month.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// "14:32"
    Time,
    /// "Thu 31 Oct" (the year when it isn't this one)
    Date,
    /// "Thu 31 Oct, 14:32"
    DateTime,
    /// "just now", "5 minutes ago", "yesterday", "next Sun"…, kept current by Rapidkoi
    Relative,
    /// "Thursday"
    Weekday,
}

impl Show {
    fn name(self) -> &'static str {
        match self {
            Show::Time => "time",
            Show::Date => "date",
            Show::DateTime => "date_time",
            Show::Relative => "relative",
            Show::Weekday => "weekday",
        }
    }
}

impl From<&str> for Part {
    fn from(s: &str) -> Part {
        Part::Text(String::from(s))
    }
}

impl From<String> for Part {
    fn from(s: String) -> Part {
        Part::Text(s)
    }
}

impl From<&String> for Part {
    fn from(s: &String) -> Part {
        Part::Text(s.clone())
    }
}

impl Part {
    pub(crate) fn val(&self) -> Val {
        let name = |which: &str| Val::Map(alloc::vec![(String::from("name"), Val::Text(String::from(which)))]);
        match self {
            Part::Text(s) => Val::Text(s.clone()),
            Part::FirstName => name("first"),
            Part::LastName => name("last"),
            Part::Time(ms, show) => {
                debug_assert!((EARLIEST..=LATEST).contains(ms), "a time part must fall within years 1 to 9999");
                Val::Map(alloc::vec![
                    (String::from("time"), Val::Int(*ms)),
                    (String::from("show"), Val::Text(String::from(show.name()))),
                ])
            }
        }
    }
}

/// The person's first name for this app, filled in by Rapidkoi.
pub fn first_name() -> Part {
    Part::FirstName
}

/// The person's last name for this app, filled in by Rapidkoi.
pub fn last_name() -> Part {
    Part::LastName
}

/// A moment, `ms` milliseconds since 1970-01-01 00:00 UTC: say how it's
/// shown with [`Time::relative`], [`Time::date`]…
pub fn time(ms: i64) -> Time {
    Time(ms)
}

/// A moment waiting to be told how it's shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time(pub i64);

impl Time {
    /// "14:32"
    pub fn time(self) -> Part {
        Part::Time(self.0, Show::Time)
    }
    /// "Thu 31 Oct"
    pub fn date(self) -> Part {
        Part::Time(self.0, Show::Date)
    }
    /// "Thu 31 Oct, 14:32"
    pub fn date_time(self) -> Part {
        Part::Time(self.0, Show::DateTime)
    }
    /// "5 minutes ago", "yesterday", "next Sun"…
    pub fn relative(self) -> Part {
        Part::Time(self.0, Show::Relative)
    }
    /// "Thursday"
    pub fn weekday(self) -> Part {
        Part::Time(self.0, Show::Weekday)
    }
}

/// The answer to [`crate::Ask::now`].
pub struct Now;

impl Now {
    /// Reads a `now` answer ([`crate::Event::Data`]'s `cbor`): the time in
    /// UTC, in milliseconds since 1970, rounded down to a second.
    pub fn read(cbor: &[u8]) -> Option<i64> {
        let mut d = Decoder::new(cbor);
        let ms = d.i64().ok()?;
        (d.position() == cbor.len()).then_some(ms)
    }
}

pub(crate) fn vals(parts: impl IntoIterator<Item = Part>) -> Vec<Val> {
    let vals: Vec<Val> = parts.into_iter().map(|p| p.val()).collect();
    debug_assert!(vals.len() <= MAX_PARTS, "a text has at most 64 parts");
    vals
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::Encoder;

    #[test]
    fn now_reads_an_integer() {
        let mut e = Encoder::new(Vec::new());
        e.i64(1_793_450_000_000).unwrap();
        assert_eq!(Now::read(&e.into_writer()), Some(1_793_450_000_000));
        assert_eq!(Now::read(&[0x80]), None);
        assert_eq!(Now::read(&[]), None);
    }
}
