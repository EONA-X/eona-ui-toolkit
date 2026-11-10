use yew::prelude::*;

use crate::atoms::{EvolutionBadge, PillButton, PillButtonKind, TagLabel, TagLabelTone};
use crate::chart::{donut_arc, trend_of};
use crate::examples::{Snippet, StackSnippet, SNIPPETS};
use crate::molecules::{
    AccordionItem, CaseCard, MemberCard, MemberFilterCard, CodeTabs, MetricTile, Modal, NavDropdown,
    NewsCard, PropSpec, PropsTable, ScoreDonut, SectionHead, TextField, TextFieldKind, UsageExample,
};

/// The components-gallery page body: buttons, tags, form fields, and cards
/// extracted from the live eona-x.eu website (not the brand-guide slides —
/// see `assets/components.css`'s "Components extracted from eona-x.eu" header
/// comment for the source and the one simplification made getting them here).
///
/// Every preview is paired with the `html!` that produced it via
/// [`UsageExample`], so the gallery doubles as copy-paste documentation. The
/// previews themselves are unchanged from what the page rendered before that
/// pairing existed — the snippet wraps them, it does not restate them.
///
/// Sections 01-08 are hand-composed: the `source` beside each preview is the
/// call that produced *that arrangement*, several components at several
/// settings, which no generator writes. Section 09 is the complement — one
/// canonical call per component, in both stacks, read straight out of
/// [`crate::examples::SNIPPETS`]. Nothing there is written here, so nothing
/// there can drift from `src/`.
#[function_component(ComponentsGallery)]
pub fn components_gallery() -> Html {
    html! {
        <main>
            <section class="block" id="buttons">
                <div class="wrap">
                    <SectionHead number="01" kicker="eona-x.eu" title="Buttons" />
                    <p class="section-intro">
                        { "Three treatments, all fully-rounded pills: a solid brand-blue call to action, and \
                           two \"glass\" outlines for placing a button over a dark or a light surface." }
                    </p>
                    { usage_example("usage-buttons", &["PillButton"], r##"<PillButton label="Solid" kind={PillButtonKind::Solid} />
<PillButton label="Glass (on dark)" kind={PillButtonKind::GlassLight} />
<PillButton label="Glass (on light)" kind={PillButtonKind::GlassDark} />"##, html! { <>
                        <div class="gallery-row">
                            <PillButton label="Solid" kind={PillButtonKind::Solid} />
                            <div style="background:var(--eona-navy-deep);padding:18px 22px;border-radius:12px;">
                                <PillButton label="Glass (on dark)" kind={PillButtonKind::GlassLight} />
                            </div>
                            <div style="background:var(--eona-gray-bg);padding:18px 22px;border-radius:12px;">
                                <PillButton label="Glass (on light)" kind={PillButtonKind::GlassDark} />
                            </div>
                        </div>
                    </> }) }
                    <PropsTable
                        component="PillButton"
                        props={vec![
                            PropSpec::required("label", "AttrValue", "Button text."),
                            PropSpec::optional("kind", "PillButtonKind", "Solid", "Solid, GlassLight (over dark), or GlassDark (over light)."),
                            PropSpec::optional("href", "Option<AttrValue>", "None", "Renders an <a> when set, a <button> otherwise."),
                            PropSpec::optional("disabled", "bool", "false", "Disabled <button>; ignored when href is set."),
                        ]}
                    />
                </div>
            </section>

            <section class="block" id="tags">
                <div class="wrap">
                    <SectionHead number="02" kicker="eona-x.eu" title="Tags" />
                    <p class="section-intro">
                        { "A small dot-and-label pill used to categorize news and case-study cards." }
                    </p>
                    { usage_example("usage-tags", &["TagLabel"], r##"<TagLabel text="Actualité" />
<TagLabel text="Transport" tone={TagLabelTone::Blue} />"##, html! { <>
                        <div class="gallery-row">
                            <TagLabel text="Actualité" />
                            <TagLabel text="Transport" tone={TagLabelTone::Blue} />
                        </div>
                    </> }) }
                </div>
            </section>

            <section class="block" id="fields">
                <div class="wrap">
                    <SectionHead number="03" kicker="eona-x.eu" title="Form fields" />
                    <p class="section-intro">
                        { "Floating-label inputs, the shape used by the site's Gravity Forms contact form: \
                           a small label sits inside the field's own top padding rather than above it." }
                    </p>
                    { usage_example("usage-fields", &["TextField"], r##"<TextField id="gallery-name" label="Name" placeholder="Ada Lovelace" />
<TextField
    id="gallery-message"
    label="Message"
    kind={TextFieldKind::Textarea}
    placeholder="Tell us about your project…"
/>"##, html! { <>
                        <div class="gallery-field-grid">
                            <TextField id="gallery-name" label="Name" placeholder="Ada Lovelace" />
                            <TextField id="gallery-message" label="Message" kind={TextFieldKind::Textarea} placeholder="Tell us about your project…" />
                        </div>
                    </> }) }
                    <PropsTable
                        component="TextField"
                        props={vec![
                            PropSpec::required("id", "AttrValue", "Ties the <label> to its control; must be unique on the page."),
                            PropSpec::required("label", "AttrValue", "Floating label text."),
                            PropSpec::optional("kind", "TextFieldKind", "Input", "Input or Textarea."),
                            PropSpec::optional("placeholder", "AttrValue", "\"\"", "Shown until the field has a value."),
                        ]}
                    />
                </div>
            </section>

            <section class="block" id="cards">
                <div class="wrap">
                    <SectionHead number="04" kicker="eona-x.eu" title="Cards" />
                    <p class="section-intro">
                        { "News cards, case-study cards (a darker variant for a more editorial feel), and \
                           small frosted-glass member/partner logo tiles." }
                    </p>
                    { usage_example("usage-cards", &["NewsCard", "CaseCard"], r##"<NewsCard
    href="#"
    tag="Actualité"
    title="Position Paper — EONA-X on Article 12 of the Data Governance Act"
/>
<CaseCard
    href="#"
    tag="Transport"
    title="Coordinating Paris 2024 without disrupting everyday travel"
/>"##, html! { <>
                        <div class="gallery-grid">
                            <NewsCard
                                href="#"
                                tag="Actualité"
                                title="Position Paper — EONA-X on Article 12 of the Data Governance Act"
                            />
                            <CaseCard
                                href="#"
                                tag="Transport"
                                title="Coordinating Paris 2024 without disrupting everyday travel"
                            />
                        </div>
                    </> }) }
                    <h3 style="font-size:16px;margin-bottom:12px;">{ "Member tiles" }</h3>
                    <p class="section-intro">
                        { "The homepage scrolls this row continuously via requestAnimationFrame — its CSS \
                           (\".marquee\"/\".marquee-wrapper\") has no keyframes of its own, the motion is all \
                           JS. Shown here as a static row instead of re-deriving that animation loop." }
                    </p>
                    { usage_example("usage-members", &["MemberCard"], r##"<MemberCard name="AllTheWay" />
<MemberCard name="AnySolution" />
<MemberCard name="Partner Co." />"##, html! { <>
                        <div class="marquee-row">
                            <MemberCard name="AllTheWay" />
                            <MemberCard name="AnySolution" />
                            <MemberCard name="Partner Co." />
                            <MemberCard name="Partner Co." />
                        </div>
                    </> }) }
                </div>
            </section>

            <section class="block" id="navigation">
                <div class="wrap">
                    <SectionHead number="05" kicker="eona-x.eu" title="Nav dropdowns" />
                    <p class="section-intro">
                        { "The site's \"EONA-X\" nav item and its language switcher both expand into a \
                           dropdown panel this same way. Click to open; the toggle is small page-glue JS \
                           flipping aria-expanded / the inert attribute — the same division of labour \
                           as the modal and the code-block copy buttons, all of it in " }
                        <code>{ "DEMO_JS" }</code>
                        { " rather than in a component." }
                    </p>
                    { usage_example("usage-dropdown", &["NavDropdown"], r##"<NavDropdown
    id="gallery-dropdown-eonax"
    label="EONA-X"
    links={vec![("#", "About"), ("#", "Our ecosystem"), ("#", "Dataspace")]}
/>
<NavDropdown id="gallery-dropdown-lang" label="EN" links={vec![("#", "Français")]} />"##, html! { <>
                        <div class="gallery-row">
                            <NavDropdown
                                id="gallery-dropdown-eonax"
                                label="EONA-X"
                                links={vec![("#", "About"), ("#", "Our ecosystem"), ("#", "Dataspace")]}
                            />
                            <NavDropdown
                                id="gallery-dropdown-lang"
                                label="EN"
                                links={vec![("#", "Français")]}
                            />
                        </div>
                    </> }) }
                </div>
            </section>

            <section class="block" id="accordion">
                <div class="wrap">
                    <SectionHead number="06" kicker="eona-x.eu" title="Accordion" />
                    <p class="section-intro">
                        { "Built on native " }<code>{ "<details>" }</code>{ "/" }<code>{ "<summary>" }</code>
                        { " — the only one of these four with no JS at all; the browser handles open/close \
                           on its own." }
                    </p>
                    { usage_example("usage-accordion", &["AccordionItem"], r##"<AccordionItem title="Mobility" open={true}>
    <p>{ "EONA-X facilitates the sharing of data between public and private operators…" }</p>
</AccordionItem>
<AccordionItem title="Logistics">
    <p>{ "Connecting freight, port, and carrier data…" }</p>
</AccordionItem>"##, html! { <>
                        <div>
                            <AccordionItem title="Mobility" open={true}>
                                <p>
                                    { "EONA-X facilitates the sharing of data between public and private operators \
                                       to streamline journeys, anticipate travel needs and offer more efficient \
                                       multimodal mobility services." }
                                </p>
                            </AccordionItem>
                            <AccordionItem title="Logistics">
                                <p>
                                    { "Connecting freight, port, and carrier data to reduce empty runs and make \
                                       multimodal logistics chains easier to plan and track." }
                                </p>
                            </AccordionItem>
                            <AccordionItem title="Tourism">
                                <p>
                                    { "Bringing together accommodation, transport, and destination data so trips \
                                       across operators feel like one connected journey." }
                                </p>
                            </AccordionItem>
                        </div>
                    </> }) }
                </div>
            </section>

            <section class="block" id="modal">
                <div class="wrap">
                    <SectionHead number="07" kicker="eona-x.eu" title="Modal" />
                    <p class="section-intro">
                        { "A fixed dark-backdrop overlay with a centered panel, toggled via the same inert \
                           attribute as the nav dropdowns above. No live example of this was found on the \
                           pages fetched while extracting these components — the classes and open/closed \
                           mechanics are real, sourced from eona-x.eu's own stylesheet; the content below is \
                           this crate's own minimal demo." }
                    </p>
                    { usage_example("usage-modal", &["Modal"], r##"// The trigger is plain markup: the toolkit ships no event handlers,
// demo-page.js clears `inert` on the element named by data-open-modal.
<button type="button" data-open-modal="gallery-modal" class="pill-btn pill-btn-solid">
    { "Open modal" }
</button>
<Modal id="gallery-modal" title="About this component">
    <p>{ "…" }</p>
</Modal>"##, html! { <>
                        <div>
                            <button type="button" data-open-modal="gallery-modal" class="pill-btn pill-btn-solid">
                                { "Open modal" }
                            </button>
                            <Modal id="gallery-modal" title="About this component">
                                <p>
                                    { "This dialog's shape (\".modal\"/\".modal-inner\", fixed dark backdrop, \
                                       centered white panel) is real, extracted eona-x.eu CSS — see this section's \
                                       intro for what wasn't found on the live site." }
                                </p>
                            </Modal>
                        </div>
                    </> }) }
                </div>
            </section>

            <section class="block" id="member-directory">
                <div class="wrap">
                    <SectionHead number="08" kicker="eona-x.eu" title="Member directory card" />
                    <p class="section-intro">
                        { "A richer member card than the homepage's logo tile above, from the members-and-\
                           users directory page: logo, name, member type, country, and sector tags. The \
                           directory's actual filter form (country/sector dropdowns, results refreshed \
                           server-side) isn't ported — there's no backend here to filter against — only \
                           the card shape." }
                    </p>
                    { usage_example("usage-filter", &["MemberFilterCard"], r##"<MemberFilterCard
    name="Carbookr"
    member_type="SME"
    country="France"
    sectors={vec!["Mobility"]}
/>"##, html! { <>
                        <div class="gallery-grid">
                            <MemberFilterCard
                                name="Carbookr"
                                member_type="SME"
                                country="France"
                                sectors={vec!["Mobility"]}
                            />
                            <MemberFilterCard
                                name="AnySolution"
                                member_type="Large group"
                                country="Spain"
                                sectors={vec!["Tourism", "Data collaboration"]}
                            />
                        </div>
                    </> }) }
                    <PropsTable
                        component="MemberFilterCard"
                        props={vec![
                            PropSpec::required("name", "AttrValue", "Member name; also the alt text of the logo tile."),
                            PropSpec::required("member_type", "AttrValue", "Free text, e.g. \"SME\" or \"Large group\"."),
                            PropSpec::required("country", "AttrValue", "Country label, rendered beside the type."),
                            PropSpec::required("sectors", "Vec<&'static str>", "One tag per sector; an empty vec renders no tag row."),
                        ]}
                    />
                </div>
            </section>

            <section class="block" id="charts">
                <div class="wrap">
                    <SectionHead number="09" kicker="data quality" title="Charts" />
                    <p class="section-intro">
                        { "Pure SVG and no script, so these render server-side and print. The \
                           geometry is computed by " }<code>{ "chart::donut_arc" }</code>
                        { " and handed over already-derived — the components themselves do no \
                           arithmetic, which is what keeps them in the React package instead of \
                           its quarantine list." }
                    </p>
                    { usage_example("usage-donut", &["ScoreDonut"], r##"let score = donut_arc(0.842);
<ScoreDonut dash_array={score.dash_array} band={score.band}
            label={score.label} caption="Completeness" />"##, html! { <>
                        <div class="gallery-row">
                            { for [(0.842, "Completeness"), (0.63, "Validity"), (0.31, "Freshness")]
                                .into_iter()
                                .map(|(score, caption)| {
                                    let arc = donut_arc(score);
                                    html! {
                                        <ScoreDonut dash_array={arc.dash_array} band={arc.band}
                                                    label={arc.label} caption={caption} />
                                    }
                                }) }
                        </div>
                    </> }) }
                    { usage_example("usage-metric", &["MetricTile", "EvolutionBadge"], r##"<MetricTile label="Records analysed" value="128 430"
            help="Across 12 datasets">
    <EvolutionBadge value="+3.2 pts" trend={trend_of(3.2)} />
</MetricTile>"##, html! { <>
                        <div class="gallery-row">
                            <MetricTile label="Records analysed" value="128 430"
                                        help="Across 12 datasets">
                                <EvolutionBadge value="+3.2 pts" trend={trend_of(3.2)} />
                            </MetricTile>
                            <MetricTile label="Rules failing" value="47">
                                <EvolutionBadge value="-11" trend={trend_of(-11.0)} />
                            </MetricTile>
                            <MetricTile label="Datasets" value="12"
                                        help="Unchanged since the last run">
                                <EvolutionBadge value="0" trend={trend_of(0.0)} />
                            </MetricTile>
                        </div>
                    </> }) }
                </div>
            </section>

            { live_charts() }

            <section class="block" id="snippets">
                <div class="wrap">
                    <SectionHead number="10" kicker="three stacks" title="Every component, in every stack" />
                    <p class="section-intro">
                        { format!("All {} components the crate defines, one canonical call each: \
                                   the Yew ", SNIPPETS.len()) }
                        <code>{ "html!" }</code>
                        { format!(", then the {} that also ship in ", react_available()) }
                        <code>{ "@eona-x/ui-toolkit-react" }</code>{ " and " }
                        <code>{ "@eona-x/ui-toolkit-svelte" }</code>
                        { ". Every pane is generated by " }
                        <code>{ "cargo xtask bridgegen" }</code>{ " from " }
                        <code>{ "abi/toolkit.abi.json" }</code>
                        { " — the same document that types both npm packages — so a prop added in " }
                        <code>{ "src/" }</code>
                        { " reaches all three at once, and none can be edited here without " }
                        <code>{ "cargo xtask check" }</code>{ " reporting it." }
                    </p>
                    <p class="section-intro">
                        { "The React and Svelte calls also run: " }
                        <a href={LIVE_REACT}>{ "every one rendered by React" }</a>{ " and " }
                        <a href={LIVE_SVELTE}>{ "every one rendered by Svelte" }</a>
                        { ", from the same generated list, styled by " }
                        <code>{ "@eona-x/ui-toolkit-css" }</code>{ "." }
                    </p>
                    <p class="section-intro">
                        { format!("The remaining {} carry a note instead of JSX for each stack that \
                                   does not ship them, because a snippet for a component ", SNIPPETS.len() - react_available()) }
                        <code>{ "index.ts" }</code>
                        { " does not export is an import that does not resolve. Section 18 argues \
                           the rule the notes quote; " }
                        <a href="#react-gaps">{ "jump to it" }</a>
                        { " for the five that could have been generated and were not." }
                    </p>
                    <nav class="snippet-jump" aria-label="Jump to a component">
                        { for SNIPPETS.iter().map(|s| html! {
                            <a href={format!("#snippet-{}", s.component)}>{ s.component }</a>
                        }) }
                    </nav>
                    <div class="snippet-index">
                        { for SNIPPETS.iter().map(snippet_entry) }
                    </div>
                </div>
            </section>
        </main>
    }
}

