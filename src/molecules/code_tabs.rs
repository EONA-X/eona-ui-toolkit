use yew::prelude::*;

/// A source snippet in three stacks: YEW | SVELTE | REACT tabs over one
/// `.code-block`, with the language chip and Copy on the bar.
///
/// Presentational like [`crate::CodeBlock`]: no handlers. Rendered as it is, the
/// Yew panel is shown, the other two are `hidden`, and the tablist is `hidden`
/// too, so without JavaScript the block reads as a Yew snippet with a `rust`
/// chip. `assets/demo-page.js` unhides the tablist and does the rest: the
/// WAI-ARIA tab behaviour, one selection shared by every block on the page
/// (remembered under `eona-ui-toolkit:stack`), and Copy of the active panel.
///
/// A stack with no version passes a `*_absent` note instead of code. Its panel
/// shows the note, holds no `<code>`, and the script disables Copy while it is
/// active.
#[derive(Properties, PartialEq)]
pub struct CodeTabsProps {
    /// Prefix of the tab and panel ids, which pair through `aria-controls` and
    /// `aria-labelledby`. Must be unique on the page.
    pub id: AttrValue,
    /// The Yew source, rendered verbatim inside `<pre>`.
    pub yew: AttrValue,
    /// The Svelte source. Ignored when `svelte_absent` is set.
    #[prop_or_default]
    pub svelte: AttrValue,
    /// The React source. Ignored when `react_absent` is set.
    #[prop_or_default]
    pub react: AttrValue,
    /// Why there is no Svelte version, and what to do instead.
    #[prop_or_default]
    pub svelte_absent: Option<AttrValue>,
    /// Why there is no React version, and what to do instead.
    #[prop_or_default]
    pub react_absent: Option<AttrValue>,
}

#[function_component(CodeTabs)]
pub fn code_tabs(props: &CodeTabsProps) -> Html {
    let id = &props.id;
    let tab = |stack: &'static str, label: &'static str, selected: bool| {
        html! {
            <button
                type="button"
                role="tab"
                class="code-tabs-tab"
                id={format!("{id}-tab-{stack}")}
                aria-controls={format!("{id}-panel-{stack}")}
                aria-selected={if selected { "true" } else { "false" }}
                tabindex={if selected { "0" } else { "-1" }}
                data-stack={stack}
            >{ label }</button>
        }
    };

    html! {
        <div class="code-block code-tabs">
            <div class="code-block-bar">
                <div class="code-tabs-list" role="tablist" aria-label="Stack" hidden=true>
                    { tab("yew", "YEW", true) }
                    { tab("svelte", "SVELTE", false) }
                    { tab("react", "REACT", false) }
                </div>
                <span class="code-block-lang code-tabs-lang">{ "rust" }</span>
                <button type="button" class="code-block-copy" aria-label="Copy the active snippet">
                    { "Copy" }
                </button>
            </div>
            <div
                role="tabpanel"
                id={format!("{id}-panel-yew")}
                aria-labelledby={format!("{id}-tab-yew")}
                data-stack="yew"
            >
                <pre class="code-block-pre"><code class="language-rust">{ props.yew.clone() }</code></pre>
            </div>
            <div
                role="tabpanel"
                id={format!("{id}-panel-svelte")}
                aria-labelledby={format!("{id}-tab-svelte")}
                data-stack="svelte"
                hidden=true
            >
                if let Some(note) = &props.svelte_absent {
                    <p class="snippet-absent">{ note.clone() }</p>
                } else {
                    <pre class="code-block-pre"><code class="language-svelte">{ props.svelte.clone() }</code></pre>
                }
            </div>
            <div
                role="tabpanel"
                id={format!("{id}-panel-react")}
                aria-labelledby={format!("{id}-tab-react")}
                data-stack="react"
                hidden=true
            >
                if let Some(note) = &props.react_absent {
                    <p class="snippet-absent">{ note.clone() }</p>
                } else {
                    <pre class="code-block-pre"><code class="language-tsx">{ props.react.clone() }</code></pre>
                }
            </div>
        </div>
    }
}
