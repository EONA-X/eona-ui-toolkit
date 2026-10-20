# Sample: a static site with Yew

A branded, two-page site rendered by Yew at build time — plain HTML and CSS, **no WebAssembly**.
The recommended way to build an EONA-X site in Rust.

```sh
cargo run          # writes dist/
python3 -m http.server -d dist 8000
```

- `src/main.rs` — the pages (Yew components), the shared header and footer, and the build.
- Outside this repository, swap the `path` dependency in `Cargo.toml` for the commented `git` one.

Guide: <https://eona-x.github.io/eona-ui-toolkit/yew/getting-started/>.
