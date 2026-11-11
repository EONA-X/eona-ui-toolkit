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

use crate::cube::{Axis, AxisKey, Cube, CubeError, Obs};
use std::collections::BTreeMap;
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

// ---------------------------------------------------------------------------
// Data-driven charts: layout from a [`Cube`]. Pure, no Yew, no serde.
// ---------------------------------------------------------------------------

/// A drawing surface in `viewBox` units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

/// The `viewBox` every chart is laid out in: 640 by 320, scaled by CSS.
pub const VIEW: Size = Size { w: 640.0, h: 320.0 };

const MARGIN_LEFT: f64 = 56.0;
const MARGIN_RIGHT: f64 = 16.0;
const MARGIN_TOP: f64 = 16.0;
const MARGIN_BOTTOM: f64 = 40.0;
/// How many y ticks [`nice_ticks`] is asked for.
const TICK_TARGET: usize = 5;
/// Share of a band left empty, split over both sides of the bar group.
const BAND_PADDING: f64 = 0.2;

/// The plot area inside the margins: `(x0, y0, width, height)`.
pub(crate) fn plot(size: Size) -> (f64, f64, f64, f64) {
    (
        MARGIN_LEFT,
        MARGIN_TOP,
        size.w - MARGIN_LEFT - MARGIN_RIGHT,
        size.h - MARGIN_TOP - MARGIN_BOTTOM,
    )
}

/// A label at a position: an x centre for x labels, a y for y ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub pos: f64,
    pub label: String,
}

/// One bar, or one segment of a stacked bar.
#[derive(Clone, Debug, PartialEq)]
pub struct BarRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub series: Option<usize>,
    /// The axis key the bar belongs to.
    pub label: String,
    /// The value with the dataset's unit.
    pub value_label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BarGeometry {
    pub bars: Vec<BarRect>,
    pub x_labels: Vec<Tick>,
    pub y_ticks: Vec<Tick>,
    /// The y of the zero baseline.
    pub zero_y: f64,
}

/// One point of a line.
#[derive(Clone, Debug, PartialEq)]
pub struct LinePoint {
    pub x: f64,
    pub y: f64,
    pub label: String,
    pub value_label: String,
}

/// One series as a path. A missing point ends a run and the next present point
/// starts a new `M`, so the line has a gap there.
#[derive(Clone, Debug, PartialEq)]
pub struct LinePath {
    pub series: Option<usize>,
    pub d: String,
    pub points: Vec<LinePoint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineGeometry {
    pub paths: Vec<LinePath>,
    pub x_labels: Vec<Tick>,
    pub y_ticks: Vec<Tick>,
    pub zero_y: f64,
}

/// Round numbers covering `min..=max`, about `target` of them: a step of 1, 2 or
/// 5 times a power of ten, with the first tick at or below `min` and the last at
/// or above `max`. An empty range (`max <= min`) is widened by one unit.
pub fn nice_ticks(min: f64, max: f64, target: usize) -> Vec<f64> {
    let max = if max > min { max } else { min + 1.0 };
    let raw = (max - min) / target.max(1) as f64;
    let exp = raw.log10().floor();
    let mag = 10f64.powf(exp);
    let residual = raw / mag;
    let step = mag
        * if residual <= 1.5 {
            1.0
        } else if residual <= 3.0 {
            2.0
        } else if residual <= 7.0 {
            5.0
        } else {
            10.0
        };
    // Multiples of the step carry float noise (3 * 0.1); round it off.
    let scale = 10f64.powf((2.0 - exp).max(0.0));
    let clean = |v: f64| (v * scale).round() / scale;
    let first = (min / step + 1e-9).floor() as i64;
    let last = (max / step - 1e-9).ceil() as i64;
    (first..=last).map(|i| clean(i as f64 * step)).collect()
}

/// A number as the charts print it: up to two decimals without trailing zeros,
/// a thin space (U+2009) between groups of three digits.
pub fn format_number(v: f64) -> String {
    let s = format!("{:.2}", v.abs());
    let s = s.trim_end_matches('0').trim_end_matches('.');
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push('\u{2009}');
        }
        out.push(c);
    }
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    if v < 0.0 && out != "0" {
        out.insert(0, '-');
    }
    out
}

/// [`format_number`] followed by the unit: `%` is attached, any other unit
/// follows a space.
pub fn format_with_unit(v: f64, unit: Option<&str>) -> String {
    match unit {
        None | Some("") => format_number(v),
        Some("%") => format!("{}%", format_number(v)),
        Some(u) => format!("{} {u}", format_number(v)),
    }
}

