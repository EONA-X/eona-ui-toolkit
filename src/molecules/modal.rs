use yew::prelude::*;

/// `.modal`/`.modal-inner` on eona-x.eu — a fixed, dark-backdrop overlay
/// with a centered white panel, toggled via the `inert` attribute (present
/// = closed, matching `.modal[inert]`'s CSS). No live instance of this was
/// found on the pages fetched while extracting these components (home/about/
/// contact) to copy content from verbatim — the classes and open/closed
/// mechanics below are real, sourced from eona-x.eu's own `app.css`, but the
/// demo content is this crate's own minimal "read more" example. Toggling is
/// page-glue JS reading `data-open-modal` / `data-close-modal`, not a
/// component concern, same as `NavDropdown`.
#[derive(Properties, PartialEq)]
pub struct ModalProps {
    /// Whether the dialog is open. Closed renders `inert`, which is what
    /// `.modal[inert]` keys off.
    ///
    /// A prop rather than a constant: hardcoding `inert` meant the open state
    /// could not be rendered at all, so the React generator had no way to
    /// enumerate it and withheld the component. See eona-x/backlog#822.
    #[prop_or_default]
    pub open: bool,
    pub id: AttrValue,
    pub title: AttrValue,
    pub children: Children,
}

#[function_component(Modal)]
pub fn modal(props: &ModalProps) -> Html {
    html! {
        <div id={props.id.clone()} role="dialog" aria-modal="true" aria-label={props.title.clone()} inert={!props.open} class="modal">
            <div class="modal-inner">
                <button type="button" data-close-modal={props.id.clone()} aria-label="Close" class="submenu-close">
                    { "\u{2715}" }
                </button>
                <h3>{ &props.title }</h3>
                { for props.children.iter() }
            </div>
        </div>
    }
}
