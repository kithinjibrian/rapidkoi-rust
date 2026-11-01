//! What the new tab reads (contract 3): the person's pins and where they
//! left off, as Rapidkoi answers [`crate::Ask::pins`] and
//! [`crate::Ask::left_off`], and their last session. Only an app built into
//! Rapidkoi may ask. Every app has an icon (contract 11): a place names it as
//! a source for [`crate::image`], which Rapidkoi draws; the app never holds it.

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::Decoder;

/// One app the person chose: a pin, or where they left off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The app's name.
    pub name: String,
    /// Where it lives: a site's host, or a local app's file name.
    pub place: String,
    /// Its icon, as a source to show: `image(&place.icon)` (contract 11).
    /// Rapidkoi draws the app's own icon, or its initial when it has none.
    pub icon: String,
    /// The screen it was showing, for where you left off.
    pub title: Option<String>,
}

impl Place {
    /// Reads a pins answer; `None` if it isn't one.
    pub fn list(cbor: &[u8]) -> Option<Vec<Place>> {
        let mut d = Decoder::new(cbor);
        let n = d.array().ok()??;
        let mut out = Vec::new();
        for _ in 0..n {
            out.push(read(&mut d)?);
        }
        Some(out)
    }

    /// Reads a where-you-left-off answer; `None` if it isn't one.
    pub fn one(cbor: &[u8]) -> Option<Place> {
        read(&mut Decoder::new(cbor))
    }
}

/// The person's last session, as Rapidkoi answers [`crate::Ask::session`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// How many windows it had.
    pub count: u32,
    /// The apps they were of, each once, in the order of their first window (contract 11).
    pub apps: Vec<Place>,
}

impl Session {
    /// Reads a session answer; `None` if it isn't one.
    pub fn read(cbor: &[u8]) -> Option<Session> {
        let mut d = Decoder::new(cbor);
        let n = d.map().ok()??;
        let (mut count, mut apps) = (None, Vec::new());
        for _ in 0..n {
            match d.str().ok()? {
                "count" => count = Some(d.u32().ok()?),
                "apps" => {
                    let k = d.array().ok()??;
                    for _ in 0..k {
                        apps.push(read(&mut d)?);
                    }
                }
                _ => d.skip().ok()?,
            }
        }
        Some(Session { count: count?, apps })
    }
}

/// One of the person's recent places, as Rapidkoi answers [`crate::Ask::recent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentPlace {
    /// Its app's name.
    pub name: String,
    /// Where its app lives: a site's host, or a local app's file name.
    pub place: String,
    /// Its app's icon, as a source to show: `image(&p.icon)`.
    pub icon: String,
    /// Its address, as the address box shows it: a site's link with its fields, or a file's name.
    pub address: String,
    /// The screen's title it last had, if any.
    pub title: Option<String>,
    /// When it was last open, in milliseconds since 1970: show it with
    /// [`crate::time`]`(seen).relative()`, so Rapidkoi keeps it current.
    pub seen: i64,
    /// Its app's primary colour, `#rrggbb`: the one it names under
    /// `[colors]`, else the one Rapidkoi gives it.
    pub primary_color: String,
}

/// The person's recent places, newest first.
pub struct Recent;

impl Recent {
    /// Reads a recent answer; `None` if it isn't one.
    pub fn read(cbor: &[u8]) -> Option<Vec<RecentPlace>> {
        let mut d = Decoder::new(cbor);
        let n = d.map().ok()??;
        let mut out = Vec::new();
        for _ in 0..n {
            match d.str().ok()? {
                "places" => {
                    let k = d.array().ok()??;
                    for _ in 0..k {
                        out.push(read_recent(&mut d)?);
                    }
                }
                _ => d.skip().ok()?,
            }
        }
        Some(out)
    }
}

fn read_recent(d: &mut Decoder) -> Option<RecentPlace> {
    let n = d.map().ok()??;
    let (mut name, mut place, mut icon, mut address, mut title, mut seen) = (None, None, None, None, None, None);
    let mut primary = None;
    for _ in 0..n {
        match d.str().ok()? {
            "name" => name = Some(String::from(d.str().ok()?)),
            "place" => place = Some(String::from(d.str().ok()?)),
            "icon" => icon = Some(String::from(d.str().ok()?)),
            "address" => address = Some(String::from(d.str().ok()?)),
            "title" => title = Some(String::from(d.str().ok()?)),
            "seen" => seen = Some(d.i64().ok()?),
            "colors" => {
                let k = d.map().ok()??;
                for _ in 0..k {
                    match d.str().ok()? {
                        "primary" => primary = Some(String::from(d.str().ok()?)),
                        _ => d.skip().ok()?,
                    }
                }
            }
            _ => d.skip().ok()?,
        }
    }
    let primary_color = primary?;
    Some(RecentPlace { name: name?, place: place?, icon: icon?, address: address?, title, seen: seen?, primary_color })
}

