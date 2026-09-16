use yew::prelude::*;

/// `.card-member__filter` on eona-x.eu (the members directory page, not the
/// homepage marquee — see `MemberCard` for that one) — a richer card with a
/// logo, name/type header, country, and sector tags. The directory's actual
/// filter *form* (country/sector dropdowns, AJAX-refreshed results) isn't
/// ported — there's no backend here to filter against — only the card shape
/// used inside it, shown as a static list in `ComponentsGallery`.
#[derive(Properties, PartialEq)]
pub struct MemberFilterCardProps {
    pub name: AttrValue,
    pub member_type: AttrValue,
    pub country: AttrValue,
    pub sectors: Vec<&'static str>,
}

#[function_component(MemberFilterCard)]
pub fn member_filter_card(props: &MemberFilterCardProps) -> Html {
    html! {
        <article class="card-member__filter">
            <div class="card-member__filter-header">
                <div class="card-member__filter-logo">{ "logo" }</div>
                <div class="card-member__filter-title">
                    <h3 class="card-member__filter-title-name">{ &props.name }</h3>
                    <div class="card-member__filter-title-type"><p>{ &props.member_type }</p></div>
                </div>
            </div>
            <div class="card-member__filter-localisation">
                <div class="term">
                    <div class="term-name">
                        <div class="term-name-label">{ &props.country }</div>
                    </div>
                </div>
            </div>
            <div class="card-member__filter-sectors">
                { for props.sectors.iter().map(|s| html! {
                    <div class="term-container">
                        <div class="term"><div class="term-name"><div class="term-name-label">{ *s }</div></div></div>
                    </div>
                }) }
            </div>
        </article>
    }
}