/// One live chart of the data-driven charts section.
#[cfg(feature = "charts")]
struct LiveChart {
    /// The component, as `CubeChart`'s `component` and `data-cube-chart` name it.
    name: &'static str,
    /// The sample it loads at first, relative to the page.
    sample: &'static str,
    /// Extra attribute the Svelte call and the container carry: `(name, value)`.
    option: Option<(&'static str, &'static str)>,
    /// What the chart shows.
    caption: &'static str,
    /// The Yew call, hand-arranged: the document text comes from the host.
    yew: &'static str,
    /// The space reserved before the chart arrives, so the page does not jump.
    min_height: &'static str,
}

#[cfg(feature = "charts")]
const LIVE_CHARTS: [LiveChart; 5] = [
    LiveChart {
        name: "BarChart",
        sample: "bar-airports.jsonld",
        option: None,
        caption: "One bar per category. The buttons load a second dataset into the same chart.",
        yew: r#"<BarChart jsonld={jsonld} />"#,
        min_height: "320px",
    },
    LiveChart {
        name: "StackedBarChart",
        sample: "stacked-modes.jsonld",
        option: None,
        caption: "Categories on the axis, one stack segment per series.",
        yew: r#"<StackedBarChart jsonld={jsonld} />"#,
        min_height: "320px",
    },
    LiveChart {
        name: "LineChart",
        sample: "line-traffic.jsonld",
        option: None,
        caption: "Dates on the axis, one line per series.",
        yew: r#"<LineChart jsonld={jsonld} />"#,
        min_height: "320px",
    },
    LiveChart {
        name: "CubeScoreDonut",
        sample: "donut-completeness.jsonld",
        option: Some(("select", "Completeness")),
        caption: "One observation as a fraction of 1; select names the category to draw.",
        yew: r#"<CubeScoreDonut jsonld={jsonld} select="Completeness" />"#,
        min_height: "200px",
    },
    LiveChart {
        name: "CubeMetricTile",
        sample: "metric-records.jsonld",
        option: Some(("help", "per month")),
        caption: "The latest value, and the change from the date before.",
        yew: r#"<CubeMetricTile jsonld={jsonld} help="per month" />"#,
        min_height: "140px",
    },
];

