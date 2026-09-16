use yew::prelude::*;

#[derive(Clone, PartialEq)]
pub struct NavLink {
    pub href: AttrValue,
    pub label: AttrValue,
}

impl NavLink {
    pub fn new(href: &'static str, label: &'static str) -> Self {
        Self { href: href.into(), label: label.into() }
    }
}

/// `nav.mainnav` — brandmark, section/page links, and the fixed
/// tokens.json/tokens.css utility links, shared by both pages.
#[derive(Properties, PartialEq)]
pub struct MainNavProps {
    /// The "← Design system" back-link icons.html adds before its own links.
    #[prop_or_default]
    pub back: Option<NavLink>,
    pub links: Vec<NavLink>,
}

#[function_component(MainNav)]
pub fn main_nav(props: &MainNavProps) -> Html {
    html! {
        <nav class="mainnav">
            <div class="wrap">
                <div class="brandmark">
                    <span class="dot"></span>
                    { "\u{a0}\u{a0}EONA-X" }
                </div>
                <div class="navlinks">
                    if let Some(back) = &props.back {
                        <a class="back" href={back.href.clone()}>{ "\u{2190} " }{ &back.label }</a>
                    }
                    { for props.links.iter().map(|l| html! {
                        <a href={l.href.clone()}>{ &l.label }</a>
                    }) }
                </div>
                <div class="navspacer"></div>
                <div class="navutil">
                    <a href="tokens.json">{ "tokens.json" }</a>
                    <a href="tokens.css">{ "tokens.css" }</a>
                </div>
            </div>
        </nav>
    }
}
