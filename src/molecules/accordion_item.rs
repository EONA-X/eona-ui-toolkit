use yew::prelude::*;

/// `.card-accordeon` on eona-x.eu — built on native `<details>`/`<summary>`,
/// so unlike `NavDropdown`/`Modal` this needs zero JS: the browser handles
/// open/close and `[open]` state on its own. The real markup also has an
/// icon swatch next to the title; skipped here (no icon asset to embed
/// without hotlinking eona-x.eu media).
#[derive(Properties, PartialEq)]
pub struct AccordionItemProps {
    pub title: AttrValue,
    pub children: Children,
    #[prop_or_default]
    pub open: bool,
}

#[function_component(AccordionItem)]
pub fn accordion_item(props: &AccordionItemProps) -> Html {
    html! {
        <details class="card-accordeon" open={props.open}>
            <summary class="card-accordeon-header">
                <div class="card-accordeon-header-inner">
                    <div class="card-accordeon-title-container">
                        <h3 class="card-accordeon-title">{ &props.title }</h3>
                    </div>
                </div>
                <div class="card-accordeon-arrow" aria-hidden="true">{ "\u{25be}" }</div>
            </summary>
            <div class="card-accordeon-content">
                { for props.children.iter() }
            </div>
        </details>
    }
}
