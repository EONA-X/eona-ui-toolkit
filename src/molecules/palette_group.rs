use yew::prelude::*;

use crate::molecules::Swatch;

#[derive(Clone, PartialEq)]
pub struct ColorSpec {
    pub name: AttrValue,
    pub hex: AttrValue,
    pub usage: AttrValue,
    pub variants: Vec<AttrValue>,
}

impl ColorSpec {
    pub fn new(name: &'static str, hex: &'static str, usage: &'static str, variants: &[&'static str]) -> Self {
        Self {
            name: name.into(),
            hex: hex.into(),
            usage: usage.into(),
            variants: variants.iter().map(|v| AttrValue::from(*v)).collect(),
        }
    }
}

/// `.palette-group` — a titled `.swatch-grid` of `Swatch`es, e.g. "Core palette"
/// or "Website dark neutrals".
#[derive(Properties, PartialEq)]
pub struct PaletteGroupProps {
    pub title: AttrValue,
    pub description: AttrValue,
    pub colors: Vec<ColorSpec>,
    /// `.ref-note` shown next to the title, e.g. "reference only — do not use for UI".
    #[prop_or_default]
    pub note: Option<AttrValue>,
}

#[function_component(PaletteGroup)]
pub fn palette_group(props: &PaletteGroupProps) -> Html {
    html! {
        <div class="palette-group">
            <h3>
                { &props.title }
                if let Some(note) = &props.note {
                    <>
                        { " " }
                        <span class="ref-note">{ note }</span>
                    </>
                }
            </h3>
            <p class="group-desc">{ &props.description }</p>
            <div class="swatch-grid">
                { for props.colors.iter().map(|c| html! {
                    <Swatch hex={c.hex.clone()} name={c.name.clone()} usage={c.usage.clone()} variants={c.variants.clone()} />
                }) }
            </div>
        </div>
    }
}
