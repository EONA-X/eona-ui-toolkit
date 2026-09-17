//! The two RDF/OWL model types that survive in the toolkit.
//!
//! The rest of this module — `TermRef`, `Term`, `OntologyModel`, `PREFIXES`,
//! `parse_ontology` and the JSON-LD machinery — moved back to
//! `eona-vocabulary-ui`, which was its only consumer, together with the
//! `interactive` tier that rendered it. Originating commit: `8c37e76`
//! ("feat: add the interactive ontology tier, theming, and a PatternFly skin").
//!
//! These two could not go with it. `TermKind` is the payload of
//! `BadgeVariant::Kind` (`src/atoms/onto_badge.rs:21`), whose eight arms the
//! React generator flattens into `OntoBadge.tsx`'s twelve-member union;
//! `LiteralValue` is the element type of `OntoAnnotationProps::values`
//! (`src/molecules/annotation.rs:17`), and `annotation.rs:33-41` branches three
//! ways on its two `Option` fields. Both components stay, so both types stay —
//! and stay *here* rather than moving into those two files, because inserting
//! them there would shift the line numbers the generated `.tsx` cites.
//!
//! Ungated, as before: neither type carries state or a renderer assumption, and
//! an `ssr` consumer needs them to construct either component.

/// The RDF/OWL entity kinds `OntoBadge` gives a charter hue.
///
/// Variant order is load-bearing: `xtask` flattens it into the TypeScript union
/// in `packages/react/src/OntoBadge.tsx`, member for member.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TermKind {
    Ontology,
    Class,
    ObjectProperty,
    DatatypeProperty,
    AnnotationProperty,
    Property,
    NamedIndividual,
    Other,
}

/// One RDF literal: its lexical form plus at most one of a language tag or a
/// datatype IRI.
///
/// `language` and `datatype_label` are the matrix axes — `OntoAnnotation`
/// renders a different trailing badge for each combination. `datatype` itself is
/// carried as data and never drives markup; see `datatype_label`.
#[derive(Clone, Debug, PartialEq)]
pub struct LiteralValue {
    pub value: String,
    pub language: Option<String>,
    pub datatype: Option<String>,
    /// The datatype's local name, precomputed by [`LiteralValue::new`].
    ///
    /// Carried rather than derived while rendering: `OntoAnnotation` used to
    /// call `short_datatype` on `datatype` in its view, which made its markup a
    /// transform of the prop and quarantined it out of the React distribution.
    /// Worse, the bug was invisible for a while — the probe alphabet was
    /// digits-only, on which `short_datatype` is the identity. See
    /// eona-x/backlog#822.
    pub datatype_label: Option<String>,
}

/// A datatype IRI's local name: whatever follows its last `#` or `/`, or the
/// whole IRI if it has neither. Mirrors the Vue source's `shortDatatype`.
pub fn short_datatype(datatype: &str) -> &str {
    match datatype.rfind('#').or_else(|| datatype.rfind('/')) {
        Some(i) => &datatype[i + 1..],
        None => datatype,
    }
}

impl LiteralValue {
    /// Builds a literal, shortening the datatype IRI once, here, rather than on
    /// every render.
    pub fn new(value: String, language: Option<String>, datatype: Option<String>) -> Self {
        let datatype_label = datatype.as_deref().map(|d| short_datatype(d).to_string());
        Self { value, language, datatype, datatype_label }
    }
}
