//! Layout geometry for the data-driven charts: pure functions over a `Cube`.
//! Every geometry is in a 640x320 viewBox with fixed margins: plot area
//! x 56..624, y 16..280 (left 56, right 16, top 16, bottom 40).

use eona_ui_toolkit::chart::{
    bar_layout, donut_of, format_number, format_with_unit, line_layout, metric_of, nice_ticks,
    stacked_layout, ScoreBand, Size, Trend, VIEW,
};
use eona_ui_toolkit::cube::{Axis, AxisKey, Cube, DateKind, LangMap, Obs};

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 0.01, "{a} != {b}");
}

fn cube(axis: Axis, series: &[&str], unit: Option<&str>, obs: &[(&str, Option<usize>, f64)]) -> Cube {
    Cube {
        id: String::new(),
        title: LangMap(vec![(String::new(), "t".into())]),
        description: None,
        axis,
        unit: unit.map(str::to_owned),
        series: series.iter().map(|s| s.to_string()).collect(),
        observations: obs
            .iter()
            .map(|(k, s, v)| Obs {
                key: match axis {
                    Axis::Category => AxisKey::Category(k.to_string()),
                    Axis::Date(_) => AxisKey::Date(k.to_string()),
                },
                series: *s,
                value: *v,
                order: None,
            })
            .collect(),
    }
}

const MONTH: Axis = Axis::Date(DateKind::YearMonth);

#[test]
fn view_is_640_by_320() {
    assert_eq!((VIEW.w, VIEW.h), (640.0, 320.0));
}

#[test]
fn nice_ticks_table() {
    assert_eq!(nice_ticks(0.0, 97.0, 5), vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    assert_eq!(nice_ticks(0.0, 4.0, 5), vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    assert_eq!(nice_ticks(-10.0, 30.0, 5), vec![-10.0, 0.0, 10.0, 20.0, 30.0]);
    assert_eq!(nice_ticks(0.0, 0.3, 3), vec![0.0, 0.1, 0.2, 0.3]);
    assert_eq!(
        nice_ticks(0.0, 1_000_000.0, 4),
        vec![0.0, 200_000.0, 400_000.0, 600_000.0, 800_000.0, 1_000_000.0]
    );
    // an empty range is widened by one unit
    assert_eq!(nice_ticks(0.0, 0.0, 5), vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]);
}

#[test]
fn format_number_groups_thousands_with_a_thin_space() {
    assert_eq!(format_number(128430.0), "128\u{2009}430");
    assert_eq!(format_number(999.0), "999");
    assert_eq!(format_number(1234567.891), "1\u{2009}234\u{2009}567.89");
    assert_eq!(format_number(-1500.5), "-1\u{2009}500.5");
    assert_eq!(format_number(0.0), "0");
    assert_eq!(format_number(-0.001), "0");
}

#[test]
fn format_with_unit_appends_the_unit() {
    assert_eq!(format_with_unit(1234.0, Some("pax")), "1\u{2009}234 pax");
    assert_eq!(format_with_unit(45.0, Some("%")), "45%");
    assert_eq!(format_with_unit(45.0, None), "45");
}

#[test]
fn bar_layout_one_series() {
    let c = cube(
        Axis::Category,
        &[],
        Some("pax"),
        &[("A", None, 10.0), ("B", None, 20.0), ("C", None, 40.0)],
    );
    let g = bar_layout(&c, VIEW);
    assert_eq!(g.bars.len(), 3);
    approx(g.zero_y, 280.0);
    // band 189.33, 10% padding each side
    let expect = [(74.93, 214.0, 66.0), (264.27, 148.0, 132.0), (453.6, 16.0, 264.0)];
    for (b, (x, y, h)) in g.bars.iter().zip(expect) {
        approx(b.x, x);
        approx(b.y, y);
        approx(b.w, 151.47);
        approx(b.h, h);
        assert_eq!(b.series, None);
    }
    let labels: Vec<_> = g.bars.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["A", "B", "C"]);
    assert_eq!(g.bars[0].value_label, "10 pax");
    let xl: Vec<_> = g.x_labels.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(xl, ["A", "B", "C"]);
    approx(g.x_labels[0].pos, 150.67);
    let yt: Vec<_> = g.y_ticks.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(yt, ["0", "10", "20", "30", "40"]);
    approx(g.y_ticks[0].pos, 280.0);
    approx(g.y_ticks[4].pos, 16.0);
}

