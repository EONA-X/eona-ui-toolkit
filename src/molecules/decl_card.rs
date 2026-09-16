use yew::prelude::*;

/// `.decl-card` — one item in the Declinations gallery. `category` maps to
/// the original's `data-cat` attribute, read by the page's filter-button
/// glue script.
#[derive(Properties, PartialEq)]
pub struct DeclCardProps {
    pub category: AttrValue,
    pub src: AttrValue,
    pub alt: AttrValue,
    pub decl_type: AttrValue,
    pub title: AttrValue,
    pub description: AttrValue,
}

#[function_component(DeclCard)]
pub fn decl_card(props: &DeclCardProps) -> Html {
    html! {
        <figure class="decl-card" data-cat={props.category.clone()}>
            <img src={props.src.clone()} loading="lazy" alt={props.alt.clone()} />
            <div class="decl-body">
                <span class="decl-type">{ &props.decl_type }</span>
                <div class="decl-title">{ &props.title }</div>
                <div class="decl-desc">{ &props.description }</div>
            </div>
        </figure>
    }
}
