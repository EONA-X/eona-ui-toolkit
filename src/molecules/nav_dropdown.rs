use yew::prelude::*;

/// `.menu-link[aria-expanded]` + `.submenu` on eona-x.eu — a nav item that
/// expands into a dropdown panel (used for both the "EONA-X" nav item's
/// About/Our ecosystem/Dataspace links and the language switcher). Starts
/// closed; toggling is small page-glue JS (see `components-page.js`) reading
/// `data-dropdown-trigger` / `data-close`, not a component concern — same
/// division of labor as the swatch click-to-copy and declination filters
/// elsewhere in this project.
///
/// Desktop-only positioning is ported (the real site's dropdown becomes a
/// full-screen panel below ~64rem, via extra CSS this crate doesn't
/// reproduce); this always renders as the desktop dropdown.
#[derive(Properties, PartialEq)]
pub struct NavDropdownProps {
    pub id: AttrValue,
    pub label: AttrValue,
    pub links: Vec<(&'static str, &'static str)>,
}

#[function_component(NavDropdown)]
pub fn nav_dropdown(props: &NavDropdownProps) -> Html {
    html! {
        <div class="menu-item" style="position:relative;">
            <button
                type="button"
                aria-controls={props.id.clone()}
                aria-expanded="false"
                data-dropdown-trigger={props.id.clone()}
                class="menu-link"
            >
                { &props.label }
                <span class="menu-arrow" aria-hidden="true">{ "\u{25be}" }</span>
            </button>
            <div id={props.id.clone()} inert={true} class="submenu">
                <div class="submenu-content">
                    <ul class="submenu-list">
                        { for props.links.iter().copied().map(|(href, label)| html! {
                            <li class="submenu-item"><a class="submenu-link" href={href}>{ label }</a></li>
                        }) }
                    </ul>
                </div>
                <button type="button" data-close={props.id.clone()} aria-label="Close" class="submenu-close">
                    { "\u{2715}" }
                </button>
            </div>
        </div>
    }
}