#[test]
fn grouped_bars_split_the_band() {
    let c = cube(
        Axis::Category,
        &["s0", "s1"],
        None,
        &[
            ("A", Some(0), 10.0),
            ("A", Some(1), 20.0),
            ("B", Some(0), 20.0),
            ("B", Some(1), 40.0),
        ],
    );
    let g = bar_layout(&c, VIEW);
    assert_eq!(g.bars.len(), 4);
    for b in &g.bars {
        approx(b.w, 113.6);
    }
    approx(g.bars[0].x, 84.4);
    approx(g.bars[1].x, 198.0);
    approx(g.bars[2].x, 368.4);
    approx(g.bars[3].x, 482.0);
    assert_eq!(g.bars[1].series, Some(1));
    // a missing series leaves its slot empty
    let c = cube(
        Axis::Category,
        &["s0", "s1"],
        None,
        &[("A", Some(0), 10.0), ("B", Some(0), 20.0), ("B", Some(1), 40.0)],
    );
    let g = bar_layout(&c, VIEW);
    assert_eq!(g.bars.len(), 3);
    approx(g.bars[1].x, 368.4);
}

#[test]
fn negative_bar_sits_below_zero() {
    let c = cube(Axis::Category, &[], None, &[("A", None, -10.0), ("B", None, 30.0)]);
    let g = bar_layout(&c, VIEW);
    approx(g.zero_y, 214.0);
    approx(g.bars[0].y, 214.0);
    approx(g.bars[0].h, 66.0);
    assert!(g.bars[0].y >= g.zero_y);
    approx(g.bars[1].y, 16.0);
    approx(g.bars[1].h, 198.0);
}

#[test]
fn empty_cube_has_no_bars() {
    let c = cube(Axis::Category, &[], None, &[]);
    let g = bar_layout(&c, VIEW);
    assert!(g.bars.is_empty());
    assert!(g.x_labels.is_empty());
    assert!(!g.y_ticks.is_empty());
}

#[test]
fn stacked_layout_sums_per_category() {
    let c = cube(
        Axis::Category,
        &["s0", "s1"],
        None,
        &[
            ("A", Some(0), 1.0),
            ("A", Some(1), 2.0),
            ("B", Some(0), 3.0),
            ("B", Some(1), 1.0),
        ],
    );
    let g = stacked_layout(&c, VIEW).unwrap();
    let yt: Vec<_> = g.y_ticks.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(yt, ["0", "1", "2", "3", "4"]);
    let r = |i: usize| (g.bars[i].y, g.bars[i].h);
    let expect = [(214.0, 66.0), (82.0, 132.0), (82.0, 198.0), (16.0, 66.0)];
    for (i, (y, h)) in expect.into_iter().enumerate() {
        approx(r(i).0, y);
        approx(r(i).1, h);
    }
    approx(g.bars[0].x, g.bars[1].x);
    approx(g.bars[0].w, 227.2);
}

#[test]
fn stacked_layout_rejects_negative_values() {
    let c = cube(
        Axis::Category,
        &["s0"],
        None,
        &[("A", Some(0), 1.0), ("B", Some(0), -2.0)],
    );
    let e = stacked_layout(&c, VIEW).unwrap_err();
    assert!(e.message.contains("non-negative"), "{e}");
}

