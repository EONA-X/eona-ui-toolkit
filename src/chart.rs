//! Chart geometry, as pure functions.
//!
//! The chart components are deliberately arithmetic-free: everything a chart
//! derives from its data is computed here and handed over already-derived. That
//! is not a style preference. A component whose markup is a *transform* of its
//! props cannot be spliced into the React distribution — the verify gate
//! quarantines exactly that shape, which is why `Swatch`, `PaletteGroup`,
//! `DatasetCard` and `OntoAnnotation` are in `abi/quarantine.json`. Charts are
//! the most transform-shaped components there are, so the split is drawn up
//! front rather than retrofitted.
//!
//! The thresholds and the percentage format match the Data Quality portal's
//! `app/dashboard/charts.py`, which these components replace.

use yew::AttrValue;

/// Radius of the donut ring in the component's own `viewBox` units.
const DONUT_RADIUS: f64 = 45.0;

/// The ring's full circumference, `2πr`. The value arc is drawn by dashing the
/// ring rather than by generating an arc path: one number instead of a path
/// expression, and nothing for the component to compute.
pub const DONUT_CIRCUMFERENCE: f64 = 2.0 * std::f64::consts::PI * DONUT_RADIUS;

/// Which band a score falls in. Thresholds are the portal's own
/// (`charts.py:25-30`): under 50% is danger, under 80% is warning, else good.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreBand {
    Good,
    Warning,
    Danger,
}

impl ScoreBand {
    /// The band a fraction in `0.0..=1.0` falls in. Out-of-range input is
    /// clamped rather than rejected: a score is a measurement, and a caller
    /// that computed 1.0000001 should get a full ring, not a panic.
    pub fn of(fraction: f64) -> Self {
        let f = clamp_fraction(fraction);
        if f < 0.5 {
            ScoreBand::Danger
        } else if f < 0.8 {
            ScoreBand::Warning
        } else {
            ScoreBand::Good
        }
    }

    pub(crate) fn class(self) -> &'static str {
        match self {
            ScoreBand::Good => "chart-donut__value chart-donut__value--good",
            ScoreBand::Warning => "chart-donut__value chart-donut__value--warning",
            ScoreBand::Danger => "chart-donut__value chart-donut__value--danger",
        }
    }
}

/// Everything [`crate::ScoreDonut`] needs, with no arithmetic left to do.
#[derive(Clone, Debug, PartialEq)]
pub struct DonutArc {
    /// `stroke-dasharray` for the value ring: the drawn length, then the gap.
    pub dash_array: AttrValue,
    pub band: ScoreBand,
    /// The percentage as the donut displays it — `charts.py:50` formats the
    /// same way, `f"{percentage:.1%}"`.
    pub label: AttrValue,
}

/// NaN is treated as zero, and anything outside `0.0..=1.0` is clamped to it.
fn clamp_fraction(fraction: f64) -> f64 {
    if fraction.is_nan() {
        0.0
    } else {
        fraction.clamp(0.0, 1.0)
    }
}

/// Turns a score in `0.0..=1.0` into the ring geometry, the band and the label.
pub fn donut_arc(fraction: f64) -> DonutArc {
    let f = clamp_fraction(fraction);
    let drawn = DONUT_CIRCUMFERENCE * f;
    DonutArc {
        dash_array: AttrValue::from(format!(
            "{:.3} {:.3}",
            drawn,
            DONUT_CIRCUMFERENCE - drawn
        )),
        band: ScoreBand::of(f),
        label: AttrValue::from(format!("{:.1}%", f * 100.0)),
    }
}

/// Which way an evolution went. Distinct from [`ScoreBand`] on purpose: a
/// falling score can still be in the good band, and the two must not be
/// conflated in either the markup or the palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trend {
    Up,
    Down,
    Flat,
}

impl Trend {
    pub(crate) fn class(self) -> &'static str {
        match self {
            Trend::Up => "evolution-badge evolution-badge--up",
            Trend::Down => "evolution-badge evolution-badge--down",
            Trend::Flat => "evolution-badge evolution-badge--flat",
        }
    }

    /// The glyph carried beside the value, so direction never rests on colour
    /// alone — which also survives a greyscale print.
    pub(crate) fn glyph(self) -> &'static str {
        match self {
            Trend::Up => "\u{25B2}",
            Trend::Down => "\u{25BC}",
            Trend::Flat => "\u{2013}",
        }
    }

    /// Spoken by screen readers in place of the glyph.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Trend::Up => "up",
            Trend::Down => "down",
            Trend::Flat => "unchanged",
        }
    }
}

/// Deltas smaller than this read as no movement. `metrics.py:104` compares
/// against zero; a float epsilon avoids rendering an arrow for `1e-15`.
const FLAT_EPSILON: f64 = 1e-9;

/// The direction of a delta. NaN is [`Trend::Flat`] — an unknown movement is
/// better shown as none than as a confident arrow.
pub fn trend_of(delta: f64) -> Trend {
    if delta.is_nan() || delta.abs() <= FLAT_EPSILON {
        Trend::Flat
    } else if delta > 0.0 {
        Trend::Up
    } else {
        Trend::Down
    }
}
