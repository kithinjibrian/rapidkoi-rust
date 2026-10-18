//! Signing in to the app's own server with the person's key for this app
//! (`docs/server-sign-in.md` says what the server checks). Three steps,
//! each an answer from Rapidkoi:
//!
//! 1. fetch a challenge (`GET /rapidkoi/challenge`), and ask for the key
//! 2. ask Rapidkoi to sign the challenge (it signs it with the app's host)
//! 3. post the key, challenge and signature (`POST /rapidkoi/sign-in`)
//!
//! Keep the [`SignIn`] in your state, pass it every event, and look at
//! what [`SignIn::on`] says. Signed out in Rapidkoi, step 1 or 2 gets no
//! key or signature, so it fails: the server never sees proof.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use minicbor::{Decode, Encode, Encoder};

use crate::{Ask, Event, Request};

const CHALLENGE: &str = "rapidkoi:sign-in:challenge";
const KEY: &str = "rapidkoi:sign-in:key";
const SIGNATURE: &str = "rapidkoi:sign-in:signature";
const POSTED: &str = "rapidkoi:sign-in:posted";

/// A sign-in in progress. Encodable, so it lives in the app's state between events.
#[derive(Debug, Clone, Default, PartialEq, Eq, Encode, Decode)]
pub struct SignIn {
    /// The app's own server, like `https://notes.acme.com`.
    #[n(0)]
    pub server: String,
    #[n(1)]
    key: Option<Vec<u8>>,
    #[n(2)]
    challenge: Option<Vec<u8>>,
}

/// What one event did to a sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Not a sign-in event: handle it yourself.
    NotMine,
    /// Still going: ask Rapidkoi for these.
    Asks(Vec<Ask>),
    /// The server accepted: its reply's body (a session token, usually).
    SignedIn(Vec<u8>),
    /// It didn't work, and why.
    Failed(String),
}

impl SignIn {
    /// Starts signing in to `server` (`https://host`, no trailing `/`).
    pub fn start(server: &str) -> (SignIn, Vec<Ask>) {
        let s = SignIn { server: server.into(), key: None, challenge: None };
        let asks = vec![
            Ask::fetch(CHALLENGE, Request::get(&format!("{}/rapidkoi/challenge", server))),
            Ask::identity(KEY),
        ];
        (s, asks)
    }

    /// Takes one event: the next asks, the result, or [`Progress::NotMine`].
    pub fn on(&mut self, event: &Event) -> Progress {
        match event {
            Event::Response { msg, status, body, .. } if msg == CHALLENGE => {
                if *status != 200 || body.is_empty() || body.len() > 1024 {
                    return Progress::Failed(format!("The server gave no challenge ({})", status));
                }
                self.challenge = Some(body.clone());
                Progress::Asks(vec![Ask::sign(SIGNATURE, body)])
            }
            Event::Answer { msg, value } if msg == KEY => match value {
                Some(k) => {
                    self.key = Some(k.clone());
                    Progress::Asks(vec![])
                }
                None => Progress::Failed("Not signed in to this app in Rapidkoi".into()),
            },
            Event::Answer { msg, value } if msg == SIGNATURE => {
                match (value, &self.key, &self.challenge) {
                    (Some(sig), Some(key), Some(challenge)) => {
                        let body = proof(key, challenge, sig);
                        let post = Request::post(&format!("{}/rapidkoi/sign-in", self.server), &body)
                            .header("content-type", "application/cbor");
                        Progress::Asks(vec![Ask::fetch(POSTED, post)])
                    }
                    _ => Progress::Failed("Not signed in to this app in Rapidkoi".into()),
                }
            }
            Event::Response { msg, status, body, .. } if msg == POSTED => {
                if *status == 200 {
                    Progress::SignedIn(body.clone())
                } else {
                    Progress::Failed(format!("The server refused the sign-in ({})", status))
                }
            }
            Event::Failed { msg, error } if msg.starts_with("rapidkoi:sign-in:") => {
                Progress::Failed(error.clone())
            }
            _ => Progress::NotMine,
        }
    }
}

/// `{key, challenge, signature}` as CBOR: what the server checks.
fn proof(key: &[u8], challenge: &[u8], signature: &[u8]) -> Vec<u8> {
    let mut e = Encoder::new(Vec::new());
    let _ = e.map(3);
    let _ = e.str("key").and_then(|e| e.bytes(key));
    let _ = e.str("challenge").and_then(|e| e.bytes(challenge));
    let _ = e.str("signature").and_then(|e| e.bytes(signature));
    e.into_writer()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(msg: &str, status: u16, body: &[u8]) -> Event {
        Event::Response { msg: msg.into(), status, headers: vec![], body: body.to_vec() }
    }

    #[test]
    fn the_three_steps() {
        let (mut s, asks) = SignIn::start("https://notes.acme.com");
        assert_eq!(asks[0], Ask::fetch(CHALLENGE, Request::get("https://notes.acme.com/rapidkoi/challenge")));
        assert_eq!(asks[1], Ask::identity(KEY));
        assert_eq!(s.on(&Event::Answer { msg: KEY.into(), value: Some(vec![1; 32]) }), Progress::Asks(vec![]));
        assert_eq!(s.on(&response(CHALLENGE, 200, &[9, 9])), Progress::Asks(vec![Ask::sign(SIGNATURE, &[9, 9])]));
        match s.on(&Event::Answer { msg: SIGNATURE.into(), value: Some(vec![2; 64]) }) {
            Progress::Asks(a) => match &a[0] {
                Ask::Fetch(m, r) => {
                    assert_eq!((m.as_str(), r.method.as_str()), (POSTED, "POST"));
                    assert_eq!(r.url, "https://notes.acme.com/rapidkoi/sign-in");
                    assert_eq!(r.body, proof(&[1; 32], &[9, 9], &[2; 64]));
                }
                other => panic!("{:?}", other),
            },
            other => panic!("{:?}", other),
        }
        assert_eq!(s.on(&response(POSTED, 200, b"token")), Progress::SignedIn(b"token".to_vec()));
        // It survives being kept in the app's state.
        let back: SignIn = minicbor::decode(&minicbor::to_vec(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn signed_out_or_refused_it_fails() {
        let (mut s, _) = SignIn::start("https://notes.acme.com");
        assert!(matches!(s.on(&Event::Answer { msg: KEY.into(), value: None }), Progress::Failed(_)));
        assert!(matches!(s.on(&Event::Answer { msg: SIGNATURE.into(), value: None }), Progress::Failed(_)));
        assert!(matches!(s.on(&response(CHALLENGE, 500, &[])), Progress::Failed(_)));
        assert!(matches!(s.on(&response(POSTED, 401, &[])), Progress::Failed(_)));
        let off = Event::Failed { msg: CHALLENGE.into(), error: "You turned off notes.acme.com for Notes".into() };
        assert_eq!(s.on(&off), Progress::Failed("You turned off notes.acme.com for Notes".into()));
        assert_eq!(s.on(&Event::Opened), Progress::NotMine);
    }
}
