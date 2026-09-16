use yew::prelude::*;

/// `.personality-card` — a brand-personality trait card.
#[derive(Properties, PartialEq)]
pub struct PersonalityCardProps {
    pub tag: AttrValue,
    pub description: AttrValue,
}

#[function_component(PersonalityCard)]
pub fn personality_card(props: &PersonalityCardProps) -> Html {
    html! {
        <div class="personality-card">
            <span class="tag">{ &props.tag }</span>
            <p>{ &props.description }</p>
        </div>
    }
}
