//! Guards the Yew 0.21 -> 0.23 migration: the toolkit must render byte-identical
//! markup to what developer.eona-x.eu committed under `crates/site/dist/`, because
//! the portal is a static site whose CSS keys off exact attributes (notably
//! `.modal[inert]` and `.submenu[inert]`).
//!
//! The 0.23 upgrade forced two source changes — `inert=""` -> `inert={true}` (the
//! attribute became typed as a boolean) and `<textarea></textarea>` -> `<textarea />`
//! (it became a void element).
//!
//! The textarea change is output-identical. The `inert` change is NOT: 0.21 serialized
//! the attribute as `inert=""`, 0.23 serializes it as `inert="inert"`. That is cosmetic
//! — `[inert]` CSS selectors match on attribute presence irrespective of value, and in
//! HTML any value sets the boolean — but it does mean regenerating the portal's `dist/`
//! produces a diff. What must never regress is the attribute being emitted at all, since
//! `.modal[inert]` / `.submenu[inert]` is what keeps them closed. That is what is asserted
//! here, deliberately without pinning the serialization.

use eona_ui_toolkit::ComponentsGallery;
use yew::ServerRenderer;

async fn gallery_html() -> String {
    ServerRenderer::<ComponentsGallery>::new().render().await
}

#[tokio::test]
async fn inert_still_renders_as_a_present_attribute() {
    let html = gallery_html().await;

    // Element shape is pinned to the 0.21 output in crates/site/dist/components.html;
    // only the inert value is left free, per the module note above.
    let submenu = regex_lite_find(&html, r#"<div id="gallery-dropdown-eonax" inert="#);
    assert!(
        submenu.is_some_and(|tag| tag.contains(r#"class="submenu""#)),
        "submenu lost its inert attribute; `.submenu[inert]` CSS would stop matching. Got: {html:?}"
    );

    let modal = regex_lite_find(
        &html,
        r#"<div id="gallery-modal" role="dialog" aria-modal="true" aria-label="About this component" inert="#,
    );
    assert!(
        modal.is_some_and(|tag| tag.contains(r#"class="modal""#)),
        "modal lost its inert attribute; `.modal[inert]` CSS would stop matching and the modal would render open"
    );
}

#[tokio::test]
async fn textarea_still_renders_with_an_explicit_closing_tag() {
    let html = gallery_html().await;

    // A self-closing <textarea /> is invalid HTML; Yew must expand it back out.
    assert!(
        html.contains(
            r#"<textarea id="gallery-message" placeholder="Tell us about your project…"></textarea>"#
        ),
        "textarea did not round-trip to an open/close pair"
    );
}

/// Returns the remainder of the tag opened by `prefix`, up to its closing `>`.
/// Avoids a regex dependency for what is a single prefix-then-scan lookup.
fn regex_lite_find<'a>(html: &'a str, prefix: &str) -> Option<&'a str> {
    let start = html.find(prefix)?;
    let tag = &html[start..];
    let end = tag.find('>')?;
    Some(&tag[..=end])
}

#[test]
fn stylesheets_ship_with_the_crate() {
    // Consumers (the portal's SSG, a Trunk app, the Streamlit bridge) get the CSS
    // from the crate rather than reaching into its assets/ by relative path.
    assert!(
        eona_ui_toolkit::TOKENS_CSS.contains(":root"),
        "tokens.css should define custom properties on :root"
    );
    assert!(
        eona_ui_toolkit::COMPONENTS_CSS.len() > 1000,
        "components.css looks truncated"
    );
}
