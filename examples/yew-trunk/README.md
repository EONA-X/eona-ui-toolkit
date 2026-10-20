# Sample: a Yew app with Trunk

A branded Yew app compiled to WebAssembly and bundled by [Trunk](https://trunkrs.dev) into a static
`dist/`. It holds state (the dataset counter) — interaction belongs to the app, the toolkit's
components stay presentational.

```sh
cargo install trunk && rustup target add wasm32-unknown-unknown
trunk serve              # http://127.0.0.1:8080
trunk build --release    # -> dist/
```

Outside this repository, swap the `path` dependency in `Cargo.toml` for the commented `git` one, and
copy the toolkit's `assets/logo/` into the project (see `index.html`). If `trunk build --release`
fails with `error parsing wasm`, your `wasm-opt` is older than your Rust toolchain.

Prefer plain HTML with no WebAssembly? See [`../yew-ssg`](../yew-ssg).
Guide: <https://eona-x.github.io/eona-ui-toolkit/yew/getting-started/>.
