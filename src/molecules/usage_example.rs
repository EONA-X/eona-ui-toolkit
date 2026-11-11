use yew::prelude::*;

use crate::molecules::CodeTabs;

/// Shows a component twice: rendered, then as the `html!` call that produced it.
///
/// The `source` is hand-written rather than reflected from `children` — Rust has
/// no way to recover a macro's own token text at runtime — so the two halves can
/// drift. They are kept literally adjacent in the call site for exactly that
/// reason: an edit to one that skips the other is visible in the diff.
///
/// The code bar is a [`CodeTabs`]: `source` is the YEW tab, `svelte` and `react`
/// are the other two, or a `*_absent` note when a stack has no version.
#[derive(Properties, PartialEq)]
pub struct UsageExampleProps {
    /// Prefix of the tab ids of the code bar, see [`CodeTabs`]. Unique on the page.
    pub id: AttrValue,
    /// The `html!` fragment, as it should be pasted into a consumer.
    pub source: AttrValue,
    /// Renders the preview on the deep-navy surface, for components designed to
    /// sit over dark imagery.
    #[prop_or_default]
    pub on_dark: bool,
    /// The Svelte version of the example. Ignored when `svelte_absent` is set.
    #[prop_or_default]
    pub svelte: AttrValue,
    /// The React version of the example. Ignored when `react_absent` is set.
    #[prop_or_default]
    pub react: AttrValue,
    /// Why there is no Svelte version.
    #[prop_or_default]
    pub svelte_absent: Option<AttrValue>,
    /// Why there is no React version.
    #[prop_or_default]
    pub react_absent: Option<AttrValue>,
    /// The live rendering.
    pub children: Html,
}

#[function_component(UsageExample)]
pub fn usage_example(props: &UsageExampleProps) -> Html {
    let preview = if props.on_dark {
        "usage-preview usage-preview-dark"
    } else {
        "usage-preview"
    };

    html! {
        <div class="usage-example">
            <div class={preview}>{ props.children.clone() }</div>
            <CodeTabs
                id={props.id.clone()}
                yew={props.source.clone()}
                svelte={props.svelte.clone()}
                react={props.react.clone()}
                svelte_absent={props.svelte_absent.clone()}
                react_absent={props.react_absent.clone()}
            />
        </div>
    }
}
