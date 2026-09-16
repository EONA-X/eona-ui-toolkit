use yew::prelude::*;

/// `.card-member` on eona-x.eu — a small frosted-glass tile for a
/// partner/member logo.
#[derive(Properties, PartialEq)]
pub struct MemberCardProps {
    pub name: AttrValue,
}

#[function_component(MemberCard)]
pub fn member_card(props: &MemberCardProps) -> Html {
    html! {
        <div class="member-card">
            <div class="member-card__logo">{ &props.name }</div>
        </div>
    }
}
