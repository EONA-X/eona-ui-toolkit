use yew::prelude::*;

/// `.form__css` field on eona-x.eu — a floating-label text input, the shape
/// used by the site's Gravity Forms contact form.
#[derive(Clone, Copy, PartialEq)]
pub enum TextFieldKind {
    Input,
    Textarea,
}

#[derive(Properties, PartialEq)]
pub struct TextFieldProps {
    pub id: AttrValue,
    pub label: AttrValue,
    #[prop_or(TextFieldKind::Input)]
    pub kind: TextFieldKind,
    #[prop_or_default]
    pub placeholder: AttrValue,
}

#[function_component(TextField)]
pub fn text_field(props: &TextFieldProps) -> Html {
    html! {
        <div class="text-field">
            <label for={props.id.clone()}>{ &props.label }</label>
            if props.kind == TextFieldKind::Textarea {
                <textarea id={props.id.clone()} placeholder={props.placeholder.clone()} />
            } else {
                <input id={props.id.clone()} type="text" placeholder={props.placeholder.clone()} />
            }
        </div>
    }
}
