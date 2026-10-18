//! Requests to the app's own server and the servers it declares
//! (`[network] servers` in `rapidkoi.toml`). Rapidkoi checks each one,
//! sends it in the background, and answers with [`crate::Event::Response`]
//! or [`crate::Event::Failed`]. There's no cookie jar and no redirect is
//! followed: keep a token in your state and send it as a header.

use alloc::string::String;
use alloc::vec::Vec;
use minicbor::Encoder;

/// One request: a method, an `https://` URL, headers and a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// GET, POST, PUT, PATCH or DELETE.
    pub method: String,
    /// Where: `https://host/path?query` (or `http://localhost:PORT/…` while developing).
    pub url: String,
    /// Header names and values. Rapidkoi refuses host, cookie, content-length,
    /// connection, transfer-encoding and upgrade.
    pub headers: Vec<(String, String)>,
    /// At most 1 MB.
    pub body: Vec<u8>,
}

impl Request {
    /// A request with no headers or body.
    pub fn new(method: &str, url: &str) -> Request {
        Request { method: method.into(), url: url.into(), headers: Vec::new(), body: Vec::new() }
    }
    /// A GET.
    pub fn get(url: &str) -> Request {
        Request::new("GET", url)
    }
    /// A POST with `body`.
    pub fn post(url: &str, body: &[u8]) -> Request {
        Request { body: body.to_vec(), ..Request::new("POST", url) }
    }
    /// The same request with one more header.
    pub fn header(mut self, name: &str, value: &str) -> Request {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Writes the effect `{kind: "fetch", msg, method, url, headers, body}`.
    pub(crate) fn encode_effect(&self, msg: &str, e: &mut Encoder<Vec<u8>>) {
        let _ = e.map(6).and_then(|e| e.str("kind")).and_then(|e| e.str("fetch"));
        let _ = e.str("msg").and_then(|e| e.str(msg));
        let _ = e.str("method").and_then(|e| e.str(&self.method));
        let _ = e.str("url").and_then(|e| e.str(&self.url));
        let _ = e.str("headers").and_then(|e| e.map(self.headers.len() as u64));
        for (k, v) in &self.headers {
            let _ = e.str(k).and_then(|e| e.str(v));
        }
        let _ = e.str("body").and_then(|e| e.bytes(&self.body));
    }
}

/// Finds a response header by name, in any case (Rapidkoi gives names in lower case).
pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}
