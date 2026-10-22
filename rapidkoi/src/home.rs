//! What the new tab reads (contract 3): the person's pins and where they
//! left off, as Rapidkoi answers [`crate::Ask::pins`] and
//! [`crate::Ask::left_off`]. Only an app built into Rapidkoi may ask.

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
    /// Its icon (PNG), when it has one.
    pub icon: Option<Vec<u8>>,
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

fn read(d: &mut Decoder) -> Option<Place> {
    let n = d.map().ok()??;
    let (mut name, mut place, mut icon, mut title) = (None, None, None, None);
    for _ in 0..n {
        match d.str().ok()? {
            "name" => name = Some(String::from(d.str().ok()?)),
            "place" => place = Some(String::from(d.str().ok()?)),
            "icon" => icon = Some(d.bytes().ok()?.to_vec()),
            "title" => title = Some(String::from(d.str().ok()?)),
            _ => d.skip().ok()?,
        }
    }
    Some(Place { name: name?, place: place?, icon, title })
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::Encoder;

    fn place(e: &mut Encoder<Vec<u8>>, name: &str, place: &str, title: Option<&str>) {
        e.map(if title.is_some() { 3 } else { 2 }).unwrap();
        e.str("name").unwrap().str(name).unwrap();
        e.str("place").unwrap().str(place).unwrap();
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
        let mut e = Encoder::new(Vec::new());
        place(&mut e, "Quote", "quote.dafifi.net", Some("Quote of the day"));
        assert_eq!(Place::one(&e.into_writer()).unwrap().title.as_deref(), Some("Quote of the day"));
    }

    #[test]
    fn anything_else_is_none() {
        assert_eq!(Place::list(&[0xa0]), None);
        assert_eq!(Place::one(&[0x80]), None);
        // A place without its place.
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("name").unwrap().str("x").unwrap();
        assert_eq!(Place::one(&e.into_writer()), None);
    }
}