fn read(d: &mut Decoder) -> Option<Place> {
    let n = d.map().ok()??;
    let (mut name, mut place, mut icon, mut title) = (None, None, None, None);
    for _ in 0..n {
        match d.str().ok()? {
            "name" => name = Some(String::from(d.str().ok()?)),
            "place" => place = Some(String::from(d.str().ok()?)),
            "icon" => icon = Some(String::from(d.str().ok()?)),
            "title" => title = Some(String::from(d.str().ok()?)),
            _ => d.skip().ok()?,
        }
    }
    Some(Place { name: name?, place: place?, icon: icon?, title })
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::Encoder;

    fn place(e: &mut Encoder<Vec<u8>>, name: &str, place: &str, title: Option<&str>) {
        e.map(if title.is_some() { 4 } else { 3 }).unwrap();
        e.str("name").unwrap().str(name).unwrap();
        e.str("place").unwrap().str(place).unwrap();
        e.str("icon").unwrap().str(&alloc::format!("icon:{place}")).unwrap();
        if let Some(t) = title {
            e.str("title").unwrap().str(t).unwrap();
        }
    }

    #[test]
    fn pins_and_where_you_left_off_read() {
        let mut e = Encoder::new(Vec::new());
        e.array(2).unwrap();
        place(&mut e, "Quote", "quote.dafifi.net", None);
        place(&mut e, "Counter", "counter.rkb", None);
        let pins = Place::list(&e.into_writer()).unwrap();
        assert_eq!(pins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Quote", "Counter"]);
        assert_eq!(pins[0].place, "quote.dafifi.net");
        // Contract 11: the icon is a source to show, never its bytes.
        assert_eq!(pins[0].icon, "icon:quote.dafifi.net");
        let mut e = Encoder::new(Vec::new());
        place(&mut e, "Quote", "quote.dafifi.net", Some("Quote of the day"));
        assert_eq!(Place::one(&e.into_writer()).unwrap().title.as_deref(), Some("Quote of the day"));
    }

    #[test]
    fn a_session_reads() {
        let mut e = Encoder::new(Vec::new());
        e.map(2).unwrap();
        e.str("count").unwrap().u32(3).unwrap();
        e.str("apps").unwrap().array(1).unwrap();
        place(&mut e, "Quote", "quote.dafifi.net", None);
        let quote = Place {
            name: "Quote".into(),
            place: "quote.dafifi.net".into(),
            icon: "icon:quote.dafifi.net".into(),
            title: None,
        };
        assert_eq!(Session::read(&e.into_writer()), Some(Session { count: 3, apps: vec![quote] }));
        assert_eq!(Session::read(&[0x80]), None);
    }

    #[test]
    fn recent_places_read_newest_first() {
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("places").unwrap().array(1).unwrap();
        e.map(7).unwrap();
        e.str("name").unwrap().str("Notes").unwrap();
        e.str("place").unwrap().str("notes.dafifi.net").unwrap();
        e.str("icon").unwrap().str("icon:notes.dafifi.net").unwrap();
        e.str("address").unwrap().str("rapidkoi://notes.dafifi.net/@0.9.0?note=3").unwrap();
        e.str("title").unwrap().str("Shopping").unwrap();
        e.str("seen").unwrap().i64(1_793_450_000_000).unwrap();
        e.str("colors").unwrap().map(1).unwrap().str("primary").unwrap().str("#14b8a6").unwrap();
        let places = Recent::read(&e.into_writer()).unwrap();
        assert_eq!(
            places,
            vec![RecentPlace {
                name: "Notes".into(),
                place: "notes.dafifi.net".into(),
                icon: "icon:notes.dafifi.net".into(),
                address: "rapidkoi://notes.dafifi.net/@0.9.0?note=3".into(),
                title: Some("Shopping".into()),
                seen: 1_793_450_000_000,
                primary_color: "#14b8a6".into(),
            }]
        );
        assert_eq!(Recent::read(&[0x80]), None);
    }

    #[test]
    fn anything_else_is_none() {
        assert_eq!(Place::list(&[0xa0]), None);
        assert_eq!(Place::one(&[0x80]), None);
        // A place without its place.
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("name").unwrap().str("x").unwrap();
        assert_eq!(Place::one(&e.into_writer()), None);
        // Or without its icon, which Rapidkoi always names (contract 11).
        let mut e = Encoder::new(Vec::new());
        e.map(2).unwrap().str("name").unwrap().str("x").unwrap().str("place").unwrap().str("x.rkb").unwrap();
        assert_eq!(Place::one(&e.into_writer()), None);
    }
}
