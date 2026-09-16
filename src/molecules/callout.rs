use yew::prelude::*;

/// `.callout` — a French-quote + English-translation "key rule" box, reused
/// across the Color, Positioning, and Declinations sections.
#[derive(Properties, PartialEq)]
pub struct CalloutProps {
    pub label: AttrValue,
    pub french: AttrValue,
    pub english: AttrValue,
}

#[function_component(Callout)]
pub fn callout(props: &CalloutProps) -> Html {
    html! {
        <div class="callout">
            <b>{ &props.label }</b>{ " " }
            <span class="fr">{ &props.french }</span>
            { " " }{ &props.english }
        </div>
    }
}
