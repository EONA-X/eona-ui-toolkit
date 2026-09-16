use yew::prelude::*;

/// A catalog entry: thumbnail, title, version, publisher and a short
/// description, ending in a call to action.
///
/// Replaces `edc_web_components::DatasetCard`, which is a PatternFly component
/// and brought PatternFly's whole design system with it. The shape is the same
/// so a DCAT catalog reads the same way; the styling is the charter's.
///
/// The action is an `href`, not a `Callback`: every use of the PatternFly
/// original resolved to `window.location = href`, so a link does the same job
/// while keeping this component presentational — and gives middle-click,
/// open-in-new-tab and a status-bar preview for free, which the button never
/// had.
#[derive(Properties, PartialEq)]
pub struct DatasetCardProps {
    pub title: AttrValue,
    /// Where the whole card points.
    pub href: AttrValue,
    #[prop_or_default]
    pub description: Option<AttrValue>,
    /// Free text, shown as a small chip beside the title. Not parsed — a
    /// catalog carries whatever its publisher wrote, semver or not.
    #[prop_or_default]
    pub version: Option<AttrValue>,
    #[prop_or_default]
    pub publisher: Option<AttrValue>,
    /// Image URL. Absent renders a lettered placeholder rather than a gap.
    #[prop_or_default]
    pub thumbnail: Option<AttrValue>,
    /// Category chip over the thumbnail, e.g. `"OWL"`.
    #[prop_or_default]
    pub badge: Option<AttrValue>,
    /// Extra class on the badge, for per-category colouring.
    #[prop_or_default]
    pub badge_class: Option<AttrValue>,
    #[prop_or(AttrValue::Static("View documentation"))]
    pub action_label: AttrValue,
}

#[function_component(DatasetCard)]
pub fn dataset_card(props: &DatasetCardProps) -> Html {
    let initial = props.title.chars().next().unwrap_or('?').to_uppercase().to_string();

    html! {
        <a class="dataset-card" href={props.href.clone()}>
            <div class="dataset-card__media">
                if let Some(src) = &props.thumbnail {
                    <img class="dataset-card__thumb" src={src.clone()} alt="" loading="lazy" />
                } else {
                    <span class="dataset-card__thumb dataset-card__thumb--empty" aria-hidden="true">
                        { initial }
                    </span>
                }
                if let Some(badge) = &props.badge {
                    <span class={classes!("dataset-card__badge", props.badge_class.clone().map(|c| c.to_string()))}>
                        { badge }
                    </span>
                }
            </div>
            <div class="dataset-card__body">
                <h3 class="dataset-card__title">
                    { &props.title }
                    if let Some(version) = &props.version {
                        <span class="dataset-card__version">{ version }</span>
                    }
                </h3>
                if let Some(publisher) = &props.publisher {
                    <p class="dataset-card__publisher">{ publisher }</p>
                }
                if let Some(description) = &props.description {
                    <p class="dataset-card__description">{ description }</p>
                }
                <span class="dataset-card__action">{ &props.action_label }</span>
            </div>
        </a>
    }
}
