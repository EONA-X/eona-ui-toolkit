use yew::prelude::*;

/// `.mvo-card` — a mission/vision/objective card in the Positioning section.
#[derive(Properties, PartialEq)]
pub struct MvoCardProps {
    pub tag: AttrValue,
    pub description: AttrValue,
}

#[function_component(MvoCard)]
pub fn mvo_card(props: &MvoCardProps) -> Html {
    html! {
        <div class="mvo-card">
            <span class="mvo-tag"><span class="dot"></span>{ &props.tag }</span>
            <p>{ &props.description }</p>
        </div>
    }
}
