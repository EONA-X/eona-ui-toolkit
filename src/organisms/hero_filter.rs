use yew::prelude::*;

/// `.hero__filter` on eona-x.eu — the listing-page hero pattern used on
/// `/fr/actualites/` and `/fr/cas-etude/`: a dark, two-column band (title +
/// description on the left, filter/action content on the right, stacking to
/// one column on narrow viewports), `padding-top: calc(var(--header-height)
/// * 2)` on the real site to clear its floating header. Visually simpler and
/// more content-focused than the marketing `Hero` (`HeroVariant::Full`),
/// which stays on `index.html`; this fits the two reference/listing pages
/// (`icons.html`, `components.html`) better.
///
/// `with_backdrop` echoes `/fr/cas-etude/`'s `.hero__filter-image` +
/// `.hero__filter-overlay` (a real background photo faded to the panel
/// color at the bottom) with a decorative gradient instead of a real photo —
/// same placeholder-over-hotlink choice this crate already makes for
/// `NewsCard`/`CaseCard`'s images.
///
/// The real site's actions column holds `<select>` filter dropdowns
/// (`/fr/actualites/` has three, `/fr/cas-etude/` has one) — that styling
/// isn't ported here since neither `icons.html` nor `components.html` has
/// anything to filter yet; `children` just holds each page's existing
/// buttons/links instead, which is what the column is for structurally.
#[derive(Properties, PartialEq)]
pub struct HeroFilterProps {
    pub title: AttrValue,
    pub description: AttrValue,
    /// Right-column content — buttons, links, a count label, etc.
    pub children: Children,
    #[prop_or_default]
    pub with_backdrop: bool,
}

#[function_component(HeroFilter)]
pub fn hero_filter(props: &HeroFilterProps) -> Html {
    let class = if props.with_backdrop { "hero-filter hero-filter--backdrop" } else { "hero-filter" };
    html! {
        <section class={class}>
            if props.with_backdrop {
                <div class="hero-filter__image"><div class="hero-filter__overlay"></div></div>
            }
            <div class="wrap hero-filter__container">
                <div class="hero-filter__content">
                    <h1 class="hero-filter__title">{ &props.title }</h1>
                    <p class="hero-filter__description">{ &props.description }</p>
                </div>
                <div class="hero-filter__actions">
                    { for props.children.iter() }
                </div>
            </div>
        </section>
    }
}
