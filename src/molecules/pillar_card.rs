use yew::prelude::*;

/// `.pillar-card` — one of the "3 pillars of positioning", a numbered dark
/// card with a bolded-label bullet list.
#[derive(Properties, PartialEq)]
pub struct PillarCardProps {
    pub num: AttrValue,
    pub title: AttrValue,
    pub points: Vec<(&'static str, &'static str)>,
}

#[function_component(PillarCard)]
pub fn pillar_card(props: &PillarCardProps) -> Html {
    html! {
        <div class="pillar-card">
            <span class="pillar-num">{ &props.num }</span>
            <h3>{ &props.title }</h3>
            <ul>
                { for props.points.iter().copied().map(|(label, text)| html! {
                    <li><b>{ label }{ ":" }</b>{ " " }{ text }</li>
                }) }
            </ul>
        </div>
    }
}
