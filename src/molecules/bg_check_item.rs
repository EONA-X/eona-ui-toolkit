use yew::prelude::*;

/// `.bg-check` — one swatch in the logo-legibility-against-backgrounds grid.
#[derive(Properties, PartialEq)]
pub struct BgCheckItemProps {
    pub label: AttrValue,
    pub background: AttrValue,
    pub color: AttrValue,
    /// "✓" / "!" / "✕"
    pub icon: AttrValue,
    #[prop_or_default]
    pub border: Option<AttrValue>,
}

#[function_component(BgCheckItem)]
pub fn bg_check_item(props: &BgCheckItemProps) -> Html {
    let mut style = format!("background:{};color:{};", props.background, props.color);
    if let Some(border) = &props.border {
        style.push_str(&format!("border:{};", border));
    }
    html! {
        <div class="bg-check" style={style}>
            <span class="ic">{ &props.icon }</span>
            { &props.label }
        </div>
    }
}
