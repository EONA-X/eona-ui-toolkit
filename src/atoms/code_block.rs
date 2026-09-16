use yew::prelude::*;

/// A source snippet: a language chip, a copy button, and the code itself in a
/// `<pre><code>` pair so whitespace survives.
///
/// Presentational like every other component here — the button carries no
/// handler, it is `assets/demo-page.js` that finds the sibling `<code>` and
/// writes it to the clipboard, the same division of labor as the design-system
/// page's swatch click-to-copy.
#[derive(Properties, PartialEq)]
pub struct CodeBlockProps {
    /// Rendered verbatim inside `<pre>`; indent it the way it should appear.
    pub code: AttrValue,
    /// Shown in the chip and mirrored onto the `<code>` element as
    /// `class="language-…"`, the convention syntax highlighters key on. No
    /// highlighter ships with the toolkit; the class is there for hosts that
    /// add one.
    #[prop_or(AttrValue::Static("rust"))]
    pub language: AttrValue,
    /// Drop the copy button for snippets nobody would paste — command output,
    /// directory trees.
    #[prop_or(true)]
    pub copyable: bool,
}

#[function_component(CodeBlock)]
pub fn code_block(props: &CodeBlockProps) -> Html {
    html! {
        <div class="code-block">
            <div class="code-block-bar">
                <span class="code-block-lang">{ props.language.clone() }</span>
                if props.copyable {
                    <button type="button" class="code-block-copy" aria-label="Copy this snippet">
                        { "Copy" }
                    </button>
                }
            </div>
            <pre class="code-block-pre"><code class={format!("language-{}", props.language)}>
                { props.code.clone() }
            </code></pre>
        </div>
    }
}