/// The second dataset of the bar chart, for the switcher: (file, button label).
#[cfg(feature = "charts")]
const BAR_DATASETS: [(&str, &str); 2] =
    [("bar-airports.jsonld", "Airports"), ("bar-stations.jsonld", "Stations")];

/// Section "09b": the data-driven charts, mounted live.
///
/// Each chart is an empty container; `assets/charts-page.js` loads the WebAssembly
/// module when it nears the viewport, fetches its sample and mounts the component.
/// Nothing is rendered on the server. Without the `charts` feature the section
/// does not exist, which is how the portal builds this page.
#[cfg(feature = "charts")]
fn live_charts() -> Html {
    html! {
        <section class="block" id="live-charts">
            <div class="wrap">
                <SectionHead number="09b" kicker="data" title="Data-driven charts" />
                <p class="section-intro">
                    { "Each chart takes the text of one JSON-LD dataset in the chart profile, " }
                    <code>{ "https://eona-x.eu/shape/chart-profile#" }</code>
                    { ", and parses it in Rust. They run as WebAssembly: this page loads the module \
                       when a chart nears the viewport, fetches the sample beside it and mounts the \
                       component, so nothing here is rendered on the server and the boxes stay empty \
                       without JavaScript. Svelte mounts them with " }
                    <code>{ "CubeChart" }</code>{ " from " }
                    <code>{ "@eona-x/ui-toolkit-svelte/wasm" }</code>
                    { "; there is no React version." }
                </p>
                { for LIVE_CHARTS.iter().map(live_chart) }
            </div>
        </section>
    }
}

