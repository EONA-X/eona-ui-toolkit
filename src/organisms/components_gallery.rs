use yew::prelude::*;

use crate::atoms::{CodeBlock, PillButton, PillButtonKind, TagLabel, TagLabelTone};
use crate::examples::{ReactSnippet, Snippet, SNIPPETS};
use crate::molecules::{
    AccordionItem, CaseCard, MemberCard, MemberFilterCard, Modal, NavDropdown, NewsCard, PropSpec,
    PropsTable, SectionHead, TextField, TextFieldKind, UsageExample,
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
                    <UsageExample source={r##"<PillButton label="Solid" kind={PillButtonKind::Solid} />
<PillButton label="Glass (on dark)" kind={PillButtonKind::GlassLight} />
<PillButton label="Glass (on light)" kind={PillButtonKind::GlassDark} />"##}>
                        <div class="gallery-row">
                            <PillButton label="Solid" kind={PillButtonKind::Solid} />
                            <div style="background:var(--eona-navy-deep);padding:18px 22px;border-radius:12px;">
                                <PillButton label="Glass (on dark)" kind={PillButtonKind::GlassLight} />
                            </div>
                            <div style="background:var(--eona-gray-bg);padding:18px 22px;border-radius:12px;">
                                <PillButton label="Glass (on light)" kind={PillButtonKind::GlassDark} />
                            </div>
                        </div>
                    </UsageExample>
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
                    <UsageExample source={r##"<TagLabel text="Actualité" />
<TagLabel text="Transport" tone={TagLabelTone::Blue} />"##}>
                        <div class="gallery-row">
                            <TagLabel text="Actualité" />
                            <TagLabel text="Transport" tone={TagLabelTone::Blue} />
                        </div>
                    </UsageExample>
                </div>
            </section>

            <section class="block" id="fields">
                <div class="wrap">
                    <SectionHead number="03" kicker="eona-x.eu" title="Form fields" />
                    <p class="section-intro">
                        { "Floating-label inputs, the shape used by the site's Gravity Forms contact form: \
                           a small label sits inside the field's own top padding rather than above it." }
                    </p>
                    <UsageExample source={r##"<TextField id="gallery-name" label="Name" placeholder="Ada Lovelace" />
<TextField
    id="gallery-message"
    label="Message"
    kind={TextFieldKind::Textarea}
    placeholder="Tell us about your project…"
/>"##}>
                        <div class="gallery-field-grid">
                            <TextField id="gallery-name" label="Name" placeholder="Ada Lovelace" />
                            <TextField id="gallery-message" label="Message" kind={TextFieldKind::Textarea} placeholder="Tell us about your project…" />
                        </div>
                    </UsageExample>
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
                    <UsageExample source={r##"<NewsCard
    href="#"
    tag="Actualité"
    title="Position Paper — EONA-X on Article 12 of the Data Governance Act"
/>
<CaseCard
    href="#"
    tag="Transport"
    title="Coordinating Paris 2024 without disrupting everyday travel"
/>"##}>
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
                    </UsageExample>
                    <h3 style="font-size:16px;margin-bottom:12px;">{ "Member tiles" }</h3>
                    <p class="section-intro">
                        { "The homepage scrolls this row continuously via requestAnimationFrame — its CSS \
                           (\".marquee\"/\".marquee-wrapper\") has no keyframes of its own, the motion is all \
                           JS. Shown here as a static row instead of re-deriving that animation loop." }
                    </p>
                    <UsageExample source={r##"<MemberCard name="AllTheWay" />
<MemberCard name="AnySolution" />
<MemberCard name="Partner Co." />"##}>
                        <div class="marquee-row">
                            <MemberCard name="AllTheWay" />
                            <MemberCard name="AnySolution" />
                            <MemberCard name="Partner Co." />
                            <MemberCard name="Partner Co." />
                        </div>
                    </UsageExample>
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
                    <UsageExample source={r##"<NavDropdown
    id="gallery-dropdown-eonax"
    label="EONA-X"
    links={vec![("#", "About"), ("#", "Our ecosystem"), ("#", "Dataspace")]}
/>
<NavDropdown id="gallery-dropdown-lang" label="EN" links={vec![("#", "Français")]} />"##}>
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
                    </UsageExample>
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
                    <UsageExample source={r##"<AccordionItem title="Mobility" open={true}>
    <p>{ "EONA-X facilitates the sharing of data between public and private operators…" }</p>
</AccordionItem>
<AccordionItem title="Logistics">
    <p>{ "Connecting freight, port, and carrier data…" }</p>
</AccordionItem>"##}>
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
                    </UsageExample>
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
                    <UsageExample source={r##"// The trigger is plain markup: the toolkit ships no event handlers,
// demo-page.js clears `inert` on the element named by data-open-modal.
<button type="button" data-open-modal="gallery-modal" class="pill-btn pill-btn-solid">
    { "Open modal" }
</button>
<Modal id="gallery-modal" title="About this component">
    <p>{ "…" }</p>
</Modal>"##}>
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
                    </UsageExample>
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
                    <UsageExample source={r##"<MemberFilterCard
    name="Carbookr"
    member_type="SME"
    country="France"
    sectors={vec!["Mobility"]}
/>"##}>
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
                    </UsageExample>
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

            <section class="block" id="snippets">
                <div class="wrap">
                    <SectionHead number="09" kicker="both stacks" title="Every component, in both stacks" />
                    <p class="section-intro">
                        { format!("All {} components the crate defines, one canonical call each: \
                                   the Yew ", SNIPPETS.len()) }
                        <code>{ "html!" }</code>
                        { format!(" on the left, the {} that also ship in ", react_available()) }
                        <code>{ "@eona-x/ui-toolkit-react" }</code>
                        { " on the right. Both panes are generated by " }
                        <code>{ "cargo xtask bridgegen" }</code>{ " from " }
                        <code>{ "abi/toolkit.abi.json" }</code>
                        { " — the same document that types the React package — so a prop added in " }
                        <code>{ "src/" }</code>
                        { " reaches both at once, and neither can be edited here without " }
                        <code>{ "cargo xtask check" }</code>{ " reporting it." }
                    </p>
                    <p class="section-intro">
                        { format!("The remaining {} carry a note instead of JSX, because a React \
                                   snippet for a component ", SNIPPETS.len() - react_available()) }
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

/// How many of [`SNIPPETS`] ship in the React package.
///
/// Counted rather than written down: the split moves whenever the verify gate
/// quarantines or releases a component, and a number typed into the prose
/// above would be the one thing on this page `cargo xtask check` cannot catch.
fn react_available() -> usize {
    SNIPPETS
        .iter()
        .filter(|s| matches!(s.react, ReactSnippet::Available { .. }))
        .count()
}

/// One component's row in section 09.
///
/// A plain `fn`, not a `#[function_component]`: the generator's parser counts
/// components by that attribute (`xtask/src/parse.rs`), and a helper that only
/// this file calls has no business appearing in `abi/toolkit.abi.json` or in
/// the in-scope tally the move is measured against.
fn snippet_entry(s: &'static Snippet) -> Html {
    // `yew_uses` is what the snippet will not compile without; joined into the
    // same block so one Copy gets both, the way the reader will paste them.
    let yew = format!("{}\n\n{}", s.yew_uses, s.yew);

    html! {
        <article class="snippet-entry" id={format!("snippet-{}", s.component)}>
            <h3 class="snippet-name">{ s.component }</h3>
            <p class="snippet-source"><code>{ s.source }</code></p>
            <div class="snippet-pair">
                <div class="snippet-pane">
                    <span class="snippet-pane-label">{ "Yew" }</span>
                    <CodeBlock code={yew} language="rust" />
                </div>
                { react_pane(&s.react) }
            </div>
        </article>
    }
}

/// The right-hand pane: the JSX, or why there is none.
///
/// The note is the generator's own words — `why` is the machine's rule, read
/// off `Status`, and `disposition` is the human's answer from
/// `abi/overrides.toml`. Neither is restated here, because a component missing
/// on purpose and one missing by accident look identical from the outside and
/// only these two strings tell them apart.
fn react_pane(react: &'static ReactSnippet) -> Html {
    match react {
        ReactSnippet::Available { import, jsx } => html! {
            <div class="snippet-pane">
                <span class="snippet-pane-label">{ "React" }</span>
                <CodeBlock code={format!("{import}\n\n{jsx}")} language="tsx" />
            </div>
        },
        ReactSnippet::Unavailable {
            why,
            disposition,
            tracked_by,
        } => html! {
            <div class="snippet-pane snippet-pane-absent">
                <span class="snippet-pane-label">{ "React — not shipped" }</span>
                <div class="snippet-absent">
                    <p>{ *why }</p>
                    if !disposition.is_empty() {
                        <p><b>{ "What to do: " }</b>{ *disposition }</p>
                    }
                    if !tracked_by.is_empty() {
                        <p class="snippet-absent-track">{ format!("Tracked by {tracked_by}.") }</p>
                    }
                </div>
            </div>
        },
    }
}
