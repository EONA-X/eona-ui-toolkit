//! The arithmetic the chart components do not do.
//!
//! Boundaries matter more than midpoints here: the bands come from the Data
//! Quality portal's thresholds (`charts.py:25-30`), and an off-by-one at 50% or
//! 80% changes a dataset's verdict.

use eona_ui_toolkit::chart::{donut_arc, trend_of, ScoreBand, Trend, DONUT_CIRCUMFERENCE};

fn drawn(fraction: f64) -> f64 {
    donut_arc(fraction)
        .dash_array
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn bands_switch_on_the_portals_thresholds_not_beside_them() {
    assert_eq!(ScoreBand::of(0.0), ScoreBand::Danger);
    assert_eq!(ScoreBand::of(0.499), ScoreBand::Danger);
    assert_eq!(ScoreBand::of(0.5), ScoreBand::Warning, "50% is warning, not danger");
    assert_eq!(ScoreBand::of(0.799), ScoreBand::Warning);
    assert_eq!(ScoreBand::of(0.8), ScoreBand::Good, "80% is good, not warning");
    assert_eq!(ScoreBand::of(1.0), ScoreBand::Good);
}

#[test]
fn the_ring_is_empty_at_zero_and_closed_at_one() {
    assert!(drawn(0.0).abs() < 1e-6);
    assert!((drawn(1.0) - DONUT_CIRCUMFERENCE).abs() < 1e-3);
    // The two lengths always add back to the whole ring, or the gap shows.
    let arc = donut_arc(0.371);
    let parts: Vec<f64> = arc.dash_array.split(' ').map(|p| p.parse().unwrap()).collect();
    assert!((parts[0] + parts[1] - DONUT_CIRCUMFERENCE).abs() < 1e-2);
}

#[test]
fn out_of_range_input_clamps_rather_than_panicking() {
    assert_eq!(ScoreBand::of(-3.0), ScoreBand::Danger);
    assert_eq!(ScoreBand::of(4.2), ScoreBand::Good);
    assert!(drawn(-3.0).abs() < 1e-6);
    assert!((drawn(4.2) - DONUT_CIRCUMFERENCE).abs() < 1e-3);
    // A NaN score is a measurement that failed, not a full ring.
    assert_eq!(ScoreBand::of(f64::NAN), ScoreBand::Danger);
    assert!(drawn(f64::NAN).abs() < 1e-6);
}

#[test]
fn the_label_matches_the_portals_own_format() {
    // `charts.py:50` renders `f"{percentage:.1%}"`.
    assert_eq!(donut_arc(0.8423).label, "84.2%");
    assert_eq!(donut_arc(1.0).label, "100.0%");
    assert_eq!(donut_arc(0.0).label, "0.0%");
}

#[test]
fn a_negligible_delta_is_flat_rather_than_a_confident_arrow() {
    assert_eq!(trend_of(0.0), Trend::Flat);
    assert_eq!(trend_of(1e-15), Trend::Flat);
    assert_eq!(trend_of(-1e-15), Trend::Flat);
    assert_eq!(trend_of(f64::NAN), Trend::Flat);
    assert_eq!(trend_of(0.1), Trend::Up);
    assert_eq!(trend_of(-0.1), Trend::Down);
}
