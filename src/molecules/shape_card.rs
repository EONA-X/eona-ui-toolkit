use yew::prelude::*;

/// `.shape-card` — one entry in the shape & component conventions strip.
#[derive(Properties, PartialEq)]
pub struct ShapeCardProps {
    pub demo: Html,
    pub title: AttrValue,
    pub description: AttrValue,
}

#[function_component(ShapeCard)]
pub fn shape_card(props: &ShapeCardProps) -> Html {
    html! {
        <div class="shape-card">
            <div class="shape-demo">{ props.demo.clone() }</div>
            <h4>{ &props.title }</h4>
            <p>{ &props.description }</p>
        </div>
    }
}