#[cfg(not(feature = "charts"))]
fn live_charts() -> Html {
    html! {}
}

/// One live chart: its title, the preview container, and the three-stack code bar.
#[cfg(feature = "charts")]
fn live_chart(c: &'static LiveChart) -> Html {
    let src = format!("samples/{}", c.sample);
    let id = format!("live-{}", c.name);
    let option_attr = |name: &str| c.option.filter(|(n, _)| *n == name).map(|(_, v)| AttrValue::from(v));
    let tail = c.option.map(|(n, v)| format!(r#" {n}="{v}""#)).unwrap_or_default();
    // The generated call names a placeholder file; the page's own is the one worth copying.
    let svelte = stack_calls(&[c.name], Stack::Svelte).map(|code| {
        code.replace(r#"src="/data/cube.jsonld""#, &format!(r#"src="{src}"{tail}"#))
    });
    let (svelte, svelte_absent) = split(svelte);
    let (react, react_absent) = split(stack_calls(&[c.name], Stack::React));
    let switcher = (c.name == "BarChart").then(|| html! {
        <div class="chart-switch" role="group" aria-label="Dataset" hidden=true>
            { for BAR_DATASETS.iter().map(|(file, label)| html! {
                <button type="button" class="pill-btn pill-btn-glass-dark"
                    data-chart-for={id.clone()}
                    data-chart-src={format!("samples/{file}")}
                    aria-pressed={if *file == c.sample { "true" } else { "false" }}>
                    { *label }
                </button>
            }) }
        </div>
    });
    html! {
        <article class="snippet-entry" id={format!("live-chart-{}", c.name)}>
            <h3 class="snippet-name">{ c.name }</h3>
            <p class="section-intro">{ c.caption }</p>
            { for switcher }
            <UsageExample id={format!("tabs-live-{}", c.name)}
                source={c.yew}
                svelte={svelte} react={react}
                svelte_absent={svelte_absent} react_absent={react_absent}>
                <div id={id}
                    data-cube-chart={c.name}
                    data-src={src}
                    data-select={option_attr("select")}
                    data-help={option_attr("help")}
                    style={format!("min-height:{}", c.min_height)}></div>
            </UsageExample>
        </article>
    }
}

/// The live pages, absolute: this page is also published inside
/// `developer.eona-x.eu`, where a relative `react/` would not resolve.
const LIVE_REACT: &str = "https://eona-x.github.io/eona-ui-toolkit/react/";
const LIVE_SVELTE: &str = "https://eona-x.github.io/eona-ui-toolkit/svelte/";

/// How many of [`SNIPPETS`] ship in the React package.
///
/// Counted rather than written down: the split moves whenever the verify gate
/// quarantines or releases a component, and a number typed into the prose
/// above would be the one thing on this page `cargo xtask check` cannot catch.
fn react_available() -> usize {
    SNIPPETS
        .iter()
        .filter(|s| matches!(s.react, StackSnippet::Available { .. }))
        .count()
}

/// One component's row in section 10: its canonical call in the three stacks,
/// as one tabbed block.
///
/// A plain `fn`, not a `#[function_component]`: the generator's parser counts
/// components by that attribute (`xtask/src/parse.rs`), and a helper that only
/// this file calls has no business appearing in `abi/toolkit.abi.json` or in
/// the in-scope tally the move is measured against.
fn snippet_entry(s: &'static Snippet) -> Html {
    // `yew_uses` is what the snippet will not compile without; joined into the
    // same block so one Copy gets both, the way the reader will paste them.
    let yew = format!("{}\n\n{}", s.yew_uses, s.yew);
    let (svelte, svelte_absent) = split(stack_text(&s.svelte, Stack::Svelte));
    let (react, react_absent) = split(stack_text(&s.react, Stack::React));

    html! {
        <article class="snippet-entry" id={format!("snippet-{}", s.component)}>
            <h3 class="snippet-name">{ s.component }</h3>
            <p class="snippet-source"><code>{ s.source }</code></p>
            <CodeTabs
                id={format!("tabs-snippet-{}", s.component)}
                yew={yew}
                svelte={svelte}
                react={react}
                svelte_absent={svelte_absent}
                react_absent={react_absent}
            />
        </article>
    }
}

/// The two stacks that sit behind the YEW tab.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Stack {
    Svelte,
    React,
}

impl Stack {
    /// One call preceded by its import line(s), in the stack's own file shape.
    fn wrap(self, imports: &str, calls: &str) -> String {
        match self {
            Stack::Svelte => format!("<script>\n{imports}\n</script>\n\n{calls}"),
            Stack::React => format!("{imports}\n\n{calls}"),
        }
    }
}

/// `Ok(code)` or `Err(note)`, the two things a CodeTabs panel can hold.
fn split(text: Result<String, String>) -> (AttrValue, Option<AttrValue>) {
    match text {
        Ok(code) => (code.into(), None),
        Err(note) => (AttrValue::default(), Some(note.into())),
    }
}

/// One stack's tab for one component: the call, or the note for why there is none.
///
/// The note is the generator's own words: `why` is the machine's rule, read off
/// `Status`, and `disposition` is the human's answer from `abi/overrides.toml`.
/// Neither is restated here, because a component missing on purpose and one
/// missing by accident look identical from the outside and only these two
/// strings tell them apart.
fn stack_text(stack: &'static StackSnippet, which: Stack) -> Result<String, String> {
    match stack {
        StackSnippet::Available { import, code } => Ok(which.wrap(import, code)),
        StackSnippet::Unavailable { why, disposition, tracked_by } => Err(absent_note(why, disposition, tracked_by)),
    }
}

fn absent_note(why: &str, disposition: &str, tracked_by: &str) -> String {
    let mut note = why.to_string();
    if !disposition.is_empty() {
        note.push_str(&format!(" What to do: {disposition}"));
    }
    if !tracked_by.is_empty() {
        note.push_str(&format!(" Tracked by {tracked_by}."));
    }
    note
}

/// The generated canonical calls of `uses`, one after the other, under one
/// import line per package. A component with no version in `stack` gets a
/// comment line with its reason after the calls of the others; `Err` is the
/// reasons alone when none of them has a version.
///
/// `uses` is written next to the example it belongs to, so a reviewer reads the
/// two together. A name that is not in [`SNIPPETS`] panics: a typo would
/// otherwise leave an empty tab on the page.
fn stack_calls(uses: &[&str], stack: Stack) -> Result<String, String> {
    let entries: Vec<&'static Snippet> = uses
        .iter()
        .map(|name| {
            SNIPPETS
                .iter()
                .find(|s| s.component == *name)
                .unwrap_or_else(|| panic!("`{name}` is listed as used by an example but is not a component"))
        })
        .collect();

    let missing: Vec<(&str, String)> = entries
        .iter()
        .filter_map(|s| {
            let which = if stack == Stack::Svelte { &s.svelte } else { &s.react };
            match which {
                StackSnippet::Unavailable { why, disposition, tracked_by } => {
                    Some((s.component, absent_note(why, disposition, tracked_by)))
                }
                StackSnippet::Available { .. } => None,
            }
        })
        .collect();
    // Nothing to show but the reasons: the tab becomes the note panel.
    if missing.len() == entries.len() {
        let notes: Vec<String> = missing.iter().map(|(c, n)| format!("{c}: {n}")).collect();
        return Err(notes.join(" "));
    }

    // `import { A } from 'pkg';` lines, merged per package in first-seen order.
    let mut packages: Vec<(&str, Vec<&str>)> = Vec::new();
    let mut calls: Vec<&str> = Vec::new();
    for s in &entries {
        let which = if stack == Stack::Svelte { &s.svelte } else { &s.react };
        let StackSnippet::Available { import, code } = which else { continue };
        calls.push(code);
        let Some((names, pkg)) = import
            .strip_prefix("import { ")
            .and_then(|r| r.split_once(" } from '"))
            .and_then(|(names, r)| Some((names, r.strip_suffix("';")?)))
        else {
            panic!("unexpected import line shape: {import}")
        };
        let at = match packages.iter().position(|(p, _)| *p == pkg) {
            Some(at) => at,
            None => {
                packages.push((pkg, Vec::new()));
                packages.len() - 1
            }
        };
        for name in names.split(", ") {
            if !packages[at].1.contains(&name) {
                packages[at].1.push(name);
            }
        }
    }
    let imports = packages
        .iter()
        .map(|(pkg, names)| format!("import {{ {} }} from '{pkg}';", names.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    // A component without a version is named after the calls that exist, in the
    // stack's own comment syntax, one line each, so Copy still gets valid code.
    let mut body = calls.join("\n\n");
    if !missing.is_empty() {
        body.push_str("\n\n");
        let notes: Vec<String> = missing
            .iter()
            .map(|(c, n)| match stack {
                Stack::Svelte => format!("<!-- {c}: {} -->", n.replace("-->", "--&gt;")),
                Stack::React => format!("// {c}: {n}"),
            })
            .collect();
        body.push_str(&notes.join("\n"));
    }
    Ok(stack.wrap(&indent(&imports, stack), &body))
}

/// Svelte's imports sit inside `<script>`, indented like the generated ones.
fn indent(imports: &str, stack: Stack) -> String {
    match stack {
        Stack::Svelte => imports.lines().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n"),
        Stack::React => imports.to_string(),
    }
}

/// A [`UsageExample`] for the hand-arranged `source` (the YEW tab), with the
/// generated calls of `uses` behind SVELTE and REACT.
///
/// `uses` names every component the example renders, in the order the source
/// first uses them. It sits next to `source` so the two are reviewed together.
pub(crate) fn usage_example(id: &'static str, uses: &[&str], source: &'static str, children: Html) -> Html {
    let (svelte, svelte_absent) = split(stack_calls(uses, Stack::Svelte));
    let (react, react_absent) = split(stack_calls(uses, Stack::React));
    html! {
        <UsageExample id={id} source={source} svelte={svelte} react={react}
                      svelte_absent={svelte_absent} react_absent={react_absent}>
            { children }
        </UsageExample>
    }
}

/// A bare [`CodeTabs`] for `source` (the YEW tab), with the generated calls of
/// `uses` behind SVELTE and REACT. For code bars that have no preview of their own.
pub(crate) fn code_tabs(id: &'static str, uses: &[&str], source: &'static str) -> Html {
    let (svelte, svelte_absent) = split(stack_calls(uses, Stack::Svelte));
    let (react, react_absent) = split(stack_calls(uses, Stack::React));
    html! {
        <CodeTabs id={id} yew={source} svelte={svelte} react={react}
                  svelte_absent={svelte_absent} react_absent={react_absent} />
    }
}

/// A [`CodeTabs`] whose three tabs are the same text: markup that is not
/// framework code (an HTML attribute to set), so every stack writes it alike.
pub(crate) fn code_tabs_same(id: &'static str, source: &'static str) -> Html {
    html! { <CodeTabs id={id} yew={source} svelte={source} react={source} /> }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_of_several_components_share_one_import_line() {
        let svelte = stack_calls(&["PillButton", "TagLabel"], Stack::Svelte).unwrap();
        assert_eq!(svelte.matches("import {").count(), 1, "{svelte}");
        assert!(svelte.starts_with("<script>\n  import { PillButton, TagLabel } from '@eona-x/ui-toolkit-svelte';\n</script>\n\n"), "{svelte}");
        assert!(svelte.contains("<PillButton") && svelte.contains("<TagLabel"));
        let react = stack_calls(&["PillButton", "TagLabel"], Stack::React).unwrap();
        assert!(react.starts_with("import { PillButton, TagLabel } from '@eona-x/ui-toolkit-react';\n\n"), "{react}");
    }

    #[test]
    fn a_component_without_the_stack_turns_the_tab_into_its_reason() {
        let note = stack_calls(&["BarChart"], Stack::React).unwrap_err();
        assert!(note.contains("not planned"), "{note}");
        assert!(note.contains("eona-x/backlog#1219"), "{note}");
    }

    #[test]
    fn a_partial_stack_keeps_the_calls_it_has_and_notes_each_component_it_lacks() {
        let react = stack_calls(&["PillButton", "BarChart", "TagLabel"], Stack::React).unwrap();
        assert!(react.contains("<PillButton") && react.contains("<TagLabel"), "{react}");
        assert!(react.contains("import { PillButton, TagLabel } from '@eona-x/ui-toolkit-react';"), "{react}");
        assert!(!react.contains("<BarChart"), "{react}");
        let note: Vec<&str> = react.lines().filter(|l| l.starts_with("// BarChart: ")).collect();
        assert_eq!(note.len(), 1, "one note line for the missing component: {react}");
        assert!(note[0].contains("not planned"), "{react}");
    }

    #[test]
    #[should_panic(expected = "NoSuchComponent")]
    fn a_misspelt_component_name_is_a_build_error_not_an_empty_tab() {
        let _ = stack_calls(&["NoSuchComponent"], Stack::Svelte);
    }
}
