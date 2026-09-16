//! Guards the demo page's three cross-file contracts, none of which the
//! compiler can see:
//!
//! 1. `DEMO_SHELL` is a template with an `__APP__` hole and a fixed list of
//!    asset names. A host writes those assets out by those exact names.
//! 2. `DEMO_JS` selects the rendered markup by class and `data-` attribute.
//!    Every component in the toolkit is presentational, so if a component is
//!    renamed or restyled out from under a selector, nothing fails to compile —
//!    the page just silently stops responding to clicks.
//! 3. `DemoPage` is every part at once: the gallery, the integration guide and
//!    the ontology-tier sections. Rendering only some is the failure mode of a
//!    bad merge.

use eona_ui_toolkit::demo::{DemoPage, DEMO_JS, DEMO_SHELL};
use yew::ServerRenderer;

async fn page_html() -> String {
    ServerRenderer::<DemoPage>::new().render().await
}

#[test]
fn shell_has_exactly_one_app_placeholder() {
    assert_eq!(
        DEMO_SHELL.matches("__APP__").count(),
        1,
        "DEMO_SHELL must have exactly one __APP__ hole; `replace` would otherwise \
         drop the body or duplicate it"
    );
}

#[test]
fn shell_links_the_assets_a_host_is_told_to_write() {
    // These names are the integration guide's instructions to consumers
    // (`IntegrationSection`, sections 12 and 13). Renaming one here without
    // renaming it there ships a page that 404s its own stylesheet.
    for asset in ["tokens.css", "components.css", "ontology.css", "demo-page.js"] {
        assert!(
            DEMO_SHELL.contains(asset),
            "DEMO_SHELL no longer links {asset}, which hosts are documented to write out"
        );
    }
}

#[tokio::test]
async fn page_renders_the_gallery_and_the_integration_guide() {
    let html = page_html().await;

    for id in ["buttons", "modal", "member-directory"] {
        assert!(html.contains(&format!(r#"id="{id}""#)), "gallery section #{id} is missing");
    }
    // The guide is two halves — `install`/`stylesheets`/`render`/`vendoring` are
    // Yew, the rest React — and a page missing one half is the failure mode of a
    // merge that kept only the stack whoever merged it was working in.
    for id in ["scope", "install", "stylesheets", "render", "vendoring"] {
        assert!(html.contains(&format!(r#"id="{id}""#)), "integration section #{id} is missing");
    }
    for id in ["npm", "react-stylesheets", "server-components", "react-gaps", "generated"] {
        assert!(html.contains(&format!(r#"id="{id}""#)), "React integration section #{id} is missing");
    }
    for id in ["ontology", "theming", "interactive", "catalog"] {
        assert!(html.contains(&format!(r#"id="{id}""#)), "ontology-tier section #{id} is missing");
    }
}

#[tokio::test]
async fn page_glue_selectors_still_match_the_rendered_markup() {
    let html = page_html().await;

    // Each entry is a substring of a selector in assets/demo-page.js paired with
    // what it drives. Keep them in step with that file.
    let contracts = [
        ("data-dropdown-trigger", "NavDropdown open/close"),
        ("submenu-close", "NavDropdown's in-panel close button"),
        ("data-open-modal", "Modal open"),
        ("data-close-modal", "Modal close"),
        ("code-block-copy", "CodeBlock copy-to-clipboard"),
    ];

    for (hook, what) in contracts {
        assert!(
            DEMO_JS.contains(hook),
            "demo-page.js no longer selects `{hook}`, so {what} is dead"
        );
        assert!(
            html.contains(hook),
            "nothing on the demo page emits `{hook}` any more, so demo-page.js's \
             {what} handler binds to nothing"
        );
    }
}

#[tokio::test]
async fn every_copy_button_has_a_code_element_to_copy_from() {
    let html = page_html().await;

    // demo-page.js resolves a copy button to its snippet with
    // `btn.closest(".code-block").querySelector("code")` — a copy button
    // outside a `.code-block`, or one whose block holds no `<code>`, is a
    // button that does nothing when clicked.
    let buttons = html.matches("code-block-copy").count();
    let code_elements = html.matches("<code class=\"language-").count();
    assert!(buttons > 0, "the integration guide rendered no copyable snippets at all");
    assert!(
        code_elements >= buttons,
        "{buttons} copy buttons but only {code_elements} language-tagged <code> \
         elements: at least one button has nothing to copy"
    );
}

#[tokio::test]
async fn section_09_reaches_every_component_in_both_stacks() {
    let html = page_html().await;

    // Section 09 is generated from `SNIPPETS`, so the page cannot fall behind
    // `src/` by omission — but it can fall behind by *filtering*, which is what
    // this guards. A component that quietly stops being rendered here is a
    // component a React consumer has no documented call for.
    assert!(
        html.contains(r#"id="snippets""#),
        "the both-stacks section is missing entirely"
    );

    for snippet in eona_ui_toolkit::examples::SNIPPETS {
        assert!(
            html.contains(&format!(r#"id="snippet-{}""#, snippet.component)),
            "{} has no row in section 09",
            snippet.component
        );
    }

    // Every row carries a Yew pane; a row whose component ships in React
    // carries a second one, and a row whose component does not carries a note
    // instead. Counted off the same table the page renders from, so the
    // expected numbers move with the verify gate rather than with this file.
    let absent = eona_ui_toolkit::examples::SNIPPETS
        .iter()
        .filter(|s| {
            matches!(
                s.react,
                eona_ui_toolkit::examples::ReactSnippet::Unavailable { .. }
            )
        })
        .count();
    let rows = eona_ui_toolkit::examples::SNIPPETS.len();

    assert_eq!(
        html.matches(r#"class="snippet-absent""#).count(),
        absent,
        "one honest note per component React does not ship"
    );
    assert_eq!(
        html.matches(r#"class="snippet-pane">"#).count(),
        rows + (rows - absent),
        "one Yew pane per row, plus one React pane per row that has JSX"
    );
}

#[tokio::test]
async fn section_09_emits_no_class_the_stylesheet_does_not_define() {
    let html = page_html().await;

    // Section 09's markup is written here but its *content* is generated, so the
    // usual review reflex — read the diff, spot the typo — does not reach it.
    // Nothing in the toolkit fails when a class has no rule: the page just
    // renders unstyled, which is why this is a test and not a comment.
    //
    // Scoped to this section rather than the whole page: several ported
    // components carry structural class names from eona-x.eu's own markup that
    // were never styled here (`.term`, `.menu-item`), and `language-*` is a hook
    // for a highlighter the toolkit deliberately does not ship
    // (`src/atoms/code_block.rs:18`). Widening this assertion would mean
    // relitigating all of those.
    let start = html.find(r#"id="snippets""#).expect("section 09 is missing");
    let end = start + html[start..].find("</section>").expect("section 09 never closes");
    let section = &html[start..end];

    for chunk in section.split(r#" class=""#).skip(1) {
        // A leading space is what separates an attribute from `badge_class="…"`
        // inside a rendered snippet, which is text, not markup.
        let value = chunk.split('"').next().unwrap_or_default();
        for class in value.split_whitespace() {
            if class.starts_with("language-") {
                continue;
            }
            assert!(
                eona_ui_toolkit::COMPONENTS_CSS.contains(&format!(".{class}")),
                "section 09 emits `.{class}`, which components.css does not define"
            );
        }
    }
}
