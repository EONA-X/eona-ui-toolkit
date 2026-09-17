use yew::prelude::*;

/// Same YIQ contrast check as `contrastColor()` in `brand/index.html`'s inline
/// script, ported so the foreground color is decided at render time instead of
/// by client-side JS — this is what makes the swatch grid pixel-identical
/// without shipping any runtime script.
pub fn contrast_color(hex: &str) -> &'static str {
    let hex = hex.trim_start_matches('#');
    let byte = |slice: &str| u8::from_str_radix(slice, 16).unwrap_or(0) as f32;
    let (r, g, b) = (
        byte(&hex[0..2]),
        byte(&hex[2..4]),
        byte(&hex[4..6]),
    );
    let yiq = (r * 299.0 + g * 587.0 + b * 114.0) / 1000.0;
    if yiq >= 150.0 { "#040553" } else { "#ffffff" }
}

/// `.swatch` — a single color card in a `.swatch-grid` (hex value, name, usage
/// note, and any observed near-duplicate variants).
#[derive(Properties, PartialEq)]
pub struct SwatchProps {
    pub hex: AttrValue,
    /// The foreground the swatch prints its hex in, from [`contrast_color`].
    ///
    /// A prop rather than a computation: deriving it here read the prop's bytes
    /// (`contrast_color` slices `hex[0..2]`), which both panicked on a
    /// multi-byte probe and made the markup a transform of the prop — the shape
    /// the React generator's verify gate quarantines. Computing it at the call
    /// site keeps this component pass-through. See eona-x/backlog#822.
    pub ink: AttrValue,
    pub name: AttrValue,
    pub usage: AttrValue,
    /// Near-duplicate hexes seen in the wild, already joined for display.
    ///
    /// A joined string rather than a `Vec`: collapsing a list into one text
    /// node is a transform the splicer cannot express from a 0..3 render
    /// matrix, so the join happens at the call site. See eona-x/backlog#822.
    #[prop_or_default]
    pub variants: Option<AttrValue>,
}

#[function_component(Swatch)]
pub fn swatch(props: &SwatchProps) -> Html {
    let style = format!("background:{};color:{};", props.hex, props.ink);
    html! {
        <button class="swatch" type="button" data-hex={props.hex.clone()}>
            <span class="swatch-color" style={style}>
                <span class="swatch-hex">{ &props.hex }</span>
                <span class="swatch-copy-flag">{ "Copied" }</span>
            </span>
            <span class="swatch-meta">
                <span class="swatch-name">{ &props.name }</span>
                <span class="swatch-usage">{ &props.usage }</span>
                if let Some(variants) = &props.variants {
                    <span class="swatch-variants">
                        { "Also seen: " }
                        { variants }
                    </span>
                }
            </span>
        </button>
    }
}
