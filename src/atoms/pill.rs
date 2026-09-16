use yew::prelude::*;

/// `.name-pill` from the "Naming" section — a correct/incorrect usage chip
/// (e.g. "✓ EONA-X" vs "✕ Eona-x").
#[derive(Clone, Copy, PartialEq)]
pub enum PillTone {
    Ok,
    Bad,
}

impl PillTone {
    fn class(self) -> &'static str {
        match self {
            PillTone::Ok => "ok",
            PillTone::Bad => "bad",
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct PillProps {
    pub children: Children,
    #[prop_or(PillTone::Ok)]
    pub tone: PillTone,
}

#[function_component(Pill)]
pub fn pill(props: &PillProps) -> Html {
    html! {
        <span class={classes!("name-pill", props.tone.class())}>
            { for props.children.iter() }
        </span>
    }
}
