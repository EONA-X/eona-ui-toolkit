//! The data model behind the data-driven charts: one RDF Data Cube dataset in the
//! chart profile (`https://eona-x.eu/shape/chart-profile#`), reduced to what a chart
//! draws.
//!
//! The model is plain Rust and always compiled. The JSON-LD reader that fills it,
//! [`from_jsonld`], needs `serde_json` and sits behind the `charts` feature, so a
//! default, `ssr` or `csr` build stays `yew`-only.

#[cfg(feature = "charts")]
mod jsonld;

#[cfg(feature = "charts")]
pub use jsonld::from_jsonld;

use std::fmt;

/// The context a document may name instead of spelling out its own. Never fetched:
/// the reader holds a copy.
pub const CONTEXT_URL: &str = "https://eona-x.eu/shape/chart-profile/context.jsonld";

/// At most this many observations are accepted.
pub const MAX_OBSERVATIONS: usize = 10_000;

/// At most this many distinct series are accepted.
pub const MAX_SERIES: usize = 8;

/// Language-tagged strings. The tag is empty for a string without a language.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LangMap(pub Vec<(String, String)>);

impl LangMap {
    /// The text for `lang` (exact tag, then its primary subtag), else the untagged
    /// text, else the first entry.
    pub fn get(&self, lang: Option<&str>) -> Option<&str> {
        let find = |tag: &str| self.0.iter().find(|(l, _)| l == tag).map(|(_, t)| t.as_str());
        if let Some(l) = lang {
            if let Some(t) = find(l) {
                return Some(t);
            }
            if let Some(t) = l.split('-').next().and_then(find) {
                return Some(t);
            }
        }
        find("").or_else(|| self.0.first().map(|(_, t)| t.as_str()))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The lexical form of a `chart:date`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateKind {
    /// `xsd:date`, `2025-03-01`
    Date,
    /// `xsd:gYearMonth`, `2025-03`
    YearMonth,
    /// `xsd:gYear`, `2025`
    Year,
}

/// What the observations are laid out along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Category,
    Date(DateKind),
}

/// The position of one observation on the axis. Dates keep their lexical form,
/// which sorts chronologically within one [`DateKind`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AxisKey {
    Category(String),
    Date(String),
}

impl AxisKey {
    pub fn as_str(&self) -> &str {
        match self {
            AxisKey::Category(s) | AxisKey::Date(s) => s,
        }
    }
}

/// One `qb:Observation`.
#[derive(Clone, Debug, PartialEq)]
pub struct Obs {
    pub key: AxisKey,
    /// Index into [`Cube::series`].
    pub series: Option<usize>,
    pub value: f64,
    pub order: Option<i64>,
}

/// A chart dataset. `observations` come sorted: categories by `chart:order` (those
/// without one last) then lexically, dates chronologically, then by series.
#[derive(Clone, Debug, PartialEq)]
pub struct Cube {
    /// The dataset's `@id`; empty for a blank node.
    pub id: String,
    pub title: LangMap,
    pub description: Option<LangMap>,
    pub axis: Axis,
    pub unit: Option<String>,
    /// Distinct series names, sorted lexically; empty when no observation has one.
    pub series: Vec<String>,
    pub observations: Vec<Obs>,
}

/// Why a document is not a valid chart dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CubeError {
    pub message: String,
    /// Where in the document: a node `@id` or a JSON pointer-like path. Empty when
    /// the error concerns the document as a whole.
    pub path: String,
}

impl CubeError {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for CubeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

impl std::error::Error for CubeError {}
