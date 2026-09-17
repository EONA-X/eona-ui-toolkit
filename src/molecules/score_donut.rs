use yew::prelude::*;

use crate::chart::{DONUT_CIRCUMFERENCE, ScoreBand};

/// `.chart-donut` — a quality score as a ring with the percentage in the
/// middle, replacing the Data Quality portal's `charts.py:24 score_donut_chart`.
///
/// Pure SVG and no JavaScript, which is what makes it printable: the portal's
/// version is a plotly figure and needs a browser to become a picture.
///
/// The ring is drawn by dashing a circle rather than by generating an arc path,
/// so the only value that varies is `stroke-dasharray` — computed by
/// [`crate::chart::donut_arc`] and passed through untouched. The component
/// contains no arithmetic, which is what keeps it out of the React
/// distribution's quarantine list.
#[derive(Properties, PartialEq)]
pub struct ScoreDonutProps {
    /// `stroke-dasharray` for the value ring, from [`crate::chart::donut_arc`].
    pub dash_array: AttrValue,
    pub band: ScoreBand,
    /// The percentage as it should read, e.g. `"84.2%"`.
    pub label: AttrValue,
    /// What the score is of. Rendered under the ring and used as the figure's
    /// accessible name, so the donut is never a bare number.
    pub caption: AttrValue,
}

#[function_component(ScoreDonut)]
pub fn score_donut(props: &ScoreDonutProps) -> Html {
    // `r` and the dash geometry must agree; both derive from DONUT_RADIUS in
    // `chart.rs`, which is why the circumference is a public constant.
    let full = DONUT_CIRCUMFERENCE.to_string();
    html! {
        <figure class="chart-donut">
            <svg class="chart-donut__svg" viewBox="0 0 120 120" role="img"
                 aria-label={format!("{}: {}", props.caption, props.label)}>
                <circle class="chart-donut__track" cx="60" cy="60" r="45"
                        stroke-dasharray={full} />
                // Rotated so the ring starts at twelve o'clock rather than at
                // three, which is where SVG angles begin.
                <circle class={props.band.class()} cx="60" cy="60" r="45"
                        stroke-dasharray={props.dash_array.clone()}
                        transform="rotate(-90 60 60)" />
                <text class="chart-donut__label" x="60" y="60"
                      text-anchor="middle" dominant-baseline="central">
                    { &props.label }
                </text>
            </svg>
            <figcaption class="chart-donut__caption">{ &props.caption }</figcaption>
        </figure>
    }
}
