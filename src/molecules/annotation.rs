//! MOLECULE · OntoAnnotation
//!
//! A labelled row of literal values (definition, note, metadata, …). Each
//! value keeps its language / datatype tag rendered with the OntoBadge atom
//! and preserves multi-line whitespace. Composes: OntoBadge.
//!
//! Ports `containers/prez-ui/theme/app/components/ontology/molecules/OntoAnnotation.vue`.

use yew::prelude::*;

use crate::atoms::{BadgeVariant, OntoBadge};
use crate::ontology::LiteralValue;

#[derive(Properties, PartialEq, Clone)]
pub struct OntoAnnotationProps {
    pub label: AttrValue,
    pub values: Vec<LiteralValue>,
}

/// Mirrors the Vue source's `shortDatatype`: the datatype IRI's local name
/// after its last `#` or `/`, falling back to the whole IRI if neither is
/// present.
fn short_datatype(datatype: &str) -> &str {
    match datatype.rfind('#').or_else(|| datatype.rfind('/')) {
        Some(i) => &datatype[i + 1..],
        None => datatype,
    }
}

/// One rendered row: the literal text plus its optional language/datatype
/// chip. Precomputed like the Vue source's `rows` computed property —
/// language is checked before datatype, and a value with neither gets no tag.
fn row_tag(value: &LiteralValue) -> Option<(AttrValue, BadgeVariant)> {
    match (&value.language, &value.datatype) {
        (Some(lang), _) => Some((AttrValue::from(lang.clone()), BadgeVariant::Lang)),
        (None, Some(datatype)) => {
            Some((AttrValue::from(short_datatype(datatype).to_string()), BadgeVariant::Datatype))
        }
        (None, None) => None,
    }
}

#[function_component(OntoAnnotation)]
pub fn onto_annotation(props: &OntoAnnotationProps) -> Html {
    let rows: Vec<(String, Option<(AttrValue, BadgeVariant)>)> =
        props.values.iter().map(|v| (v.value.clone(), row_tag(v))).collect();

    html! {
        <div class="annotation">
            <dt class="annotation__label">{ props.label.clone() }</dt>
            <dd class="annotation__values">
                { for rows.into_iter().map(|(value, tag)| html! {
                    <div class="annotation__row">
                        <span class="annotation__text">{ value }</span>
                        // The Vue source passes `class="ml-1.5 align-middle"` as a
                        // fallthrough attr straight onto OntoBadge's root element;
                        // this OntoBadge port has no class-passthrough prop, so the
                        // same spacing/alignment is applied via the
                        // `.annotation__row .badge` descendant rule in main.css.
                        if let Some((text, variant)) = tag {
                            <OntoBadge label={text} variant={variant} />
                        }
                    </div>
                }) }
            </dd>
        </div>
    }
}
