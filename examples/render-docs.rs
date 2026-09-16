//! Renders the component gallery to a static site, for publishing on GitHub Pages.
//!
//! The portal (`developer.eona-x.eu`) publishes the same page as `components.html`
//! inside a larger site; this renders it standalone so the crate's own repository
//! can serve it without the portal.
//!
//! Usage: `cargo run --example render-docs --features ssr -- <out-dir>`

use std::{env, fs, path::PathBuf};

use eona_ui_toolkit::{
    demo::{DemoPage, DEMO_JS, DEMO_SHELL},
    COMPONENTS_CSS, ONTOLOGY_CSS, TOKENS_CSS,
};
use yew::ServerRenderer;

/// Two of `DEMO_SHELL`'s references belong to the portal, not to this crate, and
/// a standalone build has no way to satisfy them:
///
/// - `site-header.js` is the scroll-collapse glue for [`SiteHeader`]. Every portal
///   page uses it, which is why it lives there; without it the header renders
///   correctly and simply never collapses. Vendoring a second copy here is how the
///   two would drift, so the tag is dropped instead of duplicated.
/// - `logo/logo-transparent.svg` is the EONA-X mark, which the integration guide
///   already documents as the host's to supply (`integration_section.rs`). The
///   portal's copy is 264 KB of base64-embedded raster rather than a real vector,
///   and the brand repository has no vector mark to use instead, so this deploy
///   ships without it. Only the favicon link is dropped here; `SiteHeader`'s two
///   `<img>` tags stay as the component emits them, and because both carry
///   `alt=""` a failed load renders nothing rather than a broken-image icon.
///
/// Dropping the tags rather than leaving them is deliberate: a 404 in the console
/// reads as a broken deploy, and this one is not broken.
fn strip_portal_assets(shell: &str) -> String {
    shell
        .lines()
        .filter(|line| {
            !line.contains("site-header.js") && !line.contains("logo/logo-transparent.svg")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out: PathBuf = env::args()
        .nth(1)
        .unwrap_or_else(|| "dist".into())
        .into();
    fs::create_dir_all(&out)?;

    // `DemoPage`'s props all default to the standalone case — no sibling pages to
    // link to in `nav`, an in-page jump for the footer link.
    let body = ServerRenderer::<DemoPage>::new().render().await;

    // `index.html`, not `components.html`: Pages serves this at the site root.
    let page = strip_portal_assets(DEMO_SHELL).replace("__APP__", &body);
    fs::write(out.join("index.html"), &page)?;

    // Order matters at load time, not on disk: `tokens.css` declares the custom
    // properties the other two consume.
    fs::write(out.join("tokens.css"), TOKENS_CSS)?;
    fs::write(out.join("components.css"), COMPONENTS_CSS)?;
    fs::write(out.join("ontology.css"), ONTOLOGY_CSS)?;
    fs::write(out.join("demo-page.js"), DEMO_JS)?;

    // Pages runs the upload through Jekyll unless told not to; `_`-prefixed paths
    // would be dropped silently. Nothing here starts with `_` today, so this is a
    // guard against a future asset that does.
    fs::write(out.join(".nojekyll"), "")?;

    println!(
        "wrote {} ({} KB of HTML) to {}",
        ["index.html", "tokens.css", "components.css", "ontology.css", "demo-page.js"].len(),
        page.len() / 1024,
        out.display()
    );
    Ok(())
}