/// A coordinate as written into a path: two decimals, trailing zeros dropped.
pub(crate) fn coord(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Distinct axis keys in the order they are drawn: dates chronologically,
/// categories as the cube lists them.
fn axis_keys(cube: &Cube) -> Vec<&AxisKey> {
    let mut seen = std::collections::BTreeSet::new();
    let mut keys: Vec<&AxisKey> = Vec::new();
    for o in &cube.observations {
        if seen.insert(&o.key) {
            keys.push(&o.key);
        }
    }
    if matches!(cube.axis, Axis::Date(_)) {
        keys.sort();
    }
    keys
}

fn key_index<'a>(keys: &[&'a AxisKey]) -> BTreeMap<&'a AxisKey, usize> {
    keys.iter().enumerate().map(|(i, k)| (*k, i)).collect()
}

/// A value scale for `lo..=hi`, always including zero.
struct YScale {
    ticks: Vec<f64>,
    y0: f64,
    height: f64,
}

impl YScale {
    fn new(lo: f64, hi: f64, size: Size) -> Self {
        let (_, y0, _, height) = plot(size);
        Self {
            ticks: nice_ticks(lo.min(0.0), hi.max(0.0), TICK_TARGET),
            y0,
            height,
        }
    }

    fn y(&self, v: f64) -> f64 {
        let (lo, hi) = (self.ticks[0], self.ticks[self.ticks.len() - 1]);
        self.y0 + self.height - (v - lo) / (hi - lo) * self.height
    }

    fn ticks(&self) -> Vec<Tick> {
        self.ticks
            .iter()
            .map(|t| Tick {
                pos: self.y(*t),
                label: format_number(*t),
            })
            .collect()
    }
}

fn value_range(cube: &Cube) -> (f64, f64) {
    cube.observations
        .iter()
        .fold((0.0f64, 0.0f64), |(lo, hi), o| (lo.min(o.value), hi.max(o.value)))
}

fn x_labels(keys: &[&AxisKey], size: Size) -> Vec<Tick> {
    let (x0, _, w, _) = plot(size);
    let band = w / keys.len().max(1) as f64;
    keys.iter()
        .enumerate()
        .map(|(i, k)| Tick {
            pos: x0 + (i as f64 + 0.5) * band,
            label: k.as_str().to_owned(),
        })
        .collect()
}

/// Bars per axis key, grouped side by side when the cube has series. Negative
/// values hang from the zero baseline.
pub fn bar_layout(cube: &Cube, size: Size) -> BarGeometry {
    let keys = axis_keys(cube);
    let index = key_index(&keys);
    let (x0, _, w, _) = plot(size);
    let (lo, hi) = value_range(cube);
    let scale = YScale::new(lo, hi, size);
    let zero_y = scale.y(0.0);
    let band = w / keys.len().max(1) as f64;
    let slots = cube.series.len().max(1);
    let bar_w = band * (1.0 - BAND_PADDING) / slots as f64;
    let bars = cube
        .observations
        .iter()
        .map(|o| {
            let i = index[&o.key];
            let top = scale.y(o.value);
            BarRect {
                x: x0 + i as f64 * band + band * BAND_PADDING / 2.0 + o.series.unwrap_or(0) as f64 * bar_w,
                y: top.min(zero_y),
                w: bar_w,
                h: (top - zero_y).abs(),
                series: o.series,
                label: o.key.as_str().to_owned(),
                value_label: format_with_unit(o.value, cube.unit.as_deref()),
            }
        })
        .collect();
    BarGeometry {
        bars,
        x_labels: x_labels(&keys, size),
        y_ticks: scale.ticks(),
        zero_y,
    }
}

/// One stack per axis key, series bottom to top. Stacking negative values has no
/// honest reading, so any is an error.
pub fn stacked_layout(cube: &Cube, size: Size) -> Result<BarGeometry, CubeError> {
    if let Some(o) = cube.observations.iter().find(|o| o.value < 0.0) {
        return Err(CubeError::new(
            o.key.as_str(),
            "stacked bars need non-negative values",
        ));
    }
    let keys = axis_keys(cube);
    let index = key_index(&keys);
    let (x0, _, w, _) = plot(size);
    let mut sums = vec![0.0f64; keys.len()];
    for o in &cube.observations {
        let i = index[&o.key];
        sums[i] += o.value;
    }
    let hi = sums.iter().cloned().fold(0.0, f64::max);
    let scale = YScale::new(0.0, hi, size);
    let band = w / keys.len().max(1) as f64;
    let bar_w = band * (1.0 - BAND_PADDING);
    let mut stacked = vec![0.0f64; keys.len()];
    let mut order: Vec<usize> = (0..cube.observations.len()).collect();
    order.sort_by_key(|&j| cube.observations[j].series);
    let mut rects: Vec<Option<BarRect>> = vec![None; cube.observations.len()];
    for j in order {
        let o = &cube.observations[j];
        let i = index[&o.key];
        let (from, to) = (scale.y(stacked[i]), scale.y(stacked[i] + o.value));
        stacked[i] += o.value;
        rects[j] = Some(BarRect {
            x: x0 + i as f64 * band + band * BAND_PADDING / 2.0,
            y: to,
            w: bar_w,
            h: from - to,
            series: o.series,
            label: o.key.as_str().to_owned(),
            value_label: format_with_unit(o.value, cube.unit.as_deref()),
        });
    }
    Ok(BarGeometry {
        // In the cube's own order, so rects line up with its observations.
        bars: rects.into_iter().flatten().collect(),
        x_labels: x_labels(&keys, size),
        y_ticks: scale.ticks(),
        zero_y: scale.y(0.0),
    })
}

