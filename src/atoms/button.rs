use yew::prelude::*;

/// Mirrors `.btn-primary` / `.btn-ghost` from `brand/index.html`'s hero CTAs.
#[derive(Clone, Copy, PartialEq)]
pub enum ButtonVariant {
    Primary,
    Ghost,
}

impl ButtonVariant {
    fn class(self) -> &'static str {
        match self {
            ButtonVariant::Primary => "btn btn-primary",
            ButtonVariant::Ghost => "btn btn-ghost",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct ButtonProps {
    pub label: AttrValue,
    #[prop_or(ButtonVariant::Primary)]
    pub variant: ButtonVariant,
    /// Renders as an `<a>` when set, a `<button>` otherwise.
    #[prop_or_default]
    pub href: Option<AttrValue>,
}

#[function_component(Button)]
pub fn button(props: &ButtonProps) -> Html {
    let class = props.variant.class();
    match &props.href {
        Some(href) => html! { <a class={class} href={href.clone()}>{ &props.label }</a> },
        None => html! { <button class={class} type="button">{ &props.label }</button> },
    }
}
