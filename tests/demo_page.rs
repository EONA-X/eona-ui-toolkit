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
        ("code-tabs", "CodeTabs stack tabs"),
        ("role=\"tablist\"", "CodeTabs: the tablist the script unhides"),
        ("role=\"tabpanel\"", "CodeTabs: the panels the script shows and hides"),
        ("data-stack", "CodeTabs: tab-to-panel pairing"),
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

#[test]
fn page_glue_disables_copy_and_remembers_the_stack() {
    // Neither shows up in the server-rendered markup: the script sets one and
    // names the other. tests/demo-page covers what they do.
    for hook in ["aria-disabled", "eona-ui-toolkit:stack"] {
        assert!(DEMO_JS.contains(hook), "demo-page.js no longer mentions `{hook}`");
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

    // Every row is one CodeTabs block with three panels. A stack that does not
    // ship the component gets a note in its panel instead of code. Counted off
    // the same table the page renders from, so the expected numbers move with
    // the verify gate rather than with this file.
    use eona_ui_toolkit::examples::StackSnippet;
    let absent = eona_ui_toolkit::examples::SNIPPETS
        .iter()
        .flat_map(|s| [&s.react, &s.svelte])
        .filter(|s| matches!(s, StackSnippet::Unavailable { .. }))
        .count();
    let rows = eona_ui_toolkit::examples::SNIPPETS.len();
    let section = snippets_section(&html);

    assert_eq!(
        section.matches(r#"class="snippet-absent""#).count(),
        absent,
        "one honest note per stack that does not ship the component"
    );
    assert_eq!(section.matches(r#"role="tablist""#).count(), rows, "one tablist per row");
    assert_eq!(section.matches(r#"role="tabpanel""#).count(), rows * 3, "three panels per row");
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

/// Operator decision OC-4: every code bar of the components page carries the
/// YEW | SVELTE | REACT tabs. The integration guide documents the crate itself
/// (install commands, stylesheets, vendoring), so its plain `CodeBlock`s stay.
#[tokio::test]
async fn every_component_example_code_bar_is_a_code_tabs() {
    let html = page_html().await;
    let guide_start = html.find(r#"id="scope""#).expect("integration guide start");
    let guide_end = html.find(r#"id="ontology""#).expect("ontology section start");
    assert!(guide_start < guide_end);
    let outside = format!("{}{}", &html[..guide_start], &html[guide_end..]);
    let plain = outside.matches(r#"class="code-block""#).count();
    assert_eq!(plain, 0, "{plain} plain CodeBlock(s) outside the integration guide");
    assert!(outside.matches(r#"class="code-block code-tabs""#).count() >= 12);
}

// ---- YEW | SVELTE | REACT tabs --------------------------------------------

/// Section 10, from its opening tag to its closing one.
fn snippets_section(html: &str) -> &str {
    let start = html.find(r#"id="snippets""#).expect("section 10 is missing");
    let end = start + html[start..].find("</section>").expect("section 10 never closes");
    &html[start..end]
}

/// Every opening tag in `html` that contains `needle`.
fn tags_with<'a>(html: &'a str, needle: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        let tag_end = rest[i..].find('>').map(|j| i + j + 1).unwrap_or(rest.len());
        let tag = &rest[i..tag_end];
        if tag.contains(needle) {
            out.push(tag);
        }
        rest = &rest[tag_end..];
    }
    out
}

/// The rows of section 10. Attribute order in the rendered tags is Yew's, not the source's.
fn entries_of(section: &str) -> Vec<&str> {
    let rows: Vec<&str> = section.split(r#"<article id="snippet-"#).skip(1).collect();
    assert_eq!(rows.len(), eona_ui_toolkit::examples::SNIPPETS.len(), "one row per component");
    rows
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    Some(&tag[i..i + tag[i..].find('"')?])
}

fn is_hidden(tag: &str) -> bool {
    tag.contains(" hidden")
}

fn unescape(s: &str) -> String {
    s.replace("&#x27;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The part of `html` that holds one block's markup: from its `class="code-block code-tabs`
/// opening to the next one (or the end).
fn blocks(html: &str) -> Vec<&str> {
    let marker = r#"class="code-block code-tabs"#;
    let starts: Vec<usize> = html.match_indices(marker).map(|(i, _)| i).collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &s)| &html[s..*starts.get(n + 1).unwrap_or(&html.len())])
        .collect()
}

/// The inner HTML of the tabpanel whose `data-stack` is `stack`, up to the next panel.
fn panel<'a>(block: &'a str, stack: &str) -> &'a str {
    let key = format!(r#"data-stack="{stack}""#);
    let at = block
        .match_indices(r#"role="tabpanel""#)
        .map(|(i, _)| i)
        .find(|&i| {
            let tag_start = block[..i].rfind('<').unwrap();
            block[tag_start..i + block[i..].find('>').unwrap()].contains(&key)
        })
        .unwrap_or_else(|| panic!("no {stack} panel"));
    let end = block[at + 1..]
        .find(r#"role="tabpanel""#)
        .map(|j| at + 1 + j)
        .unwrap_or(block.len());
    &block[at..end]
}

#[tokio::test]
async fn every_section_10_entry_has_a_hidden_tablist_and_three_panels() {
    let html = page_html().await;
    let section = snippets_section(&html);

    for entry in entries_of(section) {
        let lists = tags_with(entry, r#"role="tablist""#);
        assert_eq!(lists.len(), 1, "one tablist per entry");
        assert!(is_hidden(lists[0]), "the tablist is hidden until the script unhides it: {}", lists[0]);

        let panels = tags_with(entry, r#"role="tabpanel""#);
        assert_eq!(panels.len(), 3);
        let hidden: Vec<bool> = panels.iter().map(|p| is_hidden(p)).collect();
        assert_eq!(hidden, [false, true, true], "Yew shows, Svelte and React wait");
    }
}

#[tokio::test]
async fn tabs_and_panels_pair_up_in_yew_svelte_react_order() {
    let html = page_html().await;
    let section = snippets_section(&html);

    for entry in entries_of(section) {
        let tabs = tags_with(entry, r#"role="tab""#);
        let panels = tags_with(entry, r#"role="tabpanel""#);
        assert_eq!(tabs.len(), 3);
        let order: Vec<_> = tabs.iter().map(|t| attr(t, "data-stack").unwrap()).collect();
        assert_eq!(order, ["yew", "svelte", "react"]);
        let labels: Vec<&str> = entry
            .split(r#"role="tab""#)
            .skip(1)
            .map(|c| c.split('>').nth(1).unwrap().split('<').next().unwrap())
            .collect();
        assert_eq!(labels, ["YEW", "SVELTE", "REACT"]);

        for (tab, panel) in tabs.iter().zip(&panels) {
            assert_eq!(attr(tab, "aria-controls"), attr(panel, "id"), "{tab} / {panel}");
            assert_eq!(attr(panel, "aria-labelledby"), attr(tab, "id"), "{tab} / {panel}");
            assert_eq!(attr(tab, "data-stack"), attr(panel, "data-stack"));
        }
        let selected: Vec<_> = tabs.iter().map(|t| attr(t, "aria-selected").unwrap()).collect();
        assert_eq!(selected, ["true", "false", "false"]);
        let tabindex: Vec<_> = tabs.iter().map(|t| attr(t, "tabindex").unwrap()).collect();
        assert_eq!(tabindex, ["0", "-1", "-1"], "roving tabindex: only the selected tab is in the tab order");
    }
}

#[tokio::test]
async fn tab_ids_are_unique_across_the_page() {
    let html = page_html().await;
    let mut seen = std::collections::BTreeSet::new();
    for tag in tags_with(&html, r#"role="tab""#).into_iter().chain(tags_with(&html, r#"role="tabpanel""#)) {
        let id = attr(tag, "id").unwrap_or_else(|| panic!("no id: {tag}"));
        assert!(seen.insert(id.to_string()), "duplicate id {id}");
    }
}

#[tokio::test]
async fn a_wasm_only_component_explains_itself_in_the_react_panel() {
    let html = page_html().await;
    let section = snippets_section(&html);
    let at = section.find(r#"id="snippet-BarChart""#).expect("BarChart has a row");
    let entry = &section[at..];
    let entry = &entry[..entry.find("</article>").unwrap()];
    let react = unescape(panel(entry, "react"));
    assert!(react.contains("snippet-absent"), "{react}");
    assert!(react.contains("not planned"), "the disposition text is shown: {react}");
    assert!(react.contains("eona-x/backlog#1219"), "tracked_by is shown: {react}");
    assert!(!react.contains("<code"), "an Unavailable panel has nothing to copy: {react}");
    // The Svelte call is real.
    assert!(unescape(panel(entry, "svelte")).contains("CubeChart"));
}

#[tokio::test]
async fn usage_examples_carry_the_generated_calls_of_the_components_they_use() {
    let html = page_html().await;
    let gallery_end = html.find(r#"id="snippets""#).unwrap();
    let examples = blocks(&html[..gallery_end]);
    assert!(examples.len() >= 10, "sections 01-09 have a tabbed bar each, got {}", examples.len());

    // The first example is the three PillButtons.
    let first = examples[0];
    assert!(first.contains("PillButtonKind::Solid"), "the Yew tab keeps the hand-arranged source");
    let svelte = unescape(panel(first, "svelte"));
    assert_eq!(svelte.matches("import {").count(), 1, "one import line: {svelte}");
    assert!(svelte.contains("import { PillButton } from '@eona-x/ui-toolkit-svelte';"), "{svelte}");
    assert!(svelte.contains("<PillButton"), "{svelte}");
    let react = unescape(panel(first, "react"));
    assert_eq!(react.matches("import {").count(), 1, "one import line: {react}");
    assert!(react.contains("@eona-x/ui-toolkit-react"), "{react}");

    // An example that uses two components: still one import line, both names.
    let two = examples
        .iter()
        .map(|b| unescape(panel(b, "svelte")))
        .find(|p| p.contains("<NewsCard") && p.contains("<CaseCard"))
        .expect("the news and case cards share one example");
    assert_eq!(two.matches("import {").count(), 1, "{two}");
    assert!(two.contains("CaseCard, NewsCard") || two.contains("NewsCard, CaseCard"), "{two}");
}
