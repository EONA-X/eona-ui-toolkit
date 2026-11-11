//! The chart components rendered to HTML, from the fixtures in `tests/fixtures/cube/`.

#[cfg(feature = "charts")]
mod ssr {
    use eona_ui_toolkit::chart::{donut_arc, ScoreBand};
    use eona_ui_toolkit::charts::{
        BarChart, BarChartProps, CubeMetricTile, CubeMetricTileProps, CubeScoreDonut,
        CubeScoreDonutProps, LineChart, LineChartProps, StackedBarChart, StackedBarChartProps,
    };
    use eona_ui_toolkit::{COMPONENTS_CSS, TOKENS_CSS};
    use std::fs;
    use std::path::PathBuf;
    use yew::prelude::*;
    use yew::ServerRenderer;

    fn fixture(kind: &str, name: &str) -> String {
        fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/cube")
                .join(kind)
                .join(format!("{name}.jsonld")),
        )
        .unwrap()
    }

    #[derive(Clone, Copy, Debug)]
    enum Kind {
        Bar,
        Stacked,
        Line,
        Donut,
        Metric,
    }
    const ALL: [Kind; 5] = [Kind::Bar, Kind::Stacked, Kind::Line, Kind::Donut, Kind::Metric];
    const SVG_KINDS: [Kind; 3] = [Kind::Bar, Kind::Stacked, Kind::Line];

    async fn render(kind: Kind, jsonld: &str, fetch_error: Option<&str>, lang: Option<&str>) -> String {
        // Props hold `Rc` strings and are not `Send`; build them inside the closure.
        let jsonld = jsonld.to_owned();
        let fetch_error = fetch_error.map(str::to_owned);
        let lang = lang.map(str::to_owned);
        let attrs = move || {
            (
                AttrValue::from(jsonld),
                fetch_error.map(AttrValue::from),
                lang.map(AttrValue::from),
            )
        };
        match kind {
            Kind::Bar => {
                ServerRenderer::<BarChart>::with_props(move || {
                    let (jsonld, fetch_error, lang) = attrs();
                    BarChartProps { jsonld, fetch_error, lang }
                })
                .render()
                .await
            }
            Kind::Stacked => {
                ServerRenderer::<StackedBarChart>::with_props(move || {
                    let (jsonld, fetch_error, lang) = attrs();
                    StackedBarChartProps { jsonld, fetch_error, lang }
                })
                .render()
                .await
            }
            Kind::Line => {
                ServerRenderer::<LineChart>::with_props(move || {
                    let (jsonld, fetch_error, lang) = attrs();
                    LineChartProps { jsonld, fetch_error, lang }
                })
                .render()
                .await
            }
            Kind::Donut => {
                ServerRenderer::<CubeScoreDonut>::with_props(move || {
                    let (jsonld, fetch_error, lang) = attrs();
                    CubeScoreDonutProps { jsonld, fetch_error, lang, select: None }
                })
                .render()
                .await
            }
            Kind::Metric => {
                ServerRenderer::<CubeMetricTile>::with_props(move || {
                    let (jsonld, fetch_error, lang) = attrs();
                    CubeMetricTileProps { jsonld, fetch_error, lang, help: None }
                })
                .render()
                .await
            }
        }
    }

    /// Rows of the data table body.
    fn body_rows(html: &str) -> usize {
        let body = html.split("<tbody>").nth(1).expect("a tbody");
        body.split("</tbody>").next().unwrap().matches("<tr").count()
    }

    #[tokio::test]
    async fn each_svg_chart_has_role_img_title_and_one_table_row_per_observation() {
        for (kind, name, rows) in [
            (Kind::Bar, "bar-category", 3),
            (Kind::Stacked, "line-series-dates", 5),
            (Kind::Line, "line-series-dates", 5),
            (Kind::Bar, "dates-iso", 2),
        ] {
            let html = render(kind, &fixture("conforming", name), None, None).await;
            assert!(html.contains(r#"<svg"#) && html.contains(r#"role="img""#), "{kind:?} {name}: {html}");
            assert!(html.contains("aria-labelledby="), "{kind:?}: {html}");
            assert!(html.contains("<title"), "{kind:?}: {html}");
            assert!(html.contains("<desc"), "{kind:?}: {html}");
            assert!(html.contains(r#"<details class="chart__data">"#), "{kind:?}: {html}");
            assert!(html.contains("<table"), "{kind:?}: {html}");
            assert_eq!(body_rows(&html), rows, "{kind:?} {name}: {html}");
        }
    }

    #[tokio::test]
    async fn the_svg_is_named_by_its_title_and_described_by_its_desc() {
        let html = render(Kind::Bar, &fixture("conforming", "bar-category"), None, None).await;
        let id = html.split("aria-labelledby=\"").nth(1).unwrap().split('"').next().unwrap();
        assert!(id.ends_with("-title") && !id.contains(' '), "labelledby is the title alone: {id}");
        let described = html.split("aria-describedby=\"").nth(1).expect("aria-describedby").split('"').next().unwrap();
        assert_eq!(described, id.replace("-title", "-desc"), "{html}");
        assert!(html.contains(&format!(r#"<desc id="{described}""#)), "{html}");
    }

    #[tokio::test]
    async fn title_is_the_dataset_label_and_desc_the_comment() {
        let html = render(Kind::Bar, &fixture("conforming", "bar-category"), None, None).await;
        assert!(html.contains(">Visitors per airport</title>"), "{html}");
        assert!(html.contains(">Passengers, 2025</desc>"), "{html}");
    }

    #[tokio::test]
    async fn series_charts_carry_a_series_class_per_series() {
        let html = render(Kind::Line, &fixture("conforming", "line-series-dates"), None, None).await;
        assert!(html.contains("chart__series--1") && html.contains("chart__series--2"), "{html}");
        assert!(!html.contains("chart__series--3"), "{html}");
        assert!(!html.contains("style=\"fill"), "no inline fills: {html}");
    }

    #[tokio::test]
    async fn violating_fixture_renders_the_parser_message_as_an_alert() {
        let html = render(Kind::Bar, &fixture("violating", "remote-context"), None, None).await;
        assert!(html.contains("chart--error"), "{html}");
        assert!(html.contains(r#"role="alert""#), "{html}");
        assert!(html.contains("remote contexts are not fetched"), "{html}");
        assert!(!html.contains("<svg"), "{html}");
    }

    #[tokio::test]
    async fn fetch_error_renders_that_message_in_every_component() {
        for kind in ALL {
            let html = render(kind, "", Some("HTTP 404 for /data/cube.jsonld"), None).await;
            assert!(html.contains("chart--error"), "{kind:?}: {html}");
            assert!(html.contains(r#"role="alert""#), "{kind:?}: {html}");
            assert!(html.contains("HTTP 404 for /data/cube.jsonld"), "{kind:?}: {html}");
            // No dataset was read, so the alert is named generically.
            assert!(html.contains(r#"<strong class="chart__state-label">Chart</strong>"#), "{kind:?}: {html}");
        }
    }

    #[tokio::test]
    async fn an_unreadable_dataset_is_named_generically_and_a_failed_draw_by_its_label() {
        let html = render(Kind::Bar, &fixture("violating", "remote-context"), None, None).await;
        assert!(html.contains(r#"<strong class="chart__state-label">Chart</strong>"#), "{html}");
        let html = render(Kind::Stacked, &fixture("conforming", "years-negative"), None, None).await;
        assert!(html.contains(r#"<strong class="chart__state-label">T</strong>"#), "{html}");
        let html = render(Kind::Stacked, &fixture("conforming", "years-negative"), None, Some("fr")).await;
        assert!(html.contains("chart__state-label"), "{html}");
    }

    #[tokio::test]
    async fn zero_observations_render_the_empty_state() {
        for kind in ALL {
            let html = render(kind, &fixture("conforming", "empty"), None, None).await;
            assert!(html.contains("chart--empty"), "{kind:?}: {html}");
            assert!(html.contains("No observations"), "{kind:?}: {html}");
            assert!(html.contains(r#"<strong class="chart__state-label">T</strong>"#), "the dataset label: {kind:?}: {html}");
            assert!(!html.contains("chart--error"), "{kind:?}: {html}");
        }
    }

    #[tokio::test]
    async fn stacked_with_a_negative_value_is_an_error_not_a_panic() {
        let html = render(Kind::Stacked, &fixture("conforming", "years-negative"), None, None).await;
        assert!(html.contains("chart--error"), "{html}");
    }

    #[tokio::test]
    async fn donut_renders_score_donut_markup_for_the_value() {
        let html = render(Kind::Donut, &fixture("conforming", "donut-single"), None, None).await;
        let arc = donut_arc(0.82);
        assert_eq!(arc.band, ScoreBand::Good);
        assert!(html.contains("chart-donut"), "{html}");
        assert!(html.contains(&format!(r#"stroke-dasharray="{}""#, arc.dash_array)), "{html}");
        assert!(html.contains("chart-donut__value--good"), "{html}");
        assert!(html.contains("82.0%"), "{html}");
    }

    #[tokio::test]
    async fn donut_with_several_categories_and_no_select_is_an_error() {
        let html = render(Kind::Donut, &fixture("conforming", "bar-category"), None, None).await;
        assert!(html.contains("chart--error"), "{html}");
        assert!(html.contains("name the one to show"), "{html}");
    }

    #[tokio::test]
    async fn donut_select_picks_the_category() {
        let doc = fixture("conforming", "bar-category");
        let html = ServerRenderer::<CubeScoreDonut>::with_props(move || CubeScoreDonutProps {
            jsonld: AttrValue::from(doc),
            fetch_error: None,
            lang: None,
            select: Some(AttrValue::from("Lyon")),
        })
        .render()
        .await;
        // 128430 is clamped to a full ring.
        assert!(html.contains("100.0%"), "{html}");
    }

    #[tokio::test]
    async fn metric_tile_shows_the_latest_value_and_an_up_badge() {
        let html = render(Kind::Metric, &fixture("conforming", "dates-iso"), None, None).await;
        assert!(html.contains("metric-tile"), "{html}");
        assert!(html.contains(r#"class="metric-tile__value">3<"#), "{html}");
        assert!(html.contains("evolution-badge--up"), "{html}");
        assert!(html.contains("+2"), "{html}");
    }

    #[tokio::test]
    async fn metric_help_is_rendered() {
        let doc = fixture("conforming", "dates-iso");
        let html = ServerRenderer::<CubeMetricTile>::with_props(move || CubeMetricTileProps {
            jsonld: AttrValue::from(doc),
            fetch_error: None,
            lang: None,
            help: Some(AttrValue::from("vs previous month")),
        })
        .render()
        .await;
        assert!(html.contains("vs previous month"), "{html}");
    }

    #[tokio::test]
    async fn lang_picks_the_matching_label() {
        let doc = fixture("conforming", "donut-single");
        let fr = render(Kind::Donut, &doc, None, Some("fr")).await;
        assert!(fr.contains("Qualité"), "{fr}");
        let en = render(Kind::Donut, &doc, None, Some("en")).await;
        assert!(en.contains("Quality") && !en.contains("Qualité"), "{en}");
        let svg = render(Kind::Bar, &doc, None, Some("fr")).await;
        assert!(svg.contains(">Qualité</title>"), "{svg}");
    }

    #[tokio::test]
    async fn every_emitted_class_is_defined_in_components_css() {
        let mut checked = std::collections::BTreeSet::new();
        for name in ["bar-category", "line-series-dates", "donut-single", "dates-iso", "empty"] {
            for kind in ALL {
                let doc = fixture("conforming", name);
                let mut pages = vec![render(kind, &doc, None, None).await];
                pages.push(render(kind, "", Some("boom"), None).await);
                for html in pages {
                    for chunk in html.split(r#" class=""#).skip(1) {
                        let value = chunk.split('"').next().unwrap();
                        for class in value.split_whitespace() {
                            checked.insert(class.to_owned());
                        }
                    }
                }
            }
        }
        for class in &checked {
            assert!(
                COMPONENTS_CSS.contains(&format!(".{class}")),
                "charts emit `.{class}`, which components.css does not define"
            );
        }
        assert!(checked.contains("chart--error") && checked.contains("chart__data"));
    }

    #[test]
    fn series_tokens_exist_in_light_and_dark() {
        // The dark block is the `:root[data-theme="dark"]` rule.
        let dark_at = TOKENS_CSS.find(r#":root[data-theme="dark"]"#).expect("dark block");
        let (light, dark) = TOKENS_CSS.split_at(dark_at);
        for n in 1..=8 {
            let token = format!("--eona-chart-series-{n}:");
            assert!(light.contains(&token), "light: {token}");
            assert!(dark.contains(&token), "dark: {token}");
            assert!(
                COMPONENTS_CSS.contains(&format!(".chart__series--{n}")),
                "components.css lacks .chart__series--{n}"
            );
        }
    }

    #[tokio::test]
    async fn svg_charts_set_a_viewbox() {
        for kind in SVG_KINDS {
            let html = render(kind, &fixture("conforming", "bar-category"), None, None).await;
            assert!(html.contains(r#"viewBox="0 0 640 320""#), "{kind:?}: {html}");
        }
    }
}
