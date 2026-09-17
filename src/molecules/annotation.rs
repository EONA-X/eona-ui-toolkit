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

/// One rendered row: the literal text plus its optional language/datatype
/// chip. Precomputed like the Vue source's `rows` computed property —
/// language is checked before datatype, and a value with neither gets no tag.
fn row_tag(value: &LiteralValue) -> Option<(AttrValue, BadgeVariant)> {
    // Language wins over datatype, as the Vue source does. The datatype arm
    // reads the precomputed label rather than `datatype` itself: shortening the
    // IRI here made the markup a transform of the prop, and testing `datatype`
    // as well would leave a second axis that cannot change what renders.
    match &value.language {
        Some(lang) => Some((AttrValue::from(lang.clone()), BadgeVariant::Lang)),
        None => value
            .datatype_label
            .as_ref()
            .map(|d| (AttrValue::from(d.clone()), BadgeVariant::Datatype)),
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
