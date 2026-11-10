use yew::prelude::*;

use super::frame::{self, series_class};
use crate::chart::{coord, line_layout, VIEW};

/// `.chart--line` — one line per series over the axis keys, with a gap where a
/// series has no value.
#[derive(Properties, PartialEq)]
pub struct LineChartProps {
    /// The JSON-LD document.
    pub jsonld: AttrValue,
    /// A fetch failure from the host, shown instead of the chart.
    #[prop_or_default]
    pub fetch_error: Option<AttrValue>,
    /// Language tag for the label and comment, e.g. `"fr"`.
    #[prop_or_default]
    pub lang: Option<AttrValue>,
}

#[function_component(LineChart)]
pub fn line_chart(props: &LineChartProps) -> Html {
    let id = frame::use_chart_id();
    let loaded = use_memo((props.jsonld.clone(), props.fetch_error.clone()), |(j, e)| {
        frame::load(j, e.as_ref())
    });
    let lang = props.lang.as_deref();
    frame::guarded(&loaded, lang, |cube| {
        let g = line_layout(cube, VIEW);
        let svg = html! {
            <>
                { frame::axes(&g.y_ticks, &g.x_labels, g.zero_y) }
                <g class="chart__lines">
                    { for g.paths.iter().map(|p| html! {
                        <>
                            <path class={classes!("chart__line", series_class(p.series))}
                                  d={p.d.clone()} />
                            { for p.points.iter().map(|pt| html! {
                                <circle class={classes!("chart__point", series_class(p.series))}
                                        cx={coord(pt.x)} cy={coord(pt.y)} r="3">
                                    <title>{ format!("{}: {}", pt.label, pt.value_label) }</title>
                                </circle>
                            }) }
                        </>
                    }) }
                </g>
            </>
        };
        Ok(frame::figure("line", &id, cube, lang, "Line chart", svg))
    })
}
