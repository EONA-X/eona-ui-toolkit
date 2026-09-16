use yew::prelude::*;

/// The circular numbered badge used at the head of each design-system section
/// (`.badge-num` in `brand/index.html`, e.g. section "02 — Color").
#[derive(Properties, PartialEq)]
pub struct BadgeNumProps {
    pub number: AttrValue,
}

#[function_component(BadgeNum)]
pub fn badge_num(props: &BadgeNumProps) -> Html {
    html! { <div class="badge-num">{ &props.number }</div> }
}
