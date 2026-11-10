//! The canonical fixtures of the chart profile (copied from `eonax-chart-profile/tests/`
//! of eona-vocabularies-reference into `tests/fixtures/cube/profile/`): the reader
//! must accept every conforming one and reject every violating one, as the SHACL
//! shapes do. The CI job in `tests/differential.sh` compares the two verdicts and the
//! copies byte for byte; this test is the part that needs no Python.

#[cfg(feature = "charts")]
mod profile {
    use eona_ui_toolkit::cube::from_jsonld;
    use std::fs;
    use std::path::PathBuf;

    fn files(kind: &str) -> Vec<PathBuf> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube/profile").join(kind);
        let mut v: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "jsonld"))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn the_reader_accepts_every_conforming_fixture() {
        let all = files("conforming");
        assert!(all.len() >= 11, "{} fixtures", all.len());
        for p in all {
            let text = fs::read_to_string(&p).unwrap();
            from_jsonld(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        }
    }

    #[test]
    fn the_reader_rejects_every_violating_fixture() {
        let all = files("violating");
        assert!(all.len() >= 17, "{} fixtures", all.len());
        for p in all {
            let text = fs::read_to_string(&p).unwrap();
            assert!(from_jsonld(&text).is_err(), "{} was accepted", p.display());
        }
    }

    #[test]
    fn the_short_term_context_resolves_through_the_builtin_context() {
        // The vocabulary's own check expands this document with the context file;
        // here the reader does it with its embedded copy, naming the built-in URL.
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube/profile/context/short-terms.jsonld");
        let text = fs::read_to_string(p)
            .unwrap()
            .replace("\"context.jsonld\"", "\"https://eona-x.eu/shape/chart-profile/context.jsonld\"");
        let cube = from_jsonld(&text).unwrap();
        assert_eq!(cube.observations.len(), 2);
    }
}
