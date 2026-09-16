//! ATOM · OntoBadge
//!
//! A small outline pill used to tag an entity kind (Class, Object Property,
//! …), a language, a datatype, or a deprecated flag. Colour comes from the
//! theme's chart tokens (`styles/main.css`) so it stays consistent in light
//! and dark. Purely presentational — no dependencies on other components.
//!
//! Ports `containers/prez-ui/theme/app/components/ontology/atoms/OntoBadge.vue`.
//! That component exposes a default slot falling back to `label`, but grep
//! over prez-ui confirms no caller ever passes different slot content, so
//! this port renders `label` directly and has no children prop.

use yew::prelude::*;

use crate::ontology::TermKind;

/// Mirrors the Vue source's `Variant = TermKind | "lang" | "datatype" |
/// "deprecated" | "default"` union.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    Kind(TermKind),
    Lang,
    Datatype,
    Deprecated,
    #[default]
    Default,
}

impl From<TermKind> for BadgeVariant {
    fn from(kind: TermKind) -> Self {
        BadgeVariant::Kind(kind)
    }
}

impl BadgeVariant {
    /// The CSS class carrying this variant's colour pair — mirrors the
    /// `VARIANT_CLASS` lookup table in the Vue source.
    fn css_class(self) -> &'static str {
        match self {
            BadgeVariant::Kind(TermKind::Class) => "badge--class",
            BadgeVariant::Kind(TermKind::ObjectProperty) => "badge--object-property",
            BadgeVariant::Kind(TermKind::DatatypeProperty) => "badge--datatype-property",
            BadgeVariant::Kind(TermKind::AnnotationProperty) => "badge--annotation-property",
            BadgeVariant::Kind(TermKind::Property) => "badge--property",
            BadgeVariant::Kind(TermKind::NamedIndividual) => "badge--named-individual",
            BadgeVariant::Kind(TermKind::Ontology) => "badge--ontology",
            BadgeVariant::Kind(TermKind::Other)
            | BadgeVariant::Lang
            | BadgeVariant::Datatype
            | BadgeVariant::Default => "badge--default",
            BadgeVariant::Deprecated => "badge--deprecated",
        }
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct OntoBadgeProps {
    pub label: AttrValue,
    #[prop_or_default]
    pub variant: BadgeVariant,
    #[prop_or_default]
    pub title: Option<AttrValue>,
}

#[function_component(OntoBadge)]
pub fn onto_badge(props: &OntoBadgeProps) -> Html {
    let class = classes!("badge", props.variant.css_class());
    html! {
        <span class={class} title={props.title.clone()}>
            { props.label.clone() }
        </span>
    }
}
