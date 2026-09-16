use yew::prelude::*;

/// `.hero` (index page) vs `header.libhero` (icons page) — same structure,
/// different size/decoration. Both pages' `<h1>` follows the exact same
/// "The <em>EONA-X</em> {suffix}" pattern, so that prefix is baked in here
/// rather than exposed as a prop.
#[derive(Clone, Copy, PartialEq)]
pub enum HeroVariant {
    /// Index page's big hero: decorative mesh-of-circles SVG background.
    Full,
    /// Icons page's smaller "libhero": no mesh background.
    Compact,
}

impl HeroVariant {
    fn class(self) -> &'static str {
        match self {
            HeroVariant::Full => "hero",
            HeroVariant::Compact => "libhero",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct HeroProps {
    pub variant: HeroVariant,
    pub eyebrow: AttrValue,
    pub title_suffix: AttrValue,
    pub lede: AttrValue,
    /// Rendered inside `.hero-actions` — buttons, links, a count label, etc.
    pub children: Children,
}

#[function_component(Hero)]
pub fn hero(props: &HeroProps) -> Html {
    html! {
        <header class={props.variant.class()}>
            <div class="gold-stripe"></div>
            if props.variant == HeroVariant::Full {
                <svg class="mesh-bg" viewBox="0 0 1180 520" preserveAspectRatio="xMaxYMid slice" aria-hidden="true">
                    <circle cx="960" cy="140" r="150" fill="#BFD1FE" opacity=".14" />
                    <circle cx="1080" cy="260" r="190" fill="#006EF0" opacity=".16" />
                    <circle cx="880" cy="300" r="120" fill="#040553" opacity=".35" />
                    <circle cx="1000" cy="380" r="90" fill="#B5C8F3" opacity=".12" />
                    <circle cx="1140" cy="120" r="70" fill="#FFD047" opacity=".10" />
                    <line x1="960" y1="140" x2="1080" y2="260" stroke="#BFD1FE" stroke-opacity=".25" stroke-width="1" />
                    <line x1="1080" y1="260" x2="880" y2="300" stroke="#BFD1FE" stroke-opacity=".25" stroke-width="1" />
                    <line x1="880" y1="300" x2="1000" y2="380" stroke="#BFD1FE" stroke-opacity=".2" stroke-width="1" />
                </svg>
            }
            <div class="wrap">
                <span class="hero-eyebrow"><span class="sw"></span>{ " " }{ &props.eyebrow }</span>
                <h1>{ "The " }<em>{ "EONA-X" }</em>{ " " }{ &props.title_suffix }</h1>
                <p class="lede">{ &props.lede }</p>
                <div class="hero-actions">
                    { for props.children.iter() }
                </div>
            </div>
        </header>
    }
}
