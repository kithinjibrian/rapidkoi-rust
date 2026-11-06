//! Who the person is and whom they trust (contract 11 addition, Rapidkoi's
//! PRP-075), read by Rapidkoi's own Settings with the built-in `read_you`
//! and `read_publishers`. Public facts only: names, keys' fingerprints as
//! Rapidkoi shows them everywhere (`7f3a-91c2-0b5e-d4a8`), hosts and
//! dates. Never a secret, never a key itself.

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::Decoder;

/// An app the person signed in to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignedIn {
    /// Its name.
    pub name: String,
    /// Where it lives: its host.
    pub place: String,
    /// Its icon, for an image (`icon:PLACE`): Rapidkoi draws it.
    pub icon: String,
    /// The name it sees: its own when the person gave it one, else theirs.
    pub first: String,
    pub last: String,
    /// When the person first signed in, in milliseconds since 1970; 0 when not known.
    pub since: i64,
    /// The person's public key for it, as its fingerprint (`7f3a-91c2-0b5e-d4a8`); empty when Rapidkoi couldn't work it out.
    pub key: String,
}

/// The person: the name every app shows, and the apps they're signed in to, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct You {
    pub first: String,
    pub last: String,
    pub apps: Vec<SignedIn>,
}

/// A publisher's key the person trusts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Trusted {
    /// The host it signs for.
    pub host: String,
    /// The key's fingerprint (`7f3a-91c2-0b5e-d4a8`), as Rapidkoi's shield shows it.
    pub key: String,
    /// When it was first trusted, in milliseconds since 1970; 0 when not known.
    pub since: i64,
    /// The names of the apps it signs.
    pub signs: Vec<String>,
    /// How many times it has rotated to a new key.
    pub rotations: u32,
}

/// The keys the person trusts, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Publishers {
    pub items: Vec<Trusted>,
}

impl You {
    /// Reads a `you` answer; `None` if it isn't one.
    pub fn read(cbor: &[u8]) -> Option<You> {
        let mut d = Decoder::new(cbor);
        let n = d.map().ok()??;
        let mut out = You::default();
        for _ in 0..n {
            match d.str().ok()? {
                "first" => out.first = String::from(d.str().ok()?),
                "last" => out.last = String::from(d.str().ok()?),
                "apps" => {
                    let k = d.array().ok()??;
                    for _ in 0..k {
                        out.apps.push(signed_in(&mut d)?);
                    }
                }
                _ => d.skip().ok()?,
            }
        }
        Some(out)
    }
}

impl Publishers {
    /// Reads a `publishers` answer; `None` if it isn't one.
    pub fn read(cbor: &[u8]) -> Option<Publishers> {
        let mut d = Decoder::new(cbor);
        let n = d.map().ok()??;
        let mut out = Publishers::default();
        for _ in 0..n {
            match d.str().ok()? {
                "items" => {
                    let k = d.array().ok()??;
                    for _ in 0..k {
                        out.items.push(trusted(&mut d)?);
                    }
                }
                _ => d.skip().ok()?,
            }
        }
        Some(out)
    }
}

fn signed_in(d: &mut Decoder) -> Option<SignedIn> {
    let n = d.map().ok()??;
    let mut a = SignedIn::default();
    let mut named = false;
    for _ in 0..n {
        match d.str().ok()? {
            "name" => {
                a.name = String::from(d.str().ok()?);
                named = true;
            }
            "place" => a.place = String::from(d.str().ok()?),
            "icon" => a.icon = String::from(d.str().ok()?),
            "first" => a.first = String::from(d.str().ok()?),
            "last" => a.last = String::from(d.str().ok()?),
            "since" => a.since = d.i64().ok()?,
            "key" => a.key = String::from(d.str().ok()?),
            _ => d.skip().ok()?,
        }
    }
    if named { Some(a) } else { None }
}

fn trusted(d: &mut Decoder) -> Option<Trusted> {
    let n = d.map().ok()??;
    let mut t = Trusted::default();
    let mut hosted = false;
    for _ in 0..n {
        match d.str().ok()? {
            "host" => {
                t.host = String::from(d.str().ok()?);
                hosted = true;
            }
            "key" => t.key = String::from(d.str().ok()?),
            "since" => t.since = d.i64().ok()?,
            "signs" => {
                let k = d.array().ok()??;
                for _ in 0..k {
                    t.signs.push(String::from(d.str().ok()?));
                }
            }
            "rotations" => t.rotations = d.u32().ok()?,
            _ => d.skip().ok()?,
        }
    }
    if hosted { Some(t) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::Encoder;

    #[test]
    fn a_you_answer_reads_back() {
        let mut e = Encoder::new(Vec::new());
        e.map(3).unwrap();
        e.str("first").unwrap().str("Ada").unwrap();
        e.str("last").unwrap().str("L").unwrap();
        e.str("apps").unwrap().array(1).unwrap();
        e.map(8).unwrap();
        e.str("name").unwrap().str("Notes").unwrap();
        e.str("place").unwrap().str("notes.dafifi.net").unwrap();
        e.str("icon").unwrap().str("icon:notes.dafifi.net").unwrap();
        e.str("first").unwrap().str("Ada").unwrap();
        e.str("last").unwrap().str("").unwrap();
        e.str("since").unwrap().i64(1_760_000_000_000).unwrap();
        e.str("key").unwrap().str("4b2e9d71").unwrap();
        // Something a later Rapidkoi adds is skipped.
        e.str("later").unwrap().u32(1).unwrap();
        let you = You::read(&e.into_writer()).unwrap();
        assert_eq!((you.first.as_str(), you.last.as_str(), you.apps.len()), ("Ada", "L", 1));
        let a = &you.apps[0];
        assert_eq!((a.name.as_str(), a.key.as_str(), a.since), ("Notes", "4b2e9d71", 1_760_000_000_000));
    }

    #[test]
    fn a_signed_in_app_without_a_name_isnt_one() {
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("apps").unwrap().array(1).unwrap();
        e.map(1).unwrap().str("place").unwrap().str("x").unwrap();
        assert_eq!(You::read(&e.into_writer()), None);
        assert_eq!(You::read(&[0xff]), None);
    }

    #[test]
    fn a_publishers_answer_reads_back() {
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("items").unwrap().array(1).unwrap();
        e.map(5).unwrap();
        e.str("host").unwrap().str("counter.dafifi.net").unwrap();
        e.str("key").unwrap().str("c1d944e0").unwrap();
        e.str("since").unwrap().i64(1_761_000_000_000).unwrap();
        e.str("signs").unwrap().array(1).unwrap().str("Counter").unwrap();
        e.str("rotations").unwrap().u32(1).unwrap();
        let p = Publishers::read(&e.into_writer()).unwrap();
        let t = &p.items[0];
        assert_eq!((t.host.as_str(), t.key.as_str(), t.rotations), ("counter.dafifi.net", "c1d944e0", 1));
        assert_eq!(t.signs, alloc::vec![String::from("Counter")]);
    }

    #[test]
    fn a_key_without_a_host_isnt_one() {
        let mut e = Encoder::new(Vec::new());
        e.map(1).unwrap().str("items").unwrap().array(1).unwrap();
        e.map(1).unwrap().str("key").unwrap().str("ab").unwrap();
        assert_eq!(Publishers::read(&e.into_writer()), None);
    }
}
