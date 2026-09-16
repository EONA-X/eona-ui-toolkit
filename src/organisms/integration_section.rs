use yew::prelude::*;

use crate::atoms::CodeBlock;
use crate::molecules::SectionHead;

/// "Using the toolkit": everything between an empty manifest and a rendered
/// page, for both stacks the components ship to. Continues the gallery's
/// section numbering (it ends at 09) and runs to 19; the ontology sections
/// under [`crate::OntologySection`] pick up at 20.
///
/// The guide is in two halves and they are not interchangeable instructions.
/// Sections 11-14 are the Yew crate; 15-19 are the generated React package.
/// Each half's sections carry their stack as the [`SectionHead`] kicker, so a
/// reader who lands mid-page from an anchor can tell in one line whose
/// instructions they are reading. Section 10 is the only stack-neutral one: it
/// says what the crate contains, which is the question both halves start from.
///
/// This is the half of the demo page that documents the *crate* rather than the
/// components — the reason the page lives here and not in the portal that
/// renders it. Every snippet is the real thing: the Cargo stanza is what
/// `developer.eona-x.eu`'s `crates/site` actually declares, and the submodule
/// command is how that repository actually vendors this one. The one exception
/// is section 15, whose registry does not exist yet; it is marked as such on
/// the page rather than being quietly written in the present tense.
///
/// Section 14 deliberately gives two answers rather than one. The submodule is
/// right for a host that publishes [`crate::demo::DemoPage`], and it is right
/// for a reason specific to that host — its Dockerfile's `COPY . .` cannot
/// carry credentials. An app that only wants the components has neither that
/// constraint nor a use for the demo page, and a plain git dependency serves it
/// better.
#[function_component(IntegrationSection)]
pub fn integration_section() -> Html {
    html! {
        <>
            <section class="block block-alt" id="scope">
                <div class="wrap">
                    <SectionHead number="10" kicker="integration" title="What the crate ships" />
                    <p class="section-intro">
                        { "Presentational components and the three stylesheets that make them look \
                           like EONA-X. Nothing else: no state, no data fetching, no page. They \
                           are consumed from two stacks — as a Rust crate, and as a React package \
                           generated from that same crate — so this guide is in two halves. " }
                        <strong>{ "Sections 11-14 are Yew; sections 15-19 are React." }</strong>
                        { " Neither half's instructions apply to the other stack, and each \
                           section's kicker says which one it belongs to. For the call itself — \
                           one canonical snippet per component, side by side in both stacks — see " }
                        <a href="#snippets">{ "section 09" }</a>
                        { ", which is generated rather than written and so cannot fall behind " }
                        <code>{ "src/" }</code>{ "." }
                    </p>

                    <h3 class="integration-case">{ "The portal's page sections went home" }</h3>
                    <p class="section-intro">
                        { "The charter slides and the dataspace-architecture diagrams — " }
                        <code>{ "ColorSection" }</code>{ ", " }<code>{ "DataspaceLayers" }</code>
                        { ", " }<code>{ "IdsRamLayers" }</code>
                        { " and the rest — are in " }<code>{ "developer.eona-x.eu" }</code>
                        { " under " }<code>{ "crates/site/src/sections/" }</code>
                        { ". They took no props and had exactly one caller, the portal itself." }
                    </p>

                    <h3 class="integration-case">{ "BeeNest went with them" }</h3>
                    <p class="section-intro">
                        { "The bee-nest diagrams and the Turtle-to-Rust codegen behind them are \
                           the same repository's " }<code>{ "crates/site/build.rs" }</code>
                        { " and " }<code>{ "crates/site/data/*.ttl" }</code>
                        { ". Their content is EONA-X architecture rather than EONA-X brand, and it \
                           changes on the architecture's schedule, not the design system's." }
                    </p>

                    <h3 class="integration-case">{ "The ontology browser went back to its app" }</h3>
                    <p class="section-intro">
                        { "The stateful term browser, the ontology model and the JSON-LD parser \
                           are in " }<code>{ "eona-vocabulary-ui" }</code>
                        { ", which wrote them and was their only consumer. The two presentational \
                           pieces stayed and are documented in sections 20-23: " }
                        <code>{ "OntoBadge" }</code>{ " and " }<code>{ "OntoAnnotation" }</code>
                        { ", with the two model types they are typed on." }
                    </p>

                    <div class="integration-note">
                        <b>{ "One consumer is not reuse." }</b>
                        <p>
                            { "Each of those three had precisely one caller, and every consumer of \
                               the toolkit was paying for all of them in compile time, dependency \
                               surface and API to read past. The crate's whole dependency list is \
                               now " }<code>{ "yew" }</code>
                            { " — the Turtle parsers left with BeeNest, " }<code>{ "serde" }</code>
                            { " left with the JSON-LD parser, and the five browser crates left \
                               with the stateful tier." }
                        </p>
                    </div>
                </div>
            </section>

            <section class="block" id="install">
                <div class="wrap">
                    <SectionHead number="11" kicker="yew" title="Add the crate" />
                    <p class="section-intro">
                        { "The components are renderer-agnostic, so neither renderer feature is on by \
                           default — a consumer picks one. " }<code>{ "ssr" }</code>
                        { " renders to HTML strings at build time (what a static-site generator wants); " }
                        <code>{ "csr" }</code>
                        { " mounts them in a browser, which is what a WASM host needs. Enabling neither \
                           compiles the components but leaves you no way to render them." }
                    </p>
                    <CodeBlock
                        language="toml"
                        code={r##"# static site generation
eona-ui-toolkit = { path = "../eona-ui-toolkit", features = ["ssr"] }

# browser / WASM host
eona-ui-toolkit = { path = "../eona-ui-toolkit", features = ["csr"] }"##}
                    />
                    <p class="section-intro">
                        { "The choice is now smaller than it looks: " }<code>{ "csr" }</code>
                        { " used to gate a whole module of stateful components as well, and since \
                           that module moved to " }<code>{ "eona-vocabulary-ui" }</code>
                        { " the feature selects Yew's renderer and nothing else. It is kept under \
                           the same name so existing manifests keep resolving, and because which \
                           renderer a consumer wants is still a question only the consumer can \
                           answer." }
                    </p>
                    <p class="section-intro">
                        { "The toolkit targets Yew " }<strong>{ "0.23" }</strong>
                        { ", the same release " }<code>{ "edc-web-components" }</code>
                        { " targets. That alignment is not cosmetic: components built against different \
                           Yew versions cannot share " }<code>{ "Html" }</code>
                        { ", props or contexts, so a version mismatch makes composing the two libraries \
                           in one binary impossible rather than merely awkward." }
                    </p>
                </div>
            </section>

            <section class="block block-alt" id="stylesheets">
                <div class="wrap">
                    <SectionHead number="12" kicker="yew" title="Ship the stylesheets" />
                    <p class="section-intro">
                        { "Markup and class names are ported verbatim from the brand charter, so the \
                           stylesheets are the rendering contract, not a theme: without them the \
                           components render as unstyled HTML. They ship inside the crate as string \
                           constants so consumers never reach into its on-disk layout." }
                    </p>
                    <CodeBlock
                        code={r##"use eona_ui_toolkit::{COMPONENTS_CSS, ONTOLOGY_CSS, TOKENS_CSS};

std::fs::write(dist.join("tokens.css"), TOKENS_CSS)?;
std::fs::write(dist.join("components.css"), COMPONENTS_CSS)?;
std::fs::write(dist.join("ontology.css"), ONTOLOGY_CSS)?;   // only if you render OntoBadge / OntoAnnotation"##}
                    />
                    <p class="section-intro">
                        { "Link all three. The order below is the house convention and not a \
                           rendering requirement: " }<code>{ "tokens.css" }</code>
                        { " is the only one of the three that declares a custom property — " }
                        <code>{ "components.css" }</code>
                        { " has no " }<code>{ ":root" }</code>
                        { " block at all — and a custom property is resolved per element at \
                           computed-value time rather than in stylesheet order, so swapping the \
                           links changes nothing a browser renders. What does break the page is \
                           leaving " }<code>{ "tokens.css" }</code>
                        { " out, because then every " }<code>{ "var()" }</code>
                        { " in the other two resolves to nothing." }
                    </p>
                    <CodeBlock
                        language="html"
                        copyable=false
                        code={r##"<link rel="stylesheet" href="tokens.css">
<link rel="stylesheet" href="components.css">
<link rel="stylesheet" href="ontology.css">"##}
                    />
                    <div class="integration-note">
                        <b>{ "Why three files and not two." }</b>
                        <p>
                            { "The badge and annotation rules are a separate constant because " }
                            <code>{ "ontology.css" }</code>
                            { " is a name three repositories already write out by hand. The rest of \
                               that stylesheet — the browser's own classes — left with the stateful \
                               tier, so what remains is small; a host that renders neither of those \
                               two components can skip it." }
                        </p>
                    </div>
                </div>
            </section>

            <section class="block" id="render">
                <div class="wrap">
                    <SectionHead number="13" kicker="yew" title="Render a page" />
                    <p class="section-intro">
                        { "This very page is a component. " }<code>{ "DemoPage" }</code>
                        { " is the whole document body — header, hero, gallery, and this section — and " }
                        <code>{ "DEMO_SHELL" }</code>
                        { " is the surrounding HTML document with an " }<code>{ "__APP__" }</code>
                        { " placeholder for it. A host renders one into the other:" }
                    </p>
                    <CodeBlock
                        code={r##"use eona_ui_toolkit::demo::{DemoPage, DEMO_JS, DEMO_SHELL};
use yew::ServerRenderer;

let body = ServerRenderer::<DemoPage>::new().render().await;
std::fs::write(dist.join("components.html"), DEMO_SHELL.replace("__APP__", &body))?;
std::fs::write(dist.join("demo-page.js"), DEMO_JS)?;"##}
                    />
                    <div class="integration-note">
                        <b>{ "Six assets the shell expects, and one is not in the crate." }</b>
                        <p>
                            { "Five are crate constants, written out above and in section 12: " }
                            <code>{ "tokens.css" }</code>{ ", " }<code>{ "components.css" }</code>
                            { ", " }<code>{ "ontology.css" }</code>{ ", " }
                            <code>{ "demo-page.js" }</code>{ " and " }<code>{ "LOGO_SVG" }</code>
                            { ", the EONA-X mark — write that one to " }
                            <code>{ "logo/logo-transparent.svg" }</code>
                            { ", the path " }<code>{ "SiteHeader" }</code>
                            { " hardcodes twice (" }
                            <code>{ "src/organisms/site_header.rs:101,105" }</code>
                            { ") and the shell links as the favicon (" }
                            <code>{ "assets/demo-shell.html:14" }</code>{ "). It ships here because \
                               a component that names an asset should carry it; hosts used to have \
                               to copy the file in and got a header with a hole in it when they \
                               forgot." }
                        </p>
                        <p>
                            { "The one exception is " }<code>{ "site-header.js" }</code>
                            { ", the scroll-collapse glue for " }<code>{ "SiteHeader" }</code>
                            { ". It stays with the portal, whose every page uses it; omit it and \
                               the header renders and simply never collapses — degraded, not \
                               broken. It is worth knowing on the React side too: " }
                            <code>{ "SiteHeader" }</code>
                            { " is one of the 33 exported components and the npm package carries no \
                               assets at all." }
                        </p>
                    </div>
                    <p class="section-intro">
                        { "Every component in the toolkit is presentational: no " }
                        <code>{ "use_state" }</code>{ ", no " }<code>{ "Callback" }</code>
                        { ", no event handlers. They render identically under SSR and CSR, and the \
                           interaction you can click on this page — the dropdowns, the modal, the copy \
                           buttons — is page glue in " }<code>{ "DEMO_JS" }</code>
                        { " operating on the rendered markup. That is also what makes the React \
                           package of sections 15-19 possible at all." }
                    </p>
                </div>
            </section>

            <section class="block block-alt" id="vendoring">
                <div class="wrap">
                    <SectionHead number="14" kicker="yew" title="Vendoring: pick by what you need" />
                    <p class="section-intro">
                        { "There are two ways in, and the right one depends on whether you want the \
                           components or the whole page." }
                    </p>

                    <h3 class="integration-case">{ "A site that publishes this demo page" }</h3>
                    <p class="section-intro">
                        { "Use a git submodule plus a path dependency. That is what " }
                        <code>{ "developer.eona-x.eu" }</code>
                        { " does, and it is a deployment constraint rather than a preference: its \
                           Dockerfile builds from " }<code>{ "COPY . ." }</code>
                        { ", so a submodule arrives in the image as ordinary files, while a git \
                           dependency on a private GitLab remote would need credentials inside the \
                           build. It also keeps the crate's working tree in the host's checkout, so \
                           the page and the components it documents move together." }
                    </p>
                    <CodeBlock
                        language="sh"
                        code={r##"git submodule add \
  ssh://git@gitlab.eona-x.org:29418/eona-x/web/eona-ui-toolkit.git \
  crates/eona-ui-toolkit

# EXCLUDE it from the host workspace, then depend on it by path. In the host's
# own Cargo.toml:
#   [workspace]    exclude = ["crates/eona-ui-toolkit"]
# and in the crate that renders (crates/site/Cargo.toml, so the path is relative
# to THAT manifest, not to the workspace root):
#   [dependencies] eona-ui-toolkit = { path = "../eona-ui-toolkit", features = ["ssr"] }

git clone --recurse-submodules <host-repo>   # for a fresh checkout
git submodule update --init                  # for an existing one"##}
                    />
                    <p class="section-intro">
                        { "The " }<code>{ "exclude" }</code>
                        { " is not optional. This crate became its own workspace root when it \
                           gained " }<code>{ "xtask" }</code>
                        { ", and a path dependency living inside the host's workspace directory is \
                           pulled in as a member by default, which is a hard " }
                        <code>{ "error: multiple workspace roots found in the same workspace" }</code>
                        { " before anything compiles. The error names two directories and no file \
                           you edited, so it is worth recognising. " }
                        <code>{ "developer.eona-x.eu" }</code>
                        { " carries exactly that line (" }<code>{ "Cargo.toml:10" }</code>
                        { "), and nothing is lost by it: the toolkit runs its own tests from its \
                           own repository." }
                    </p>
                    <p class="section-intro">
                        { "A checkout that skips the submodule step gets an empty " }
                        <code>{ "crates/eona-ui-toolkit/" }</code>
                        { " and a Cargo error about a missing manifest — the usual first symptom." }
                    </p>

                    <h3 class="integration-case">{ "An app that just needs the components" }</h3>
                    <p class="section-intro">
                        { "Use an ordinary git dependency. Nothing about a submodule helps here: an \
                           app that renders its own screens never touches " }<code>{ "DemoPage" }</code>
                        { ", " }<code>{ "DEMO_SHELL" }</code>{ " or " }<code>{ "DEMO_JS" }</code>
                        { ", so the demo page is weight it carries without using. Cargo resolves the \
                           dependency, " }<code>{ "Cargo.lock" }</code>
                        { " pins the exact revision, and there is no submodule to forget to \
                           initialise." }
                    </p>
                    <CodeBlock
                        language="toml"
                        code={r##"[dependencies]
# No tag contains the crate this page describes. The remote has v0.2.0 and v0.3.0,
# both cut before the page sections, BeeNest and the ontology tier moved out, so
# pin a rev until a release is tagged off the slimmed history.
eona-ui-toolkit = { git = "ssh://git@gitlab.eona-x.org:29418/eona-x/web/eona-ui-toolkit.git", rev = "<commit>", features = ["csr"] }"##}
                    />
                    <div class="integration-note">
                        <b>{ "Pin a tag or a rev, not a branch." }</b>
                        <p>
                            { "A branch dependency re-resolves whenever the lockfile is regenerated, \
                               which turns an unrelated " }<code>{ "cargo update" }</code>
                            { " into a design-system change nobody asked for. Check which refs \
                               exist before you pin one: " }
                            <code>{ "git ls-remote --tags" }</code>
                            { " today returns only " }<code>{ "v0.2.0" }</code>{ " and " }
                            <code>{ "v0.3.0" }</code>
                            { ", and pinning a tag that was never cut fails at resolve time with a \
                               fetch error, which reads like a permissions problem and is not one." }
                        </p>
                    </div>
                    <p class="section-intro">
                        { "The credentials objection that rules this out for the portal does not \
                           generally apply: an ordinary " }<code>{ "cargo build" }</code>
                        { " on a developer machine, or a CI job with an SSH key or deploy token, \
                           authenticates fine. It only bites when the build happens somewhere that \
                           cannot carry a credential — which is exactly the portal's Docker stage, and \
                           is why that one repository does it the other way." }
                    </p>
                </div>
            </section>

            <section class="block" id="npm">
                <div class="wrap">
                    <SectionHead number="15" kicker="react" title="Install the package" />
                    <div class="integration-note">
                        <b>{ "Not published yet." }</b>
                        <p>
                            { "Nothing below installs today. " }
                            <code>{ "@eona-x/ui-toolkit-react" }</code>
                            { " is generated into this repository at " }
                            <code>{ "packages/react" }</code>
                            { ", and packaging and publishing it is Layer 4 of " }
                            <code>{ "eona-x/backlog#809" }</code>
                            { " — not " }<code>{ "#808" }</code>
                            { ", which ships the stylesheet package and lists this one as out of \
                               scope. Watch " }<code>{ "#809" }</code>
                            { " if you are waiting to install; " }<code>{ "#808" }</code>
                            { " is its prerequisite and can close with nothing to install yet." }
                        </p>
                        <p>
                            { "Two fallbacks work meanwhile, and one obvious one does not. Copy the \
                               generated " }<code>{ "packages/react/src/*.tsx" }</code>
                            { " into the app, or vendor this repository and run " }
                            <code>{ "npm install && npm run build" }</code>
                            { " inside " }<code>{ "packages/react" }</code>
                            { " first. Pointing a dependency at that directory without building it \
                               resolves to nothing: the package's entry points are all under " }
                            <code>{ "dist/" }</code>
                            { ", which is generated by " }<code>{ "tsup" }</code>
                            { " and is neither committed nor present in a fresh checkout." }
                        </p>
                    </div>
                    <p class="section-intro">
                        { "The package is scoped to " }<code>{ "@eona-x" }</code>
                        { " and will live in the " }<code>{ "eona-x" }</code>
                        { " group's npm registry on " }<code>{ "gitlab.eona-x.org" }</code>
                        { ", not on npmjs.org. npm has no way to discover that, so a consumer \
                           declares it in " }<code>{ ".npmrc" }</code>{ ":" }
                    </p>
                    <CodeBlock
                        language="ini"
                        code={r##"# route only the @eona-x scope to GitLab; everything else still resolves from npmjs.org
@eona-x:registry=https://gitlab.eona-x.org/api/v4/groups/299/-/packages/npm/

# key the token on the SAME path as the registry line above: npm walks the registry
# URL's own path looking for a match, so a token on any other path is never sent
//gitlab.eona-x.org/api/v4/groups/299/-/packages/npm/:_authToken=${GITLAB_NPM_TOKEN}"##}
                    />
                    <p class="section-intro">
                        { "299 is the " }<code>{ "eona-x" }</code>
                        { " group — GitLab's npm endpoint takes the numeric id, not the group path, \
                           which is why it is written out rather than spelled. Keep the token in the \
                           environment and let npm interpolate it, rather than committing a " }
                        <code>{ ".npmrc" }</code>
                        { " with the secret in it; a token pasted into a repository is the usual way \
                           a private registry stops being private." }
                    </p>
                    <p class="section-intro">
                        { "The two paths must match, and this is the line that costs an afternoon \
                           when it does not. npm resolves a credential by walking the registry \
                           URL's own path upwards, so a token keyed on " }
                        <code>{ "/api/v4/packages/npm/" }</code>
                        { " is never offered for a registry at " }
                        <code>{ "/api/v4/groups/299/-/packages/npm/" }</code>
                        { " — the first is not a prefix of the second. npm then requests the \
                           package anonymously and the private registry answers 401, which reads \
                           as a credentials problem rather than as the configuration error it is. " }
                        <code>{ "//gitlab.eona-x.org/:_authToken=" }</code>
                        { " on its own is the other correct answer, and the better one if the same \
                           machine also talks to other endpoints on this instance." }
                    </p>
                    <CodeBlock
                        language="sh"
                        code={r##"npm install @eona-x/ui-toolkit-react"##}
                    />
                    <p class="section-intro">
                        { "React " }<strong>{ "19" }</strong>
                        { " or newer is a peer dependency, and for a specific reason rather than a \
                           general preference: " }<code>{ "NavDropdown" }</code>
                        { " renders the " }<code>{ "inert" }</code>
                        { " attribute, which React only added to its DOM attribute types in 19. The \
                           package declares no runtime dependency of its own." }
                    </p>
                </div>
            </section>

            <section class="block block-alt" id="react-stylesheets">
                <div class="wrap">
                    <SectionHead number="16" kicker="react" title="The stylesheets are still the contract" />
                    <p class="section-intro">
                        { "The React components carry the same class names as the Yew ones and no \
                           styles of their own, so the rule from section 12 is unchanged: without " }
                        <code>{ "tokens.css" }</code>{ " and " }<code>{ "components.css" }</code>
                        { " they render as unstyled HTML. Both are required and, as section 12 \
                           explains, their relative order is not. An app that renders " }
                        <code>{ "OntoBadge" }</code>{ " — which " }<em>{ "is" }</em>
                        { " one of the 33 exports — needs " }<code>{ "ontology.css" }</code>
                        { " as well." }
                    </p>
                    <div class="integration-note">
                        <b>{ "That package does not exist yet either." }</b>
                        <p>
                            { "The stylesheets are Rust constants today. " }
                            <code>{ "@eona-x/ui-toolkit-css" }</code>
                            { " is where they are going, as Layer 3 of " }
                            <code>{ "eona-x/backlog#808" }</code>
                            { " — a different issue from the component package's " }
                            <code>{ "#809" }</code>
                            { ", and the one that will ship first. That is also why the npm package \
                               declares no dependencies: it would rather ship no stylesheet than a \
                               copy that drifts from the one the Yew consumers get." }
                        </p>
                    </div>
                    <div class="integration-note">
                        <b>{ "Do not import this repository's " }<code>{ "assets/components.css" }</code>{ " into an app." }</b>
                        <p>
                            { "It is written for a page this crate owns end to end, so it opens with \
                               a page-shell reset on " }<code>{ "*" }</code>{ ", " }
                            <code>{ "html" }</code>{ ", " }<code>{ "body" }</code>{ ", " }
                            <code>{ "img" }</code>{ ", " }<code>{ "a" }</code>{ ", " }
                            <code>{ "h1" }</code>{ "–" }<code>{ "h4" }</code>{ ", " }
                            <code>{ "p" }</code>{ " and " }<code>{ "code" }</code>{ " (" }
                            <code>{ "assets/components.css:13-30" }</code>
                            { ") and sets " }<code>{ "html > body { padding-top: 156px }" }</code>
                            { " at " }<code>{ ":393" }</code>
                            { " to reserve room for " }<code>{ "SiteHeader" }</code>
                            { ". Imported whole into an existing React app it resets the host's \
                               typography and pushes the whole page down 156 pixels. A Yew host that \
                               renders the full page shell wants exactly those rules, which is why \
                               they are still there; a component consumer does not." }
                        </p>
                    </div>
                    <div class="integration-note">
                        <b>{ "The published CSS will be scoped, so the markup needs a wrapper." }</b>
                        <p>
                            { "Layer 3 rewrites every rule in " }<code>{ "components.css" }</code>
                            { " and " }<code>{ "ontology.css" }</code>
                            { " as a descendant of " }<code>{ ".eona-ui" }</code>
                            { " and emits " }<code>{ ".eona-ui { display: contents }" }</code>
                            { " so the wrapper costs no layout — that scoping is what drops the \
                               seven page-shell rules above and keeps the stylesheet from restyling \
                               the host. It also means a component rendered outside a " }
                            <code>{ ".eona-ui" }</code>
                            { " ancestor gets none of the rules. " }<code>{ "tokens.css" }</code>
                            { " is the exception and passes through unscoped, because custom \
                               properties have to be declared on the host document for " }
                            <code>{ "data-theme" }</code>{ " to reach anything." }
                        </p>
                    </div>
                    <CodeBlock
                        language="tsx"
                        code={r##"// what it will look like — in the app's root layout, before anything that renders a component
import '@eona-x/ui-toolkit-css/tokens.css';      // :root custom properties, unscoped
import '@eona-x/ui-toolkit-css/components.css';  // every rule scoped under .eona-ui

// and the markup needs that class somewhere above it; `display: contents` means
// the wrapper adds no box of its own
<div className="eona-ui">
  <Button label="Get started" href="/docs" />
</div>"##}
                    />
                </div>
            </section>

            <section class="block" id="server-components">
                <div class="wrap">
                    <SectionHead number="17" kicker="react" title="No WebAssembly, no client boundary" />
                    <p class="section-intro">
                        { "The package is TypeScript and nothing else. The Rust runs here, at \
                           generation time, and none of it survives into what a consumer installs — \
                           no WebAssembly, no loader, no bundler plugin, no asynchronous \
                           initialisation before the first render. This is worth saying because a \
                           Rust-derived component library is normally exactly the thing that ships a " }
                        <code>{ ".wasm" }</code>{ " and asks the host to wire it up." }
                    </p>
                    <p class="section-intro">
                        { "For the same reason the string " }<code>{ "\"use client\"" }</code>
                        { " does not occur anywhere in the package. Every component is a pure \
                           function of its props with no hooks, no state and no event handlers, so \
                           each one renders inside a React Server Component — no client bundle, no \
                           hydration, no boundary to declare." }
                    </p>
                    <div class="integration-note">
                        <b>{ "A property, not an accident." }</b>
                        <p>
                            { "It follows from the generator's scope rule rather than from \
                               discipline: a component that held state could not be rendered to \
                               fixed markup and so would never have been generated in the first \
                               place. Interaction stays the host's — a hand-written client component \
                               that wraps these, exactly as " }<code>{ "demo-page.js" }</code>
                            { " is the Yew page's glue. The one genuinely stateful thing this crate \
                               used to carry was not ported to React; it went to " }
                            <code>{ "eona-vocabulary-ui" }</code>{ " as Rust." }
                        </p>
                    </div>
                </div>
            </section>

            <section class="block block-alt" id="react-gaps">
                <div class="wrap">
                    <SectionHead number="18" kicker="react" title="What React does not get, and why" />
                    <p class="section-intro">
                        { "Five components have no React counterpart. The generator ships a \
                           component only when it can prove the rendered markup is that component's \
                           props passed through unchanged; four of these five derive their markup \
                           from their props instead, so there is nothing to splice a prop back \
                           into." }
                    </p>
                    <h3 class="integration-case">{ "Swatch" }</h3>
                    <p class="section-intro">
                        { "Recorded as " }<em>{ "panics while rendering a sentinel" }</em>
                        { " — which is what section 09 and " }<code>{ "index.ts" }</code>
                        { " print — and the panic and the derivation are one fact, not two. " }
                        <code>{ "contrast_color" }</code>{ " (" }
                        <code>{ "src/molecules/swatch.rs:7-16" }</code>
                        { ") picks a foreground by parsing the hex prop, so the markup is a \
                           function of the value rather than a slot the value goes into; " }
                        <code>{ ":11" }</code>{ " slices " }<code>{ "&hex[0..2]" }</code>
                        { ", and a sentinel's first character is three or four bytes, so byte 2 is \
                           never a character boundary. Making the panic go away would make it \
                           worse: with a hex-safe probe alphabet every channel parses as zero, the \
                           foreground freezes to white, and the gate emits an unreadable component \
                           quietly instead of refusing one loudly." }
                    </p>

                    <h3 class="integration-case">{ "PaletteGroup" }</h3>
                    <p class="section-intro">
                        { "Renders a " }<code>{ "Swatch" }</code>{ " per colour (" }
                        <code>{ "src/molecules/palette_group.rs:52" }</code>
                        { "), so the panic propagates — and so does the derivation. Fixing " }
                        <code>{ "Swatch" }</code>
                        { " by lifting the foreground out into a prop does not release this one: it \
                           would then have to compute that prop for each colour, and its markup \
                           stays a transform of " }<code>{ "colors" }</code>
                        { ". Both levels have to be lifted, which is what " }
                        <code>{ "abi/overrides.toml" }</code>{ " records." }
                    </p>

                    <h3 class="integration-case">{ "DatasetCard" }</h3>
                    <p class="section-intro">
                        { "Derives the placeholder initial from the first character of " }
                        <code>{ "title" }</code>
                        { ", for entries with no thumbnail. The rest of the card passes through \
                           faithfully, but a component is emitted whole or not at all." }
                    </p>

                    <h3 class="integration-case">{ "OntoAnnotation" }</h3>
                    <p class="section-intro">
                        { "Shortens a datatype IRI to its local name — the part after the last " }
                        <code>{ "#" }</code>{ " or " }<code>{ "/" }</code>
                        { ". A React port that passed the prop straight through would print the \
                           whole IRI where Yew prints " }<code>{ "string" }</code>{ "." }
                    </p>

                    <h3 class="integration-case">{ "Modal" }</h3>
                    <p class="section-intro">
                        { "A different reason: " }<code>{ "inert" }</code>
                        { " is hardcoded, so the dialog has an open state that cannot be rendered \
                           and only the closed one can be proven. Tracked as " }
                        <code>{ "#809" }</code>{ "; it needs an " }<code>{ "open: bool" }</code>
                        { " prop upstream before there is anything to generate." }
                    </p>

                    <p class="section-intro">
                        { "Each is recorded in " }<code>{ "abi/overrides.toml" }</code>
                        { " with the line that forces it, and listed in the package's " }
                        <code>{ "index.ts" }</code>
                        { " — because a component missing on purpose and one missing by accident \
                           look identical from the outside, and only one of them is a bug." }
                    </p>
                    <div class="integration-note">
                        <b>{ "Both ways out are open." }</b>
                        <p>
                            { "Hand-port the component in the consuming app, or lift the transform \
                               out of the Rust component — pass the initial, or the already-shortened \
                               datatype, in as a prop — and it becomes generable and regenerates on \
                               the next run. The second fix is usually the better one: it is also \
                               the change that lets a Yew caller supply its own value." }
                        </p>
                    </div>
                </div>
            </section>

            <section class="block" id="generated">
                <div class="wrap">
                    <SectionHead number="19" kicker="react" title="Generated, never hand-edited" />
                    <p class="section-intro">
                        { "Every file under " }<code>{ "packages/react/src" }</code>
                        { " is written by " }<code>{ "cargo xtask bridgegen" }</code>
                        { " from the Rust in " }<code>{ "src/" }</code>
                        { ". Each component is rendered by Yew's own server renderer with sentinel \
                           values in its props, and JSX is spliced over the byte ranges where those \
                           sentinels landed — so the markup React ships is markup Yew produced, not \
                           markup somebody re-implemented from it." }
                    </p>
                    <p class="section-intro">
                        { "So a bug in a React component is a bug in a Rust component. The " }
                        <code>{ "file:line" }</code>
                        { " it came from is in the header of every generated file; fix it there and \
                           regenerate. A hand edit does not survive, and it does not survive quietly \
                           either:" }
                    </p>
                    <CodeBlock
                        language="sh"
                        code={r##"cargo xtask bridgegen   # regenerate packages/react and abi/ from src/
cargo xtask check       # is packages/react what this src/ would produce? drift fails"##}
                    />
                    <p class="section-intro">
                        { "That gate is what makes \"the same component in two stacks\" a checkable \
                           claim rather than an intention. Without it the two would drift apart on \
                           the first hotfix applied to whichever copy was in front of somebody." }
                    </p>
                    <div class="integration-note">
                        <b>{ "Equivalent output, not byte-identical output." }</b>
                        <p>
                            { "The elements, attributes and class names match the Yew render; the \
                               serialisation does not, in four measured ways that change nothing a \
                               browser shows. Yew writes an author's " }<code>{ "style" }</code>
                            { " string verbatim, trailing semicolon and all, while React serialises \
                               a style object; React normalises whitespace inside " }
                            <code>{ "style" }</code>{ " and Yew's " }<code>{ "Classes" }</code>
                            { " collapses whitespace inside " }<code>{ "class" }</code>
                            { " where React leaves it; and React 19 hoists a " }
                            <code>{ "<link rel=\"preload\" as=\"image\">" }</code>
                            { " for every image it renders. Do not write a snapshot test that diffs \
                               this package's SSR bytes against the portal's — it will fail on the \
                               first render and tell you nothing." }
                        </p>
                    </div>
                </div>
            </section>
        </>
    }
}
