use yew::prelude::*;

use super::frame;
use crate::atoms::EvolutionBadge;
use crate::chart::{format_number, format_with_unit, metric_of};
use crate::molecules::MetricTile;

/// A [`MetricTile`] fed from a dataset: the latest value, and the change from the
/// one before as an [`EvolutionBadge`] when there is one.
#[derive(Properties, PartialEq)]
pub struct CubeMetricTileProps {
    /// The JSON-LD document.
    pub jsonld: AttrValue,
    /// A fetch failure from the host, shown instead of the tile.
    #[prop_or_default]
    pub fetch_error: Option<AttrValue>,
    /// Language tag for the label, e.g. `"fr"`.
    #[prop_or_default]
    pub lang: Option<AttrValue>,
    /// Small print under the value.
    #[prop_or_default]
    pub help: Option<AttrValue>,
}

#[function_component(CubeMetricTile)]
pub fn cube_metric_tile(props: &CubeMetricTileProps) -> Html {
    let loaded = use_memo((props.jsonld.clone(), props.fetch_error.clone()), |(j, e)| {
        frame::load(j, e.as_ref())
    });
    frame::guarded(&loaded, props.lang.as_deref(), |cube| {
        let m = metric_of(cube)?;
        let label = cube.title.get(props.lang.as_deref()).unwrap_or_default().to_owned();
        let value = format_with_unit(m.value, cube.unit.as_deref());
        let badge = m.delta.map(|d| {
            let sign = if d > 0.0 { "+" } else { "" };
            html! {
                <EvolutionBadge value={format!("{sign}{}", format_number(d))} trend={m.trend} />
            }
        });
        Ok(html! {
            <MetricTile label={label} value={value} help={props.help.clone()}>
                { for badge }
            </MetricTile>
        })
    })
}
