# rapidkoi-rust

Write [Rapidkoi](https://github.com/kithinjibrian/rapidkoi) apps in Rust. An app is a WebAssembly module: Rapidkoi runs it sandboxed and draws the screens it describes.

- `rapidkoi/`: the library. Your app's state, its events, its screens (boxes, texts, inputs, styles and roles), links, storage (SQLite), network requests and sign-in. It speaks Rapidkoi's contract 6.
- `rapidkoi-derive/`: `#[derive(State)]` and `#[derive(Message)]`.
- `examples/counter/`: the shortest app: a count in its address, and a greeting.
- `examples/styled/`: styling, shown.

## Using it

```toml
[dependencies]
rapidkoi = { git = "https://github.com/kithinjibrian/rapidkoi-rust", tag = "v0.1.0" }
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
