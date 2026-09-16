use yew::prelude::*;

/// `.label-tags` on eona-x.eu — a small dot+text pill used on news/case cards.
#[derive(Clone, Copy, PartialEq)]
pub enum TagLabelTone {
    Default,
    Blue,
}

impl TagLabelTone {
    fn class(self) -> &'static str {
        match self {
            TagLabelTone::Default => "tag-label",
            TagLabelTone::Blue => "tag-label tag-label-blue",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct TagLabelProps {
    pub text: AttrValue,
    #[prop_or(TagLabelTone::Default)]
    pub tone: TagLabelTone,
}

#[function_component(TagLabel)]
pub fn tag_label(props: &TagLabelProps) -> Html {
    html! { <span class={props.tone.class()}>{ &props.text }</span> }
}
