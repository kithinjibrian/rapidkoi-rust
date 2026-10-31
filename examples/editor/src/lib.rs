//! A small text editor: open a file to modify, edit it, save it back or
//! save it as another. The person chooses every file in Rapidkoi's own
//! dialog; the editor learns only its name and size, and keeps the handle
//! that lets it save back while its window is open.
//!
//! The text is held in the window's state, so a file near 16 MB won't fit.
//!
//! Build: `cargo build --release --target wasm32-unknown-unknown` in this
//! repository's root, then open `target/wasm32-unknown-unknown/release/editor.wasm` in Rapidkoi.

use rapidkoi::{button, column, row, text, text_area, App, Ask, Event, Message, Next, Space, State, Step, View};

/// The blob the file's bytes go through, both ways.
const DOC: &str = "doc";

/// Rapidkoi's sentences the editor answers with something more than showing them.
const CHANGED: &str = "This file changed since it was opened";
const GONE: &str = "This file is no longer open here";

#[derive(Default, State)]
struct Editor {
    /// The file's name (`notes.txt`), empty until one is open.
    name: String,
    /// The handle to save back to it: only good in this window, while it's open.
    handle: Option<String>,
    text: String,
    /// The file's bytes while its pieces arrive; empty once it's read.
    loading: Vec<u8>,
    /// Typed into since it was opened or saved.
    edited: bool,
    /// The last thing to tell the person: "Saved", or why not.
    note: String,
    /// The file changed on disk since it was opened: offer to open it again, or save as.
    changed: bool,
}

/// What the editor's screen sends.
#[derive(Debug, Clone, PartialEq, Message)]
enum Msg {
    Open,
    /// The text changed: the text comes as the event's `value`.
    Edit,
    Save,
    SaveAs,
}

/// Stores the text as the blob, so `then` (asked on the batch's answer) reads it.
fn store(msg: &str, text: &str) -> Ask {
    Ask::sql_batch_steps(msg, vec![Step::blob_set(DOC, text.as_bytes().to_vec())])
}

impl Editor {
    fn told(self, note: &str) -> Editor {
        Editor { note: note.into(), ..self }
    }
}

impl App for Editor {
    type Msg = Msg;

    fn handle(self, event: Event<Msg>) -> Next<Self> {
        match event {
            Event::Message { msg, value } => match msg {
                Msg::Open => Next::state(self).ask(Ask::file_open("opened", DOC).edit()),
                Msg::Edit => Next::state(Editor { text: value.unwrap_or_default(), edited: true, ..self }),
                // Never opened, or its handle ended: there's nothing to save back to.
                Msg::Save if self.handle.is_none() => self.handle(Event::Message { msg: Msg::SaveAs, value: None }),
                Msg::Save => {
                    let ask = store("stored", &self.text);
                    Next::state(self).ask(ask)
                }
                Msg::SaveAs => {
                    let ask = store("stored-as", &self.text);
                    Next::state(self).ask(ask)
                }
            },
            // Opened: now read it, piece by piece.
            Event::FileOpened { msg, name, handle, .. } if msg == "opened" => {
                Next::state(Editor { name, handle, ..Editor::default() }).ask(Ask::blob_get("read", DOC))
            }
            Event::Blob { msg, bytes, from, size, hash } if msg == "read" => {
                let mut s = self;
                s.loading.extend_from_slice(&bytes);
                let next = from + bytes.len() as u64;
                if next < size && !bytes.is_empty() {
                    return Next::state(s).ask(Ask::blob_get("read", DOC).from(next).hash(&hash));
                }
                s.text = String::from_utf8_lossy(&s.loading).into_owned();
                s.loading.clear();
                Next::state(s)
            }
            // Stored: the blob holds the text now, so the file can take it.
            Event::Done { msg, .. } if msg == "stored" => match self.handle.clone() {
                Some(h) => Next::state(self).ask(Ask::file_save("saved", &h, DOC)),
                None => Next::state(self),
            },
            Event::Done { msg, .. } if msg == "stored-as" => {
                let name = if self.name.is_empty() { String::from("untitled.txt") } else { self.name.clone() };
                Next::state(self).ask(Ask::file_save_as("exported", DOC).name(&name))
            }
            Event::FileSaved { msg, .. } if msg == "saved" => {
                Next::state(Editor { edited: false, changed: false, ..self.told("Saved") })
            }
            Event::FileSavedAs { msg, name, handle, .. } if msg == "exported" => {
                Next::state(Editor { name, handle: Some(handle), edited: false, changed: false, ..self.told("Saved") })
            }
            // Closing a dialog without choosing changes nothing.
            Event::FileCancelled { .. } => Next::state(self),
            Event::Failed { error, .. } if error == CHANGED => Next::state(Editor { changed: true, ..self.told(&error) }),
            // Its window was reloaded: saving goes through Save as now.
            Event::Failed { error, .. } if error == GONE => Next::state(Editor { handle: None, ..self.told(&error) }),
            Event::Failed { error, .. } => Next::state(Editor { loading: Vec::new(), ..self.told(&error) }),
            _ => Next::state(self),
        }
    }