/// One path per series over the axis keys, evenly spaced. A series without a
/// value at a key has a gap there.
pub fn line_layout(cube: &Cube, size: Size) -> LineGeometry {
    let keys = axis_keys(cube);
    let (x0, _, w, _) = plot(size);
    let (lo, hi) = value_range(cube);
    let scale = YScale::new(lo, hi, size);
    let by_cell: BTreeMap<(&AxisKey, Option<usize>), &Obs> = cube
        .observations
        .iter()
        .map(|o| ((&o.key, o.series), o))
        .collect();
    let band = w / keys.len().max(1) as f64;
    let n_series = cube.series.len().max(1);
    let paths = (0..n_series)
        .map(|s| {
            let series = if cube.series.is_empty() { None } else { Some(s) };
            let mut d = String::new();
            let mut points = Vec::new();
            let mut open = false;
            for (i, k) in keys.iter().enumerate() {
                let Some(o) = by_cell.get(&(*k, series)) else {
                    open = false;
                    continue;
                };
                let (x, y) = (x0 + (i as f64 + 0.5) * band, scale.y(o.value));
                if !d.is_empty() {
                    d.push(' ');
                }
                d.push_str(&format!("{}{} {}", if open { "L" } else { "M" }, coord(x), coord(y)));
                open = true;
                points.push(LinePoint {
                    x,
                    y,
                    label: k.as_str().to_owned(),
                    value_label: format_with_unit(o.value, cube.unit.as_deref()),
                });
            }
            LinePath { series, d, points }
        })
        .collect();
    LineGeometry {
        paths,
        x_labels: x_labels(&keys, size),
        y_ticks: scale.ticks(),
        zero_y: scale.y(0.0),
    }
}

/// The observations in drawing order: dates chronologically, categories as given.
fn drawn_order(cube: &Cube) -> Vec<&Obs> {
    let mut v: Vec<&Obs> = cube.observations.iter().collect();
    if matches!(cube.axis, Axis::Date(_)) {
        v.sort_by(|a, b| a.key.cmp(&b.key));
    }
    v
}

fn single_series(cube: &Cube, what: &str) -> Result<(), CubeError> {
    if cube.series.len() > 1 {
        Err(CubeError::new(
            &cube.id,
            format!("{what} shows one series, the dataset has {}", cube.series.len()),
        ))
    } else {
        Ok(())
    }
}

/// The ring for one observation, its value read as a fraction in `0..=1` (see
/// [`donut_arc`]). `select` names the axis key; without it the latest date is
/// taken, or the only observation. Several categories without a key is an error.
pub fn donut_of(cube: &Cube, select: Option<&str>) -> Result<DonutArc, CubeError> {
    single_series(cube, "a donut")?;
    let obs = drawn_order(cube);
    let picked = match select {
        Some(key) => obs
            .iter()
            .find(|o| o.key.as_str() == key)
            .ok_or_else(|| CubeError::new(&cube.id, format!("no observation for {key:?}")))?,
        None => match (cube.axis, obs.as_slice()) {
            (_, []) => return Err(CubeError::new(&cube.id, "no observations")),
            (Axis::Date(_), all) => all.last().expect("non-empty"),
            (Axis::Category, [only]) => only,
            (Axis::Category, _) => {
                return Err(CubeError::new(
                    &cube.id,
                    "several categories: name the one to show",
                ))
            }
        },
    };
    Ok(donut_arc(picked.value))
}

/// What a metric tile shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metric {
    pub value: f64,
    /// Latest minus previous; `None` with a single observation.
    pub delta: Option<f64>,
    pub trend: Trend,
}

/// The latest value, and its change from the one before (see [`trend_of`]).
pub fn metric_of(cube: &Cube) -> Result<Metric, CubeError> {
    single_series(cube, "a metric tile")?;
    let obs = drawn_order(cube);
    let Some(last) = obs.last() else {
        return Err(CubeError::new(&cube.id, "no observations"));
    };
    let delta = obs.len().checked_sub(2).map(|i| last.value - obs[i].value);
    Ok(Metric {
        value: last.value,
        delta,
        trend: delta.map_or(Trend::Flat, trend_of),
    })
}
