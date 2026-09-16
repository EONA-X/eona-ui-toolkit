use yew::prelude::*;

use crate::organisms::main_nav::NavLink;

/// `footer.icons-footer` — the icons page's short one-line footer.
#[derive(Properties, PartialEq)]
pub struct SimpleFooterProps {
    pub text: AttrValue,
    pub link: NavLink,
}

#[function_component(SimpleFooter)]
pub fn simple_footer(props: &SimpleFooterProps) -> Html {
    html! {
        <footer class="icons-footer">
            <div class="wrap bottom-line">
                <span>{ &props.text }</span>
                <span><a href={props.link.href.clone()}>{ "\u{2190} " }{ &props.link.label }</a></span>
            </div>
        </footer>
    }
}
