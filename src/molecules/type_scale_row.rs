use yew::prelude::*;

/// `.type-scale-row` — one row of the type scale table (role, a live sample
/// at that size/weight, and the spec string). `sample` carries its own
/// inline style, since every row's sample is styled differently.
#[derive(Properties, PartialEq)]
pub struct TypeScaleRowProps {
    pub role: AttrValue,
    pub sample: Html,
    pub spec: AttrValue,
}

#[function_component(TypeScaleRow)]
pub fn type_scale_row(props: &TypeScaleRowProps) -> Html {
    html! {
        <div class="type-scale-row">
            <div class="role">{ &props.role }</div>
            { props.sample.clone() }
            <div class="spec">{ &props.spec }</div>
        </div>
    }
}
