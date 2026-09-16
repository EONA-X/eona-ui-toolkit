use yew::prelude::*;

use crate::atoms::CodeBlock;

/// Shows a component twice: rendered, then as the `html!` call that produced it.
///
/// The `source` is hand-written rather than reflected from `children` — Rust has
/// no way to recover a macro's own token text at runtime — so the two halves can
/// drift. They are kept literally adjacent in the call site for exactly that
/// reason: an edit to one that skips the other is visible in the diff.
#[derive(Properties, PartialEq)]
pub struct UsageExampleProps {
    /// The `html!` fragment, as it should be pasted into a consumer.
    pub source: AttrValue,
    /// Renders the preview on the deep-navy surface, for components designed to
    /// sit over dark imagery.
    #[prop_or_default]
    pub on_dark: bool,
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
            <CodeBlock code={props.source.clone()} language="rust" />
        </div>
    }
}
