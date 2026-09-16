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
    COMPONENTS_CSS, LOGO_SVG, ONTOLOGY_CSS, TOKENS_CSS,
};
use yew::ServerRenderer;

/// One of `DEMO_SHELL`'s references belongs to the portal and a standalone build
/// cannot satisfy it.
///
/// `site-header.js` is the scroll-collapse glue for [`SiteHeader`]. Every portal
/// page uses it, which is why it lives there; without it the header renders and
/// simply never collapses. Vendoring a second copy here is how the two would
/// drift, so the tag is dropped rather than duplicated — a 404 in the console
/// reads as a broken deploy, and this one is not broken.
///
/// The favicon and the header's mark are no longer a problem: `LOGO_SVG` is a
/// crate constant now, written out below like the stylesheets.
fn strip_portal_assets(shell: &str) -> String {
    shell
        .lines()
        .filter(|line| !line.contains("site-header.js"))
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

    // `SiteHeader` hardcodes this path and the shell links it as the favicon, so
    // the directory name is part of the contract, not a choice.
    fs::create_dir_all(out.join("logo"))?;
    fs::write(out.join("logo/logo-transparent.svg"), LOGO_SVG)?;

    // Pages runs the upload through Jekyll unless told not to; `_`-prefixed paths
    // would be dropped silently. Nothing here starts with `_` today, so this is a
    // guard against a future asset that does.
    fs::write(out.join(".nojekyll"), "")?;

    println!(
        "wrote {} ({} KB of HTML) to {}",
        ["index.html", "tokens.css", "components.css", "ontology.css", "demo-page.js",
         "logo/logo-transparent.svg"].len(),
        page.len() / 1024,
        out.display()
    );
    Ok(())
}
