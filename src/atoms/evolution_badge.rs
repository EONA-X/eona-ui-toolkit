use yew::prelude::*;

use crate::chart::Trend;

/// `.evolution-badge` — a signed delta beside a metric, replacing the Data
/// Quality portal's `metrics.py:104 _evolution_badge`.
///
/// Takes the value already formatted and the direction already decided
/// ([`crate::chart::trend_of`]): the component picks a class and a glyph and
/// renders, and does no arithmetic of its own.
///
/// Direction is carried by the glyph as well as the colour, so it survives a
/// greyscale print and a colour-blind reader.
#[derive(Properties, PartialEq)]
pub struct EvolutionBadgeProps {
    /// The delta as it should read, e.g. `"+3.2 pts"`. Formatting belongs to
    /// the caller, which knows the unit.
    pub value: AttrValue,
    pub trend: Trend,
}

#[function_component(EvolutionBadge)]
pub fn evolution_badge(props: &EvolutionBadgeProps) -> Html {
    html! {
        <span class={props.trend.class()}>
            <span class="evolution-badge__glyph" aria-hidden="true">
                { props.trend.glyph() }
            </span>
            <span class="sr-only">{ props.trend.label() }</span>
            <span class="evolution-badge__value">{ &props.value }</span>
        </span>
    }
}
