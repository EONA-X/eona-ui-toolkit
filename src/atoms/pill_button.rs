use yew::prelude::*;

/// `.css__button` on eona-x.eu — a fully-rounded pill button in one of three
/// treatments: solid brand blue, glass-on-dark, or glass-on-light.
#[derive(Clone, Copy, PartialEq)]
pub enum PillButtonKind {
    Solid,
    GlassLight,
    GlassDark,
}

impl PillButtonKind {
    fn class(self) -> &'static str {
        match self {
            PillButtonKind::Solid => "pill-btn pill-btn-solid",
            PillButtonKind::GlassLight => "pill-btn pill-btn-glass-light",
            PillButtonKind::GlassDark => "pill-btn pill-btn-glass-dark",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct PillButtonProps {
    pub label: AttrValue,
    #[prop_or(PillButtonKind::Solid)]
    pub kind: PillButtonKind,
    /// Renders as an `<a>` when set, a `<button>` otherwise.
    #[prop_or_default]
    pub href: Option<AttrValue>,
    /// Renders a disabled `<button>` — used for the SiteHeader's
    /// not-yet-wired-up login placeholder.
    #[prop_or_default]
    pub disabled: bool,
}

#[function_component(PillButton)]
pub fn pill_button(props: &PillButtonProps) -> Html {
    let class = props.kind.class();
    match &props.href {
        Some(href) => html! { <a class={class} href={href.clone()}>{ &props.label }</a> },
        None => html! {
            <button class={class} type="button" disabled={props.disabled}>{ &props.label }</button>
        },
    }
}
