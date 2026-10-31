//! Requests to the app's own server and the servers it declares
//! (`[network] servers` in `rapidkoi.toml`). Rapidkoi checks each one,
//! sends it in the background, and answers with [`crate::Event::Response`]
//! or [`crate::Event::Failed`]. There's no cookie jar and no redirect is
//! followed: keep a token in your state and send it as a header.
//!
//! A body can be one of the app's blobs ([`Request::body_blob`], contract
//! 9, PRP-055): Rapidkoi reads it once the request is allowed and sends it,
//! up to 16 MB, never passing through the app. No such key:
//! [`crate::Event::Failed`] with "This app has no blob with that key", and
//! nothing is sent.
//!
//! ```
//! use rapidkoi::{Ask, Request};
//!
//! let upload = Ask::fetch("uploaded", Request::new("PUT", "https://api.example.com/photos/17")
//!     .header("content-type", "image/jpeg")
//!     .body_blob("photo-17"));
//! ```

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
    /// At most 1 MB. Not sent when [`Request::body_blob`] is.
    pub body: Vec<u8>,
    /// A stored blob's key, sent as the body instead of `body` (contract 9).
    pub body_blob: Option<String>,
}

impl Request {
    /// A request with no headers or body.
    pub fn new(method: &str, url: &str) -> Request {
        Request { method: method.into(), url: url.into(), headers: Vec::new(), body: Vec::new(), body_blob: None }
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

    /// The same request with the blob stored under `key` as its body (up
    /// to 16 MB), in place of `body`: a fetch never carries both.
    pub fn body_blob(mut self, key: &str) -> Request {
        debug_assert!(crate::blobs::key_ok(key), "a blob's key is 1 to 256 bytes, no control characters");
        self.body.clear();
        self.body_blob = Some(key.into());
        self
    }

    /// Writes the effect `{kind: "fetch", msg, method, url, headers, body}`,
    /// with `body_blob` in place of `body` when it has one.
    pub(crate) fn encode_effect(&self, msg: &str, e: &mut Encoder<Vec<u8>>) {
        let _ = e.map(6).and_then(|e| e.str("kind")).and_then(|e| e.str("fetch"));
        let _ = e.str("msg").and_then(|e| e.str(msg));
        let _ = e.str("method").and_then(|e| e.str(&self.method));
        let _ = e.str("url").and_then(|e| e.str(&self.url));
        let _ = e.str("headers").and_then(|e| e.map(self.headers.len() as u64));
        for (k, v) in &self.headers {
            let _ = e.str(k).and_then(|e| e.str(v));
        }
        let _ = match &self.body_blob {
            Some(key) => e.str("body_blob").and_then(|e| e.str(key)),
            None => e.str("body").and_then(|e| e.bytes(&self.body)),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::Decoder;

    fn fields(r: &Request) -> Vec<String> {
        let mut e = Encoder::new(Vec::new());
        r.encode_effect("m", &mut e);
        let bytes = e.into_writer();
        let mut d = Decoder::new(&bytes);
        let n = d.map().unwrap().unwrap();
        let mut out = Vec::new();
        for _ in 0..n {
            out.push(String::from(d.str().unwrap()));
            d.skip().unwrap();
        }
        out
    }

    #[test]
    fn a_blob_body_goes_in_place_of_body() {
        let r = Request::post("https://api.example.com/p", b"old").body_blob("photo-17");
        assert_eq!(fields(&r), ["kind", "msg", "method", "url", "headers", "body_blob"]);
        let mut e = Encoder::new(Vec::new());
        r.encode_effect("m", &mut e);
        let bytes = e.into_writer();
        assert!(bytes.ends_with(b"\x69body_blob\x68photo-17"));
        // Without one, a request is as before.
        assert_eq!(fields(&Request::get("https://a.example.com/")), ["kind", "msg", "method", "url", "headers", "body"]);
    }
}

/// Finds a response header by name, in any case (Rapidkoi gives names in lower case).
pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}