#[test]
fn line_layout_sorts_dates_and_splits_at_a_missing_point() {
    let c = cube(
        MONTH,
        &["a", "b"],
        None,
        &[
            ("2025-03", Some(0), 30.0),
            ("2025-01", Some(0), 10.0),
            ("2025-02", Some(0), 20.0),
            ("2025-03", Some(1), 15.0),
            ("2025-01", Some(1), 5.0),
        ],
    );
    let g = line_layout(&c, VIEW);
    assert_eq!(g.paths.len(), 2);
    assert_eq!(g.paths[0].d, "M150.67 192 L340 104 L529.33 16");
    assert_eq!(g.paths[0].points.len(), 3);
    assert_eq!(g.paths[1].d, "M150.67 236 M529.33 148");
    assert_eq!(g.paths[1].points.len(), 2);
    assert_eq!(g.paths[1].series, Some(1));
    let xl: Vec<_> = g.x_labels.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(xl, ["2025-01", "2025-02", "2025-03"]);
    approx(g.zero_y, 280.0);
    assert_eq!(g.paths[0].points[0].value_label, "10");
}

#[test]
fn donut_of_selects_by_key_and_defaults_to_the_latest() {
    let c = cube(
        Axis::Category,
        &[],
        None,
        &[("Completeness", None, 0.82), ("Validity", None, 0.4)],
    );
    let d = donut_of(&c, Some("Validity")).unwrap();
    assert_eq!(d.label, "40.0%");
    assert_eq!(d.band, ScoreBand::Danger);
    assert!(donut_of(&c, None).is_err(), "several categories: a key is needed");
    let e = donut_of(&c, Some("Nope")).unwrap_err();
    assert!(e.message.contains("Nope"), "{e}");

    let c = cube(
        MONTH,
        &[],
        None,
        &[("2025-01", None, 0.5), ("2025-03", None, 0.9), ("2025-02", None, 0.7)],
    );
    // observations are taken as given sorted: the latest date wins either way
    assert_eq!(donut_of(&c, None).unwrap().label, "90.0%");
    assert_eq!(donut_of(&c, Some("2025-02")).unwrap().label, "70.0%");

    let c = cube(Axis::Category, &[], None, &[("Only", None, 0.82)]);
    assert_eq!(donut_of(&c, None).unwrap().label, "82.0%");
    // clamped like donut_arc
    let c = cube(Axis::Category, &[], None, &[("Only", None, 1.7)]);
    assert_eq!(donut_of(&c, None).unwrap().label, "100.0%");
    assert!(donut_of(&cube(Axis::Category, &[], None, &[]), None).is_err());
}

#[test]
fn metric_of_reports_latest_delta_and_trend() {
    let c = cube(MONTH, &[], None, &[("2025-02", None, 80.0), ("2025-01", None, 100.0)]);
    let m = metric_of(&c).unwrap();
    assert_eq!(m.value, 80.0);
    assert_eq!(m.delta, Some(-20.0));
    assert_eq!(m.trend, Trend::Down);

    let c = cube(MONTH, &[], None, &[("2025-01", None, 100.0), ("2025-02", None, 130.0)]);
    let m = metric_of(&c).unwrap();
    assert_eq!((m.value, m.delta, m.trend), (130.0, Some(30.0), Trend::Up));

    let c = cube(MONTH, &[], None, &[("2025-01", None, 100.0)]);
    let m = metric_of(&c).unwrap();
    assert_eq!((m.value, m.delta, m.trend), (100.0, None, Trend::Flat));

    assert!(metric_of(&cube(MONTH, &[], None, &[])).is_err());
}

#[test]
fn single_value_views_reject_several_series() {
    let c = cube(
        Axis::Category,
        &["a", "b"],
        None,
        &[("X", Some(0), 0.1), ("X", Some(1), 0.2)],
    );
    assert!(donut_of(&c, Some("X")).is_err());
    assert!(metric_of(&c).is_err());
}

#[test]
fn size_is_a_plain_struct() {
    let s = Size { w: 640.0, h: 320.0 };
    assert_eq!(s, VIEW);
}
