use yew::prelude::*;

use crate::atoms::{BadgeVariant, CodeBlock, OntoBadge};
use crate::molecules::{
    DatasetCard, OntoAnnotation, PropSpec, PropsTable, SectionHead, UsageExample,
};
use crate::molecules::dataset_initial;
use crate::ontology::{LiteralValue, TermKind};

fn literal(value: &str, language: Option<&str>, datatype: Option<&str>) -> LiteralValue {
    LiteralValue::new(
        value.to_string(),
        language.map(str::to_string),
        datatype.map(str::to_string),
    )
}

/// Sections 20-23: the ontology components this crate keeps — `OntoBadge`,
/// `OntoAnnotation`, the themeable token layer underneath them, and the dataset
/// card that let `eona-vocabulary-ui` drop PatternFly.
///
/// Kept here rather than moved with the stateful tier, which went back to
/// `eona-vocabulary-ui`: three of these four sections document components that
/// still ship from this crate, and `DemoPage` (`src/demo.rs:83`) composes this
/// one. Section 22 is the one that changed — it used to catalogue the eight
/// components of the `csr`-gated tier, and now says where they went.
#[function_component(OntologySection)]
pub fn ontology_section() -> Html {
    html! {
        <>
            <section class="block" id="ontology">
                <div class="wrap">
                    <SectionHead number="20" kicker="ontology" title="Term badges and annotations" />
                    <p class="section-intro">
                        { "Two components from the ontology tier that hold no state, so they render \
                           here like any other. " }<code>{ "OntoBadge" }</code>
                        { " tags an RDF/OWL entity kind, a language or a datatype; each variant takes \
                           a charter hue rather than a colour of its own." }
                    </p>
                    <UsageExample source={r##"<OntoBadge label="Class" variant={TermKind::Class.into()} />
<OntoBadge label="Object Property" variant={TermKind::ObjectProperty.into()} />
<OntoBadge label="en" variant={BadgeVariant::Lang} />
<OntoBadge label="Deprecated" variant={BadgeVariant::Deprecated} />"##}>
                        <div class="gallery-row">
                            <OntoBadge label="Ontology" variant={BadgeVariant::from(TermKind::Ontology)} />
                            <OntoBadge label="Class" variant={BadgeVariant::from(TermKind::Class)} />
                            <OntoBadge label="Object Property" variant={BadgeVariant::from(TermKind::ObjectProperty)} />
                            <OntoBadge label="Datatype Property" variant={BadgeVariant::from(TermKind::DatatypeProperty)} />
                            <OntoBadge label="Annotation Property" variant={BadgeVariant::from(TermKind::AnnotationProperty)} />
                            <OntoBadge label="Named Individual" variant={BadgeVariant::from(TermKind::NamedIndividual)} />
                            <OntoBadge label="en" variant={BadgeVariant::Lang} />
                            <OntoBadge label="dateTime" variant={BadgeVariant::Datatype} />
                            <OntoBadge label="Deprecated" variant={BadgeVariant::Deprecated} />
                        </div>
                    </UsageExample>
                    <PropsTable
                        component="OntoBadge"
                        props={vec![
                            PropSpec::required("label", "AttrValue", "Badge text."),
                            PropSpec::optional("variant", "BadgeVariant", "Default", "A TermKind, or Lang / Datatype / Deprecated / Default. Picks the colour pair."),
                            PropSpec::optional("title", "Option<AttrValue>", "None", "Native tooltip, e.g. the full datatype IRI behind a short label."),
                        ]}
                    />
                    <p class="section-intro">
                        { "An annotation row is one predicate and its literal values, each carrying \
                           its own language or datatype chip. A value with neither gets no chip." }
                    </p>
                    <UsageExample source={r##"<OntoAnnotation
    label="rdfs:comment"
    values={vec![
        LiteralValue { value: "A journey…".into(), language: Some("en".into()), datatype: None },
        LiteralValue { value: "Un trajet…".into(), language: Some("fr".into()), datatype: None },
    ]}
/>"##}>
                        <div>
                            <OntoAnnotation
                                label="rdfs:comment"
                                values={vec![
                                    literal("A journey undertaken by a passenger across one or more modes of transport.", Some("en"), None),
                                    literal("Un trajet effectué par un passager sur un ou plusieurs modes de transport.", Some("fr"), None),
                                ]}
                            />
                            <OntoAnnotation
                                label="dcterms:issued"
                                values={vec![
                                    literal("2026-03-14", None, Some("http://www.w3.org/2001/XMLSchema#date")),
                                ]}
                            />
                        </div>
                    </UsageExample>
                </div>
            </section>

            <section class="block block-alt" id="theming">
                <div class="wrap">
                    <SectionHead number="21" kicker="tokens" title="Theming" />
                    <p class="section-intro">
                        { "The charter's palette is fixed — tokens named after their ink. The crate no \
                           longer renders it: the colour and declination sections moved to " }
                        <code>{ "developer.eona-x.eu" }</code>
                        { ", and what stayed here is the token layer itself, in " }
                        <code>{ "TOKENS_CSS" }</code>
                        { ". On top of the palette sits a second layer named after the job, and that \
                           is the one a theme changes. Every value resolves to a palette token; dark \
                           mode never introduces a colour the charter does not already have." }
                    </p>
                    <div class="theme-demo">
                        <div class="theme-demo__pane">
                            <span class="theme-demo__caption">{ "light (default)" }</span>
                            <div class="theme-demo__card">
                                <b>{ "Surface, raised" }</b>
                                <p>{ "Body text on a raised surface, with " }<a href="#theming">{ "a link" }</a>{ "." }</p>
                                <span class="theme-demo__muted">{ "Muted secondary text" }</span>
                            </div>
                        </div>
                        <div class="theme-demo__pane" data-theme="dark">
                            <span class="theme-demo__caption">{ "data-theme=\"dark\"" }</span>
                            <div class="theme-demo__card">
                                <b>{ "Surface, raised" }</b>
                                <p>{ "Body text on a raised surface, with " }<a href="#theming">{ "a link" }</a>{ "." }</p>
                                <span class="theme-demo__muted">{ "Muted secondary text" }</span>
                            </div>
                        </div>
                    </div>
                    <CodeBlock
                        language="html"
                        code={r##"<!-- opt in on the root element; there is no automatic switch -->
<html data-theme="dark">"##}
                    />
                    <div class="integration-note">
                        <b>{ "Opt-in, deliberately." }</b>
                        <p>
                            { "Dark mode is not wired to " }<code>{ "prefers-color-scheme" }</code>
                            { ". A consumer that never sets the attribute keeps the appearance it has \
                               today — this very page would otherwise flip to dark for any visitor \
                               whose operating system is, which is not a decision a brand reference \
                               should make on its own." }
                        </p>
                    </div>
                    <p class="section-intro">
                        { "Dark values come from the charter's own " }<code>{ "websiteDarkNeutrals" }</code>
                        { " (slide 5): Navy Deep for the page, Slate for cards. " }
                        <code>{ "--eona-accent" }</code>
                        { " becomes Lavender rather than staying Action Blue, which is measured \
                           rather than taste — #006EF0 on #091B3B is 3.64:1, under the 4.5:1 floor \
                           for body text, where Lavender is 11.18:1." }
                    </p>
                    <p class="section-intro">
                        { "One limit worth knowing: " }<code>{ "COMPONENTS_CSS" }</code>
                        { " still reads the raw palette directly for most rules, so the gallery \
                           components in sections 01-08 do not follow " }<code>{ "data-theme" }</code>
                        { " yet. What does follow it is on this page to compare: the two panes above \
                           are built from the semantic layer, and so are the badge and annotation \
                           classes in " }<code>{ "ONTOLOGY_CSS" }</code>{ "." }
                    </p>
                </div>
            </section>

            <section class="block" id="interactive">
                <div class="wrap">
                    <SectionHead number="22" kicker="ontology" title="Where the browser went" />
                    <p class="section-intro">
                        { "Everything in this crate is presentational: no " }
                        <code>{ "use_state" }</code>{ ", no " }<code>{ "Callback" }</code>
                        { ", identical output under SSR and CSR. The ontology browser needed the \
                           opposite — clipboard writes, " }<code>{ "localStorage" }</code>
                        { ", expand/collapse — so it moved back to " }
                        <code>{ "eona-vocabulary-ui" }</code>
                        { ", the app that wrote it and its only consumer. The two components \
                           above stayed, because they hold no state and this page renders them." }
                    </p>
                    <div class="integration-note">
                        <b>{ "Which is why there is nothing to click here." }</b>
                        <p>
                            { "This page is rendered by " }<code>{ "ServerRenderer" }</code>
                            { " at build time. What a consumer still gets from this crate is the \
                               model the two components are typed on — " }
                            <code>{ "TermKind" }</code>{ " and " }<code>{ "LiteralValue" }</code>
                            { " — which an app that owns the rest of the model must re-export \
                               rather than redefine, or its terms will not fit these props." }
                        </p>
                    </div>
                    <CodeBlock
                        code={r##"use eona_ui_toolkit::ontology::{LiteralValue, TermKind};
use eona_ui_toolkit::{BadgeVariant, OntoAnnotation, OntoBadge};

html! {
    <>
        <OntoBadge label="Class" variant={BadgeVariant::Kind(TermKind::Class)} />
        <OntoAnnotation label="Definition" values={definitions} />
    </>
}"##}
                    />
                </div>
            </section>

            <section class="block block-alt" id="catalog">
                <div class="wrap">
                    <SectionHead number="23" kicker="ontology" title="Dataset cards" />
                    <p class="section-intro">
                        { "A catalog entry: thumbnail, title, version, publisher, description, and a \
                           call to action. It replaces " }<code>{ "edc-web-components" }</code>
                        { "' PatternFly card, which rendered a second design system's brand inside \
                           EONA-X pages and pulled PatternFly's entire stylesheet in to do it." }
                    </p>
                    <UsageExample source={r##"<DatasetCard
    title="EONA-X Mobility"
    href="ontologies?ontology=eona-mobility"
    version="1.2.0"
    publisher="EONA-X"
    badge="OWL"
    description="Core mobility vocabulary…"
/>"##}>
                        <div class="gallery-grid">
                            <DatasetCard
                                title="EONA-X Mobility"
                                initial={dataset_initial("EONA-X Mobility")}
                                href="#catalog"
                                version="1.2.0"
                                publisher="EONA-X"
                                badge="OWL"
                                description="Core mobility vocabulary: trips, legs, modes and the operators that run them."
                            />
                            <DatasetCard
                                title="NeTEx ↔ GTFS"
                                initial={dataset_initial("NeTEx ↔ GTFS")}
                                href="#catalog"
                                publisher="EONA-X"
                                badge="CROSSWALK"
                                action_label="View 3D crosswalk"
                                description="Alignment between the two dominant public-transport schedule formats."
                            />
                        </div>
                    </UsageExample>
                    <div class="integration-note">
                        <b>{ "A link, not a button." }</b>
                        <p>
                            { "The card it replaces took an " }<code>{ "on_offer_click" }</code>
                            { " callback that every caller resolved to a navigation. An " }
                            <code>{ "href" }</code>
                            { " does the same job, keeps the component presentational, and restores \
                               middle-click, open-in-new-tab and a status-bar preview." }
                        </p>
                    </div>
                    <PropsTable
                        component="DatasetCard"
                        props={vec![
                            PropSpec::required("title", "AttrValue", "Entry name."),
                            PropSpec::required("href", "AttrValue", "Where the whole card points."),
                            PropSpec::optional("description", "Option<AttrValue>", "None", "Short summary."),
                            PropSpec::optional("version", "Option<AttrValue>", "None", "Free text, not parsed — publishers write what they write."),
                            PropSpec::optional("publisher", "Option<AttrValue>", "None", "Shown under the title."),
                            PropSpec::optional("thumbnail", "Option<AttrValue>", "None", "Image URL; absent renders a lettered placeholder."),
                            PropSpec::optional("badge", "Option<AttrValue>", "None", "Category chip over the thumbnail, e.g. \"OWL\"."),
                            PropSpec::optional("badge_class", "Option<AttrValue>", "None", "Extra class for per-category colouring."),
                            PropSpec::optional("action_label", "AttrValue", "\"View documentation\"", "Call-to-action text."),
                        ]}
                    />
                </div>
            </section>
        </>
    }
}
