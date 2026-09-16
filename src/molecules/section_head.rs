use yew::prelude::*;

use crate::atoms::BadgeNum;

/// `.section-head` — the numbered-badge + kicker + `<h2>` header that opens
/// every top-level section of the design-system page.
#[derive(Properties, PartialEq)]
pub struct SectionHeadProps {
    pub number: AttrValue,
    pub kicker: AttrValue,
    pub title: AttrValue,
}

#[function_component(SectionHead)]
pub fn section_head(props: &SectionHeadProps) -> Html {
    html! {
        <div class="section-head">
            <BadgeNum number={props.number.clone()} />
            <div>
                <span class="kicker">{ &props.kicker }</span>
                <h2>{ &props.title }</h2>
            </div>
        </div>
    }
}
