//! The toolkit's own demo page — the component gallery plus the integration
//! guide, as one renderable document.
//!
//! It lives in the library, not in the site that publishes it, because it
//! documents *this crate*: the sections under [`IntegrationSection`] describe
//! the Cargo features, the stylesheet constants and the submodule layout, all
//! of which change here and would otherwise go stale in a downstream repo. A
//! host renders [`DemoPage`] into [`DEMO_SHELL`] and writes [`DEMO_JS`] beside
//! it; `developer.eona-x.eu` publishes the result as `components.html`.
//!
//! [`DemoPage`] takes no required props, so `ServerRenderer::<DemoPage>::new()`
//! works as-is. A host that wants the page to sit inside its own navigation
//! passes `nav`, `footer_text` and `footer_link`.

use yew::prelude::*;

use crate::atoms::{PillButton, PillButtonKind};
use crate::organisms::{
    ComponentsGallery, HeroFilter, IntegrationSection, NavLink, OntologySection, SimpleFooter,
    SiteHeader, SiteNavLink,
};

/// The HTML document the page renders into: `__APP__` marks where the rendered
/// body goes.
///
/// Links five assets by relative name. Four are crate constants —
/// `tokens.css`, `components.css`, `ontology.css` and `demo-page.js`
/// ([`crate::TOKENS_CSS`], [`crate::COMPONENTS_CSS`], [`crate::ONTOLOGY_CSS`],
/// [`DEMO_JS`]). The fifth, `site-header.js`, is the portal's, and a host that
/// omits it gets a header that no longer collapses on scroll.
///
/// `ontology.css` is needed because sections 20-23 render `OntoBadge` and
/// `OntoAnnotation`, whose classes live there rather than in `components.css`.
pub const DEMO_SHELL: &str = include_str!("../assets/demo-shell.html");

/// Page glue for [`DemoPage`]: opens and closes the `NavDropdown`/`Modal`
/// components, and wires the [`crate::CodeBlock`] copy buttons to the
/// clipboard.
///
/// The components themselves are presentational — no `Callback`, no event
/// handlers — so every interaction on the page is this file operating on the
/// rendered markup. `AccordionItem` appears in neither: it is a native
/// `<details>` and handles itself.
pub const DEMO_JS: &str = include_str!("../assets/demo-page.js");

#[derive(Properties, PartialEq, Default)]
pub struct DemoPageProps {
    /// Site-header links. Empty by default — a standalone demo has no sibling
    /// pages to link to.
    #[prop_or_default]
    pub nav: Vec<SiteNavLink>,
    /// Footer byline. Defaults to the toolkit's own.
    #[prop_or_default]
    pub footer_text: Option<AttrValue>,
    /// Footer back-link. Defaults to an in-page jump, for the same reason `nav`
    /// defaults to empty.
    #[prop_or_default]
    pub footer_link: Option<NavLink>,
}

#[function_component(DemoPage)]
pub fn demo_page(props: &DemoPageProps) -> Html {
    let footer_text = props.footer_text.clone().unwrap_or(AttrValue::Static(
        "eona-ui-toolkit — component gallery and integration guide",
    ));
    let footer_link = props
        .footer_link
        .clone()
        .unwrap_or_else(|| NavLink::new("#buttons", "Back to the top of the gallery"));

    html! {
        <>
            <SiteHeader links={props.nav.clone()} />
            <HeroFilter
                title="The EONA-X component gallery"
                description="Buttons, tags, form fields and cards pulled from the live EONA-X website's own WordPress theme, ported here as eona-ui-toolkit components — each shown next to the html! call that renders it, followed by what it takes to consume the crate."
                with_backdrop=true
            >
                <PillButton label="View eona-x.eu" kind={PillButtonKind::GlassLight} href="https://eona-x.eu/" />
            </HeroFilter>
            <ComponentsGallery />
            <IntegrationSection />
            <OntologySection />
            <SimpleFooter text={footer_text} link={footer_link} />
        </>
    }
}
