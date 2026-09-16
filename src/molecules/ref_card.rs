use yew::prelude::*;

/// `.ref-card` — a captioned thumbnail figure pointing back at an original
/// source slide, used throughout the design-system page in `.ref-grid` rows.
#[derive(Properties, PartialEq)]
pub struct RefCardProps {
    pub src: AttrValue,
    pub alt: AttrValue,
    pub title: AttrValue,
    pub description: AttrValue,
}

#[function_component(RefCard)]
pub fn ref_card(props: &RefCardProps) -> Html {
    html! {
        <figure class="ref-card">
            <img src={props.src.clone()} loading="lazy" alt={props.alt.clone()} />
            <figcaption>
                <div class="rc-title">{ &props.title }</div>
                <div class="rc-desc">{ &props.description }</div>
            </figcaption>
        </figure>
    }
}
