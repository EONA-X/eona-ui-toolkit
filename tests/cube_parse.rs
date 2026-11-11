//! The chart-profile JSON-LD reader. Fixtures are in `tests/fixtures/cube/`;
//! each violating one carries a `.expect` file with a fragment of the error.

#[cfg(feature = "charts")]
mod parse {
    use eona_ui_toolkit::cube::{from_jsonld, Axis, AxisKey, Cube, DateKind, MAX_OBSERVATIONS};
    use std::fs;
    use std::path::PathBuf;

    fn dir(kind: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/cube")
            .join(kind)
    }

    fn load(kind: &str, name: &str) -> String {
        fs::read_to_string(dir(kind).join(format!("{name}.jsonld"))).unwrap()
    }

    fn parse(name: &str) -> Cube {
        from_jsonld(&load("conforming", name)).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    fn keys(c: &Cube) -> Vec<(&str, Option<usize>, f64)> {
        c.observations
            .iter()
            .map(|o| (o.key.as_str(), o.series, o.value))
            .collect()
    }

    #[test]
    fn every_conforming_fixture_parses() {
        let mut n = 0;
        for e in fs::read_dir(dir("conforming")).unwrap() {
            let p = e.unwrap().path();
            let s = fs::read_to_string(&p).unwrap();
            from_jsonld(&s).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            n += 1;
        }
        assert!(n >= 8);
    }

    #[test]
    fn bar_category_sorts_lexically() {
        let c = parse("bar-category");
        assert_eq!(c.id, "https://example.org/cube/visitors");
        assert_eq!(c.title.get(None), Some("Visitors per airport"));
        assert_eq!(
            c.description.as_ref().unwrap().get(None),
            Some("Passengers, 2025")
        );
        assert_eq!(c.axis, Axis::Category);
        assert_eq!(c.unit.as_deref(), Some("pax"));
        assert!(c.series.is_empty());
        assert_eq!(
            keys(&c),
            vec![
                ("Lyon", None, 128430.0),
                ("Nice", None, 96210.5),
                ("Paris", None, 210000.0)
            ]
        );
    }

    #[test]
    fn three_forms_give_equal_cubes() {
        let a = parse("bar-category");
        assert_eq!(a, parse("bar-category-inline"));
        assert_eq!(a, parse("bar-category-expanded"));
    }

    #[test]
    fn line_series_dates() {
        let c = parse("line-series-dates");
        assert_eq!(c.axis, Axis::Date(DateKind::YearMonth));
        assert_eq!(c.series, vec!["Arrivals", "Departures"]);
        assert_eq!(c.unit.as_deref(), Some("k"));
        assert_eq!(
            keys(&c),
            vec![
                ("2025-01", Some(0), 10.0),
                ("2025-01", Some(1), 12.0),
                ("2025-02", Some(0), 20.0),
                ("2025-03", Some(0), 30.0),
                ("2025-03", Some(1), 31.0),
            ]
        );
        assert!(matches!(c.observations[0].key, AxisKey::Date(_)));
    }

    #[test]
    fn language_tagged_title() {
        let c = parse("donut-single");
        assert_eq!(c.title.get(Some("fr")), Some("Qualité"));
        assert_eq!(c.title.get(Some("fr-CA")), Some("Qualité"));
        assert_eq!(c.title.get(Some("en")), Some("Quality"));
        assert_eq!(c.title.get(Some("de")), Some("Qualité"));
        assert_eq!(c.observations.len(), 1);
        assert_eq!(c.observations[0].value, 0.82);
    }

    #[test]
    fn order_then_lexical_unordered_last() {
        let c = parse("ordered");
        let k: Vec<_> = c.observations.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(k, vec!["Low", "Medium", "High", "Unknown"]);
        assert_eq!(c.observations[0].order, Some(1));
        assert_eq!(c.observations[3].order, None);
    }

    #[test]
    fn dates_and_years() {
        let c = parse("dates-iso");
        assert_eq!(c.axis, Axis::Date(DateKind::Date));
        assert_eq!(keys(&c)[0].0, "2025-01-15");
        let c = parse("years-negative");
        assert_eq!(c.axis, Axis::Date(DateKind::Year));
        assert_eq!(keys(&c), vec![("2023", None, 7.0), ("2025", None, -4.5)]);
    }

    #[test]
    fn zero_observations_is_ok_and_empty() {
        let c = parse("empty");
        assert!(c.observations.is_empty());
    }

    #[test]
    fn too_many_observations() {
        let obs = |n: usize| {
            let items: Vec<String> = (0..n)
                .map(|i| format!(r#"{{"@type":"Observation","category":"c{i}","value":{i}}}"#))
                .collect();
            format!(
                r#"{{"@context":"https://eona-x.eu/shape/chart-profile/context.jsonld","@type":"DataSet","label":"x","observations":[{}]}}"#,
                items.join(",")
            )
        };
        assert_eq!(
            from_jsonld(&obs(MAX_OBSERVATIONS)).unwrap().observations.len(),
            MAX_OBSERVATIONS
        );
        let e = from_jsonld(&obs(MAX_OBSERVATIONS + 1)).unwrap_err();
        assert!(e.message.contains("10000"), "{e}");
    }

    #[test]
    fn every_violating_fixture_is_rejected_with_its_fragment() {
        let mut n = 0;
        for e in fs::read_dir(dir("violating")).unwrap() {
            let p = e.unwrap().path();
            if p.extension().and_then(|x| x.to_str()) != Some("jsonld") {
                continue;
            }
            let expect = fs::read_to_string(p.with_extension("expect")).unwrap();
            let expect = expect.trim();
            let err = from_jsonld(&fs::read_to_string(&p).unwrap())
                .expect_err(&format!("{} should be rejected", p.display()));
            assert!(
                err.message.contains(expect),
                "{}: {:?} does not contain {:?}",
                p.display(),
                err.message,
                expect
            );
            n += 1;
        }
        assert!(n >= 18);
    }

    #[test]
    fn repeated_identical_values_are_one_triple() {
        let c = parse("repeated-identical-values");
        assert_eq!(keys(&c), vec![("a", None, 1.0)]);
    }

    #[test]
    fn dates_take_a_timezone_and_a_negative_year_and_sort_by_calendar_day() {
        let c = parse("dates-timezone-negative-year");
        assert_eq!(c.observations[0].key.as_str(), "2023-12-31Z");
        let c = parse("years-bce");
        let ys: Vec<&str> = c.observations.iter().map(|o| o.key.as_str()).collect();
        assert_eq!(ys, ["-0100", "-0044", "2024Z"]);
    }

    #[test]
    fn remote_context_is_never_fetched() {
        let e = from_jsonld(r#"{"@context":"https://example.org/x.jsonld","@type":"x"}"#)
            .unwrap_err();
        assert!(e.message.contains("remote contexts are not fetched"), "{e}");
        assert_eq!(e.path, "/@context");
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(from_jsonld("{").unwrap_err().message.contains("invalid JSON"));
    }
}

/// `yew` itself reaches `serde_json` through `gloo`, so the check is on this crate's
/// own direct dependencies: `serde_json` is there with `charts` and only then.
#[test]
fn serde_json_is_a_direct_dependency_only_with_charts() {
    let direct = |feature: Option<&str>| -> bool {
        let mut cmd = std::process::Command::new(env!("CARGO"));
        cmd.args([
            "tree", "-e", "normal", "--depth", "1", "--prefix", "none", "--offline", "-p",
            "eona-ui-toolkit",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"));
        if let Some(f) = feature {
            cmd.args(["--features", f]);
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .any(|l| l.starts_with("serde_json "))
    };
    assert!(!direct(None));
    assert!(!direct(Some("ssr")));
    assert!(!direct(Some("csr")));
    assert!(direct(Some("charts")));
}