    fn view(&self) -> View<Msg> {
        let title = if self.name.is_empty() { "Editor" } else { self.name.as_str() };
        let mut items = vec![
            text(title).title(),
            row([
                button("Open").on_press(Msg::Open),
                button("Save").primary().on_press_if(self.edited, Msg::Save),
                button("Save as").on_press(Msg::SaveAs),
            ])
            .spacing(Space::Sm),
        ];
        if self.changed {
            items.push(text(&self.note));
            items.push(row([button("Open it again").on_press(Msg::Open), button("Save as").on_press(Msg::SaveAs)]).spacing(Space::Sm));
        } else if !self.note.is_empty() {
            items.push(text(&self.note).dim());
        }
        items.push(text_area(&self.text).lines(20).label("Text").on_change(Msg::Edit));
        column(items).spacing(Space::Md).screen_title(title)
    }
}

rapidkoi::export!(Editor);

#[cfg(test)]
mod tests {
    use super::*;

    fn press(e: Editor, msg: Msg) -> Next<Editor> {
        e.handle(Event::Message { msg, value: None })
    }

    fn opened() -> Editor {
        let next = Editor::default().handle(Event::FileOpened {
            msg: "opened".into(),
            key: DOC.into(),
            name: "notes.txt".into(),
            size: 9,
            handle: Some("h1".into()),
        });
        assert_eq!(next.asks(), &[Ask::blob_get("read", DOC).into()]);
        let e = next.into_state();
        // café's é split across two pieces.
        let word = "café\nok\n".as_bytes();
        let next = e.handle(Event::Blob { msg: "read".into(), bytes: word[..4].to_vec(), from: 0, size: 9, hash: vec![5; 32] });
        assert_eq!(next.asks(), &[Ask::blob_get("read", DOC).from(4).hash(&[5; 32]).into()]);
        let e = next.into_state().handle(Event::Blob { msg: "read".into(), bytes: word[4..].to_vec(), from: 4, size: 9, hash: vec![5; 32] }).into_state();
        assert!(e.loading.is_empty());
        e
    }

    #[test]
    fn open_asks_to_modify_and_reads_the_file_in_pieces() {
        assert_eq!(press(Editor::default(), Msg::Open).asks(), &[Ask::file_open("opened", DOC).edit().into()]);
        let e = opened();
        assert_eq!((e.name.as_str(), e.handle.as_deref(), e.text.as_str()), ("notes.txt", Some("h1"), "café\nok\n"));
    }

    #[test]
    fn save_stores_then_saves_back() {
        let e = opened().handle(Event::Message { msg: Msg::Edit, value: Some("new".into()) }).into_state();
        assert!(e.edited);
        let next = press(e, Msg::Save);
        assert_eq!(next.asks(), &[store("stored", "new")]);
        let next = next.into_state().handle(Event::Done { msg: "stored".into(), changes: 0, last_rowid: 0 });
        assert_eq!(next.asks(), &[Ask::file_save("saved", "h1", DOC)]);
        let e = next.into_state().handle(Event::FileSaved { msg: "saved".into(), size: 3 }).into_state();
        assert_eq!((e.edited, e.note.as_str()), (false, "Saved"));
    }

    #[test]
    fn save_as_suggests_the_name_and_keeps_the_new_handle() {
        let next = press(opened(), Msg::SaveAs);
        assert_eq!(next.asks(), &[store("stored-as", "café\nok\n")]);
        let next = next.into_state().handle(Event::Done { msg: "stored-as".into(), changes: 0, last_rowid: 0 });
        assert_eq!(next.asks(), &[Ask::file_save_as("exported", DOC).name("notes.txt").into()]);
        let e = next
            .into_state()
            .handle(Event::FileSavedAs { msg: "exported".into(), name: "copy.txt".into(), size: 9, handle: "h2".into() })
            .into_state();
        assert_eq!((e.name.as_str(), e.handle.as_deref()), ("copy.txt", Some("h2")));
        // Nothing open yet: Save is Save as, named untitled.
        let next = press(Editor::default(), Msg::Save).into_state().handle(Event::Done { msg: "stored-as".into(), changes: 0, last_rowid: 0 });
        assert_eq!(next.asks(), &[Ask::file_save_as("exported", DOC).name("untitled.txt").into()]);
    }

    #[test]
    fn a_file_changed_elsewhere_offers_to_open_again_or_save_as() {
        let e = opened().handle(Event::Failed { msg: "saved".into(), error: CHANGED.into() }).into_state();
        assert!(e.changed);
        let v = e.view();
        assert_eq!(
            v,
            column([
                text("notes.txt").title(),
                row([button("Open").on_press(Msg::Open), button("Save").primary(), button("Save as").on_press(Msg::SaveAs)]).spacing(Space::Sm),
                text(CHANGED),
                row([button("Open it again").on_press(Msg::Open), button("Save as").on_press(Msg::SaveAs)]).spacing(Space::Sm),
                text_area("café\nok\n").lines(20).label("Text").on_change(Msg::Edit),
            ])
            .spacing(Space::Md)
            .screen_title("notes.txt")
        );
    }

    #[test]
    fn cancelling_changes_nothing_and_a_lost_handle_means_save_as() {
        let e = opened().handle(Event::FileCancelled { msg: "opened".into() }).into_state();
        assert_eq!((e.text.as_str(), e.note.as_str()), ("café\nok\n", ""));
        let e = e.handle(Event::Failed { msg: "saved".into(), error: GONE.into() }).into_state();
        assert_eq!(e.handle, None);
        assert_eq!(press(e, Msg::Save).asks(), &[store("stored-as", "café\nok\n")]);
    }
}
