use yew::prelude::*;

/// `.rules-list li.do/.dont/.caution` — a logo usage do/don't/caution line.
#[derive(Clone, Copy, PartialEq)]
pub enum RuleKind {
    Do,
    Dont,
    Caution,
}

impl RuleKind {
    fn class(self) -> &'static str {
        match self {
            RuleKind::Do => "do",
            RuleKind::Dont => "dont",
            RuleKind::Caution => "caution",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct RuleListItemProps {
    pub kind: RuleKind,
    pub children: Children,
}

#[function_component(RuleListItem)]
pub fn rule_list_item(props: &RuleListItemProps) -> Html {
    html! {
        <li class={props.kind.class()}>{ for props.children.iter() }</li>
    }
}

// This component cannot style itself. Every rule that gives it its glyph and its
// card is scoped under a `.rules-list` ancestor (assets/components.css:218-222),
// and the `<ul class="rules-list">` that used to wrap it went to
// developer.eona-x.eu with `logo_section.rs`. So a host must supply that element:
//
//     <ul class="rules-list"><RuleListItem kind={RuleKind::Do}>{ "..." }</RuleListItem></ul>
//
// Left as a contract rather than fixed here, because moving the class onto the
// `<li>` would change the markup the portal already renders — and because the
// React export has the same requirement, it is stated in the package README too
// (xtask/src/emit.rs, the assets paragraph).
