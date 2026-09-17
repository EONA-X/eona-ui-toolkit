use yew::prelude::*;

/// `.metric-tile` — a headline number with its label, replacing the Data
/// Quality portal's `metrics.py:93 _build_metric` (`st.metric`).
///
/// A number is not a chart: the form heuristic says a single value with no
/// comparison is a stat tile, so this renders text rather than a plot.
///
/// `children` is the slot for an [`crate::EvolutionBadge`]; the tile does not
/// take the delta itself, because deciding what a delta means belongs to the
/// caller and pushing it in here would make the tile compute.
#[derive(Properties, PartialEq)]
pub struct MetricTileProps {
    /// What the number is, e.g. `"Records analysed"`.
    pub label: AttrValue,
    /// The number as it should read, thousands separators and unit included.
    pub value: AttrValue,
    /// Optional clarification, rendered as small print under the value rather
    /// than hidden behind a tooltip — the target is a printed page.
    #[prop_or_default]
    pub help: Option<AttrValue>,
    /// An `EvolutionBadge`, or nothing.
    #[prop_or_default]
    pub children: Children,
}

#[function_component(MetricTile)]
pub fn metric_tile(props: &MetricTileProps) -> Html {
    html! {
        <div class="metric-tile">
            <span class="metric-tile__label">{ &props.label }</span>
            <span class="metric-tile__value">{ &props.value }</span>
            { for props.help.as_ref().map(|h| html! {
                <span class="metric-tile__help">{ h }</span>
            }) }
            if !props.children.is_empty() {
                <span class="metric-tile__delta">{ for props.children.iter() }</span>
            }
        </div>
    }
}
