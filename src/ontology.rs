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
/// datatype IRI. Both `Option`s are matrix axes — `OntoAnnotation` renders a
/// different trailing badge for each of the three combinations it can see.
#[derive(Clone, Debug, PartialEq)]
pub struct LiteralValue {
    pub value: String,
    pub language: Option<String>,
    pub datatype: Option<String>,
}
