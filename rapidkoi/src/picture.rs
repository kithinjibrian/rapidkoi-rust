//! Pictures (contract 7, PRP-047): from the app's bundle (`asset:NAME`,
//! packed from `[assets]` in `rapidkoi.toml`), from a server the app may
//! reach (its own, or one it declares), or from its own blobs
//! ([`image_blob`], contract 9, PRP-055). Rapidkoi loads a server's or a
//! blob's picture in the background, the box keeping its size meanwhile,
//! and shows a broken-picture mark if it can't.
//!
//! ```
//! use rapidkoi::{image, image_blob, Fit, Message, View};
//! use rapidkoi::style::{Color, Sizing};
//!
//! #[derive(Message)]
//! enum Msg { Shown, Broken }
//!
//! let trash: View<Msg> = image("asset:trash.svg").color(Color::system("text_dim")).label("Delete");
//! let photo: View<Msg> = image("https://photos.example.com/7.jpg")
//!     .fit(Fit::Cover)
//!     .style([Sizing::width(320), Sizing::height(200)])
//!     .on_load(Msg::Shown)
//!     .on_fail(Msg::Broken);
//! let mine: View<Msg> = image_blob("photo-17").fit(Fit::Cover).on_fail(Msg::Broken);
//! ```

use alloc::string::String;

use crate::message::{bytes_of, Message};
use crate::raw::{Val, View};
use crate::style::Color;

/// How a picture fills its box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    /// All of it, inside the box: the default.
    Contain,
    /// The whole box, cropped.
    Cover,
}

/// A picture: `"asset:NAME"` from the app's bundle (PNG, JPEG or SVG),
/// `"blob:KEY"` from the app's blobs ([`image_blob`]), or an `https://`
/// address on a server the app may reach (PNG or JPEG).
pub fn image<M>(src: &str) -> View<M> {
    View::new("image").prop("src", Val::Text(String::from(src)))
}

/// A picture from the app's own blobs (contract 9, PRP-055): `blob:` and
/// its key. Rapidkoi reads it from this device; its bytes never pass
/// through the app. PNG or JPEG, at most 10 MB, 4,096 px a side and 16 MP,
/// as a server's picture. No such key, or bytes that aren't a picture:
/// the broken-picture mark, and [`View::on_fail`]'s message. When a write
/// stores, replaces or removes blobs, the app's windows read their `blob:`
/// pictures again. It takes no [`View::color`].
pub fn image_blob<M>(key: &str) -> View<M> {
    debug_assert!(crate::blobs::key_ok(key), "a blob's key is 1 to 256 bytes, no control characters");
    image(&alloc::format!("blob:{key}"))
}

impl<M> View<M> {
    /// A picture: how it fills its box.
    pub fn fit(self, how: Fit) -> View<M> {
        let word = match how {
            Fit::Contain => "contain",
            Fit::Cover => "cover",
        };
        self.prop("fit", Val::Text(String::from(word)))
    }

    /// A one-colour SVG from the bundle: drawn in `c` (its `currentColor`).
    pub fn color(self, c: Color) -> View<M> {
        debug_assert!(
            !self.text_prop("src").is_some_and(|s| s.starts_with("blob:")),
            "a blob picture takes no color: only a bundle SVG does"
        );
        self.prop("color", c.into_val())
    }
}

impl<M: Message> View<M> {
    /// A picture: the message sent once it's shown, as [`crate::Event::Message`] with no value.
    pub fn on_load(self, msg: M) -> View<M> {
        self.prop("on_load", Val::Raw(bytes_of(&msg)))
    }

    /// A picture: the message sent when it can't be shown, as
    /// [`crate::Event::Message`] with Rapidkoi's sentence why as its value.
    pub fn on_fail(self, msg: M) -> View<M> {
        self.prop("on_fail", Val::Raw(bytes_of(&msg)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Sizing;

    #[test]
    fn an_image_encodes_its_source_fit_colour_label_and_messages() {
        let v = image::<String>("asset:trash.svg")
            .fit(Fit::Cover)
            .color(Color::system("text_dim"))
            .label("Delete")
            .on_load(String::from("shown"))
            .on_fail(String::from("broken"))
            .style([Sizing::width(24)]);
        let bytes = v.encode();
        let mut d = minicbor::Decoder::new(&bytes);
        // No children: an image has none.
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "image"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("props", Some(7)));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("src", "asset:trash.svg"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("fit", "cover"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("color", "text_dim"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("label", "Delete"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("on_load", "shown"));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("on_fail", "broken"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("style", Some(1)));
        assert_eq!((d.str().unwrap(), d.u32().unwrap()), ("width", 24));
    }

    #[test]
    fn a_blob_picture_is_blob_and_its_key() {
        let v = image_blob::<String>("photo-17").fit(Fit::Cover).on_fail(String::from("broken"));
        let bytes = v.encode();
        let mut d = minicbor::Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "image"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("props", Some(3)));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("src", "blob:photo-17"));
        assert_eq!(image_blob::<String>("photo-17"), image("blob:photo-17"));
    }

    #[test]
    #[should_panic(expected = "takes no color")]
    fn a_blob_picture_with_a_colour_is_caught() {
        let _ = image_blob::<String>("icon").color(Color::system("text"));
    }

    #[test]
    #[should_panic(expected = "1 to 256 bytes")]
    fn a_bad_blob_key_is_caught() {
        let _ = image_blob::<String>("a\nb");
    }

    #[test]
    fn a_plain_image_is_its_source() {
        let bytes = image::<String>("https://a.example.com/7.jpg").encode();
        let mut d = minicbor::Decoder::new(&bytes);
        assert_eq!(d.map().unwrap(), Some(2));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("kind", "image"));
        assert_eq!((d.str().unwrap(), d.map().unwrap()), ("props", Some(1)));
        assert_eq!((d.str().unwrap(), d.str().unwrap()), ("src", "https://a.example.com/7.jpg"));
    }
}
