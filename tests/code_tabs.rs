//! `CodeTabs` on its own: the markup the page glue (`assets/demo-page.js`) binds to.

use eona_ui_toolkit::CodeTabs;
use yew::{prelude::*, ServerRenderer};

#[derive(Properties, PartialEq)]
struct Notes {
    svelte: Option<&'static str>,
    react: Option<&'static str>,
}

#[function_component(Wrapper)]
fn wrapper(n: &Notes) -> Html {
    html! {
        <CodeTabs
            id="t1"
            yew="<A />"
            svelte="<A />"
            react="<A />"
            svelte_absent={n.svelte.map(AttrValue::Static)}
            react_absent={n.react.map(AttrValue::Static)}
        />
    }
}

async fn render(svelte: Option<&'static str>, react: Option<&'static str>) -> String {
    ServerRenderer::<Wrapper>::with_props(move || Notes { svelte, react }).render().await
}

#[tokio::test]
async fn renders_a_hidden_tablist_a_language_chip_copy_and_three_panels() {
    let html = render(None, None).await;
    assert!(html.contains(r#"class="code-block code-tabs""#), "{html}");
    assert!(html.contains(r#"role="tablist""#));
    assert_eq!(html.matches(r#"role="tabpanel""#).count(), 3);
    assert_eq!(html.matches("<code class=\"language-").count(), 3);
    assert!(html.contains("code-block-lang"), "the chip is there for the no-JS case");
    assert!(html.contains("code-block-copy"));
    assert!(!html.contains("aria-disabled"), "Yew is active, so Copy is live: {html}");
    assert!(html.contains(r#"id="t1-tab-svelte""#) && html.contains(r#"aria-controls="t1-panel-svelte""#));
}

#[tokio::test]
async fn an_absent_stack_shows_its_note_and_no_code() {
    let html = render(Some("Svelte note"), Some("React note")).await;
    assert_eq!(html.matches("<code class=\"language-").count(), 1, "only Yew has code: {html}");
    assert!(html.contains("Svelte note") && html.contains("React note"));
    assert_eq!(html.matches(r#"class="snippet-absent""#).count(), 2);
    assert_eq!(html.matches(r#"role="tabpanel""#).count(), 3);
}
