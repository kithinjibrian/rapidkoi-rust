# rapidkoi-rust

Write [Rapidkoi](https://github.com/kithinjibrian/rapidkoi) apps in Rust. An app is a WebAssembly module: Rapidkoi runs it sandboxed and draws the screens it describes.

- `rapidkoi/`: the library. Your app's state, its events, its screens (boxes, texts, inputs, styles and roles), links, storage (SQLite), network requests and sign-in. It speaks Rapidkoi's contract 9, the only one Rapidkoi runs. Beside its database an app keeps blobs: large bytes under keys it names, stored in a batch with its rows (`Step::blob_set`, owned by a row or not), read in pieces (`Ask::blob_get`) or line by line (`Ask::blob_lines`), shown as a picture (`image_blob`) and sent as a request's body (`Request::body_blob`). Names starting `rapidkoi_` are Rapidkoi's. A text can be made of parts (`text_of`): the person's name and times ("Edited 5 minutes ago"), filled in by Rapidkoi while drawing, so the app never learns them; `Ask::now` gives the time. A link field that can be empty is an `Option`: none leaves it out of the address, and a link can still set it.
- `rapidkoi-derive/`: `#[derive(State)]` and `#[derive(Message)]`.
- `examples/counter/`: the shortest app: a count in its address, and a greeting.
- `examples/styled/`: styling, shown.

## Using it

```toml
[dependencies]
rapidkoi = { git = "https://github.com/kithinjibrian/rapidkoi-rust", tag = "v0.3.0-pre.3" }
minicbor = { version = "2", features = ["alloc", "derive"] }

[lib]
crate-type = ["cdylib"]
```

The library's docs: `cargo doc --open -p rapidkoi`.

## Building an example

Needs Rust's `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`), and Rapidkoi's `./main` for opening and packing.

```bash
cargo test
cargo build --release --target wasm32-unknown-unknown
./main target/wasm32-unknown-unknown/release/counter.wasm

# One bundle, then open it
./main pack target/wasm32-unknown-unknown/release/counter.wasm --name Counter --version 0.1.0 \
    --description "Counts, and says hello" --icon examples/counter/icon.png -o counter.rkb
./main counter.rkb

# Signed: examples/counter/rapidkoi.toml describes it
cd examples/counter
./main keygen     # once: the secret key goes outside the project
./main publish    # writes site/.well-known/rapidkoi/
./main serve site/
```
