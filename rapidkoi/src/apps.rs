//! The person's apps (contract 11 addition, Rapidkoi's PRP-067): every app
//! they've opened, A–Z, read by Rapidkoi's own All apps with the built-in
//! `read_apps`. Each is a name, where it's from (a host, or a file's name),
//! an icon for Rapidkoi to draw, and what the person may want to know
//! before opening, pinning or forgetting it: never a folder, state or key.

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::Decoder;

/// One of the person's apps, as Rapidkoi lists it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListedApp {
    /// Its name.
    pub name: String,
    /// Where it lives: a site's host, or a local app's file name.
    pub place: String,
    /// Its icon, for an image (`icon:PLACE`): Rapidkoi draws it.
    pub icon: String,
    /// Where it's from, to tell apps with the same name apart; empty for Rapidkoi's own.
    pub from: String,
    /// Its address as the address box shows it; empty for Rapidkoi's own.
    pub address: String,
    /// The host that signed it; empty when unsigned.
    pub publisher: String,
    /// Whether it's signed.
    pub signed: bool,
    /// Whether it's on the dock.
    pub pinned: bool,
    /// When the person last opened it, in milliseconds since 1970; 0 when not known.
    pub opened: i64,
}

/// The person's apps, A–Z.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Apps {
    pub items: Vec<ListedApp>,
}

impl Apps {
    /// Reads an apps answer; `None` if it isn't one.
    pub fn read(cbor: &[u8]) -> Option<Apps> {
        let mut d = Decoder::new(cbor);
        let n = d.map().ok()??;
        let mut out = Apps::default();
        for _ in 0..n {
            match d.str().ok()? {
                "items" => {
                    let k = d.array().ok()??;
                    for _ in 0..k {
                        out.items.push(read_one(&mut d)?);
                    }
                }
                _ => d.skip().ok()?,
            }
        }
        Some(out)
    }
}

fn read_one(d: &mut Decoder) -> Option<ListedApp> {
    let n = d.map().ok()??;
    let mut a = ListedApp::default();
    let mut named = false;
    for _ in 0..n {
        match d.str().ok()? {
            "name" => {
                a.name = String::from(d.str().ok()?);
                named = true;
            }
            "place" => a.place = String::from(d.str().ok()?),
            "icon" => a.icon = String::from(d.str().ok()?),
            "from" => a.from = String::from(d.str().ok()?),
            "address" => a.address = String::from(d.str().ok()?),
            "publisher" => a.publisher = String::from(d.str().ok()?),
            "signed" => a.signed = d.bool().ok()?,
            "pinned" => a.pinned = d.bool().ok()?,
            "opened" => a.opened = d.i64().ok()?,
            _ => d.skip().ok()?,
        }
    }
    if named { Some(a) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::Encoder;

    #[test]
    fn an_apps_answer_reads_back() {
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("items").unwrap().array(1).unwrap();
        e.map(10).unwrap();
        e.str("name").unwrap().str("Notes").unwrap();
        e.str("place").unwrap().str("notes.dafifi.net").unwrap();
        e.str("icon").unwrap().str("icon:notes.dafifi.net").unwrap();
        e.str("from").unwrap().str("notes.dafifi.net").unwrap();
        e.str("address").unwrap().str("rapidkoi://notes.dafifi.net").unwrap();
        e.str("publisher").unwrap().str("notes.dafifi.net").unwrap();
        e.str("signed").unwrap().bool(true).unwrap();
        e.str("pinned").unwrap().bool(false).unwrap();
        e.str("opened").unwrap().i64(1_762_000_000_000).unwrap();
        // Something a later Rapidkoi adds is skipped.
        e.str("later").unwrap().u32(1).unwrap();
        let apps = Apps::read(&e.into_writer()).unwrap();
        assert_eq!(apps.items.len(), 1);
        let a = &apps.items[0];
        assert_eq!((a.name.as_str(), a.from.as_str(), a.signed, a.pinned), ("Notes", "notes.dafifi.net", true, false));
        assert_eq!(a.opened, 1_762_000_000_000);
    }

    #[test]
    fn an_app_without_a_name_isnt_one() {
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("items").unwrap().array(1).unwrap();
        e.map(1).unwrap().str("place").unwrap().str("x").unwrap();
        assert_eq!(Apps::read(&e.into_writer()), None);
        assert_eq!(Apps::read(&[0xff]), None);
    }
}
