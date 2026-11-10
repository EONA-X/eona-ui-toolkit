//! Renders the toolkit's documentation site, for publishing on GitHub Pages.
//!
//! ```text
//! /                          overview: the three stacks, and where to start
//! /yew/                      the component gallery (`DemoPage`'s sections)
//! /yew/getting-started/      site/guides/yew.md
//! /react/getting-started/    site/guides/react.md
//! /svelte/getting-started/   site/guides/svelte.md
//! /menu/                     site/guides/menu.md
//! /create-app/               site/guides/create-app.md
//! ```
//!
//! `/react/` and `/svelte/` — the live component pages — are built by Vite from
//! `site/react` and `site/svelte` and copied in by `.github/workflows/pages.yml`.
//! Every page, theirs included, carries the same navigation: the menu from
//! `site/menu.ttl`. This binary leaves `<!--eona-menu-->` where it goes;
//! `site/scripts/inject-menu.mjs` renders it with `packages/menu` into the finished
//! `dist/`. A stack's pages also carry a `HeroFilter` whose two pills switch between
//! its getting started and its components.
//!
//! The portal (`developer.eona-x.eu`) publishes the gallery as `components.html`
//! inside a larger site; this renders it standalone so the crate's own repository
//! can serve it without the portal.
//!
//! Usage: `cargo run --example render-docs --features ssr -- <out-dir>`, then
//! `node site/scripts/inject-menu.mjs <out-dir>`.

use std::{env, fs, path::Path, path::PathBuf};

use eona_ui_toolkit::{
    demo::{DEMO_JS, DEMO_SHELL},
    ComponentsGallery, HeroFilter, IntegrationSection, NavLink, OntologySection, PillButton,
    PillButtonKind, PillarCard, SimpleFooter, COMPONENTS_CSS, LOGO_SVG,
    ONTOLOGY_CSS, TOKENS_CSS,
};
use yew::prelude::*;
use yew::ServerRenderer;

/// The release the guides install, so a copied snippet names a tag that exists.
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Where the menu goes; `site/scripts/inject-menu.mjs` replaces it in the built pages.
const MENU_PLACEHOLDER: &str = "<!--eona-menu-->";
const REPO: &str = "https://github.com/EONA-X/eona-ui-toolkit";

/// A documented stack, in navigation order.
#[derive(Clone, Copy, PartialEq)]
enum Stack {
    Yew,
    React,
    Svelte,
}

impl Stack {
    /// Recommended first: Yew (the source) and Svelte; React is supported.
    const ALL: [Stack; 3] = [Stack::Yew, Stack::Svelte, Stack::React];

    fn slug(self) -> &'static str {
        match self {
            Stack::Yew => "yew",
            Stack::React => "react",
            Stack::Svelte => "svelte",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Stack::Yew => "Yew",
            Stack::React => "React",
            Stack::Svelte => "Svelte",
        }
    }

    fn recommended(self) -> bool {
        !matches!(self, Stack::React)
    }

    fn package(self) -> &'static str {
        match self {
            Stack::Yew => "the eona-ui-toolkit crate",
            Stack::React => "@eona-x/ui-toolkit-react",
            Stack::Svelte => "@eona-x/ui-toolkit-svelte",
        }
    }
}

/// Which page of the site is being rendered: decides the relative prefix back
/// to the root and which navigation entry is current.
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Overview,
    Components(Stack),
    Guide(Stack),
    /// The `@eona-x/ui-toolkit-menu` guide.
    Menu,
    /// The `npm create @eona-x/app` guide.
    CreateApp,
}

impl Page {
    fn dir(self) -> String {
        match self {
            Page::Overview => String::new(),
            Page::Components(s) => format!("{}/", s.slug()),
            Page::Guide(s) => format!("{}/getting-started/", s.slug()),
            Page::Menu => "menu/".into(),
            Page::CreateApp => "create-app/".into(),
        }
    }

    /// `../` once per directory level, so the site works under any base path —
    /// `/eona-ui-toolkit/` on Pages, `/` from a local server.
    fn root(self) -> String {
        "../".repeat(self.dir().matches('/').count())
    }

    fn stack(self) -> Option<Stack> {
        match self {
            Page::Overview | Page::Menu | Page::CreateApp => None,
            Page::Components(s) | Page::Guide(s) => Some(s),
        }
    }
}

#[derive(Properties, PartialEq)]
struct ShellProps {
    page: Page,
    title: AttrValue,
    description: AttrValue,
    children: Children,
}

/// Menu placeholder, hero with the stack switch, content, footer — every Yew-rendered page.
#[function_component(Shell)]
fn shell(props: &ShellProps) -> Html {
    let page = props.page;
    let pills = match page.stack() {
        Some(s) => {
            let guide = matches!(page, Page::Guide(_));
            let root = page.root();
            html! {
                <>
                    <PillButton label="Getting started"
                        kind={if guide { PillButtonKind::Solid } else { PillButtonKind::GlassLight }}
                        href={format!("{root}{}/getting-started/", s.slug())} />
                    <PillButton label="Components"
                        kind={if guide { PillButtonKind::GlassLight } else { PillButtonKind::Solid }}
                        href={format!("{root}{}/", s.slug())} />
                </>
            }
        }
        None => html! {
            <PillButton label={if page == Page::Overview { "View on GitHub" } else { "Package source" }}
                kind={PillButtonKind::GlassLight}
                href={match page {
                    Page::Menu => format!("{REPO}/tree/main/packages/menu"),
                    Page::CreateApp => format!("{REPO}/tree/main/packages/create-app"),
                    _ => REPO.to_string(),
                }} />
        },
    };
    html! {
        <>
            { Html::from_html_unchecked(MENU_PLACEHOLDER.into()) }
            <HeroFilter title={props.title.clone()} description={props.description.clone()} with_backdrop=true>
                { pills }
            </HeroFilter>
            { for props.children.iter() }
            <SimpleFooter
                text={format!("eona-ui-toolkit {VERSION} — one design system, generated for Yew, React and Svelte")}
                link={NavLink::new(REPO, "Source on GitHub")} />
        </>
    }
}

#[function_component(Overview)]
fn overview() -> Html {
    html! {
        <Shell page={Page::Overview}
            title="eona-ui-toolkit"
            description="The EONA-X design system: authored once in Yew, generated for Svelte and React, styled by one stylesheet. Recommended: Rust with Yew, or Svelte — start a Svelte site with npm create @eona-x/app. React is supported for existing React apps.">
            <section class="block">
                <div class="wrap">
                    <div class="docs-stacks">
                        { for Stack::ALL.into_iter().enumerate().map(|(i, s)| html! {
                            <div class="docs-stack">
                                <PillarCard num={format!("0{}", i + 1)}
                                    title={format!("{} — {}", s.name(), if s.recommended() { "recommended" } else { "supported" })}
                                    points={stack_points(s)} />
                                <div class="docs-stack__links">
                                    <PillButton label="Getting started" href={format!("{}/getting-started/", s.slug())} />
                                    // Glass-dark: this row sits on the light page, not on the hero.
                                    <PillButton label="Components" kind={PillButtonKind::GlassDark} href={format!("{}/", s.slug())} />
                                </div>
                            </div>
                        }) }
                    </div>
                </div>
            </section>
        </Shell>
    }
}

fn stack_points(s: Stack) -> Vec<(&'static str, &'static str)> {
    match s {
        Stack::Yew => vec![
            ("Rust", "the components themselves; every other stack is generated from them"),
            ("Builds", "Yew's server renderer for plain HTML, or Trunk for WebAssembly"),
            ("Samples", "examples/yew-ssg and examples/yew-trunk"),
        ],
        Stack::React => vec![
            ("For", "existing React apps; new projects should prefer Yew or Svelte"),
            ("Package", "@eona-x/ui-toolkit-react, TSX generated from the Yew render"),
            ("Builds", "Vite, Next.js, Astro: anything that runs React 19"),
        ],
        Stack::Svelte => vec![
            ("Start", "npm create @eona-x/app: a branded SvelteKit static site"),
            ("Package", "@eona-x/ui-toolkit-svelte, generated from the Yew render"),
            ("Extra", "an opt-in mount for interactive Yew components over WebAssembly"),
        ],
    }
}

#[function_component(YewComponents)]
fn yew_components() -> Html {
    html! {
        <Shell page={Page::Components(Stack::Yew)}
            title="Components"
            description="Every component the crate defines, with the call that renders it in Yew, React and Svelte — followed by the design sections and what it takes to consume the crate.">
            <ComponentsGallery />
            <IntegrationSection />
            <OntologySection />
        </Shell>
    }
}

#[derive(Properties, PartialEq)]
struct GuideProps {
    page: Page,
    title: AttrValue,
    description: AttrValue,
    body: AttrValue,
}

#[function_component(Guide)]
fn guide(props: &GuideProps) -> Html {
    html! {
        <Shell page={props.page} title={props.title.clone()} description={props.description.clone()}>
            <section class="block">
                <div class="wrap docs-guide">
                    { Html::from_html_unchecked(props.body.clone()) }
                </div>
            </section>
        </Shell>
    }
}

/// Render one guide page from `site/guides/<file>.md`.
async fn write_guide(
    out: &Path,
    guides: &Path,
    file: &str,
    page: Page,
    title: String,
    description: String,
) -> Result<usize, Box<dyn std::error::Error>> {
    let body = markdown(&guides.join(format!("{file}.md")))?;
    let t = title.clone();
    let html = ServerRenderer::<Guide>::with_props(move || GuideProps {
        page,
        title: t.into(),
        description: description.into(),
        body: body.into(),
    })
    .render()
    .await;
    write_page(out, page, &format!("{title} — eona-ui-toolkit"), &html)
}

/// Markdown -> HTML, with `{{version}}` filled in.
fn markdown(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    use pulldown_cmark::{html, Options, Parser};
    let src = fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e}", path.display()))?
        .replace("{{version}}", VERSION);
    let mut out = String::new();
    html::push_html(&mut out, Parser::new_ext(&src, Options::ENABLE_TABLES | Options::ENABLE_HEADING_ATTRIBUTES));
    Ok(with_heading_ids(&out))
}

/// `<h2>Install</h2>` -> `<h2 id="install">Install</h2>`, so the guides' own
/// `#the-page-shell` links resolve.
fn with_heading_ids(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find("<h2>") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 4..];
        let end = after.find("</h2>").unwrap_or(after.len());
        let text = &after[..end];
        let slug: String = text
            .chars()
            .filter_map(|c| match c {
                c if c.is_ascii_alphanumeric() => Some(c.to_ascii_lowercase()),
                ' ' | '-' => Some('-'),
                _ => None,
            })
            .collect();
        out.push_str(&format!("<h2 id=\"{slug}\">"));
        rest = after;
    }
    out.push_str(rest);
    out
}

/// One of `DEMO_SHELL`'s references belongs to the portal and a standalone build
/// cannot satisfy it.
///
/// `site-header.js` is the scroll-collapse glue for [`SiteHeader`]. Every portal
/// page uses it, which is why it lives there; without it the header renders and
/// simply never collapses. Vendoring a second copy here is how the two would
/// drift, so the tag is dropped rather than duplicated — a 404 in the console
/// reads as a broken deploy, and this one is not broken.
fn strip_portal_assets(shell: &str) -> String {
    shell
        .lines()
        .filter(|line| !line.contains("site-header.js"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `DEMO_SHELL` for a page at `page`: assets resolved from the root, the docs
/// stylesheet added, and the title set.
fn shell_html(page: Page, title: &str, body: &str) -> String {
    let root = page.root();
    let mut html = strip_portal_assets(DEMO_SHELL);
    for asset in ["tokens.css", "components.css", "ontology.css", "demo-page.js", "logo/logo-transparent.svg"] {
        html = html
            .replace(&format!("href=\"{asset}\""), &format!("href=\"{root}{asset}\""))
            .replace(&format!("src=\"{asset}\""), &format!("src=\"{root}{asset}\""));
    }
    let ontology = format!("<link rel=\"stylesheet\" href=\"{root}ontology.css\">");
    let html = html.replacen(&ontology, &format!("{ontology}\n<link rel=\"stylesheet\" href=\"{root}docs.css\">"), 1);
    let html = match (html.find("<title>"), html.find("</title>")) {
        (Some(a), Some(b)) => format!("{}<title>{title}</title>{}", &html[..a], &html[b + "</title>".len()..]),
        _ => html,
    };
    html.replace("__APP__", body)
}

/// Write a page, with the logo `SiteHeader` loads from `logo/` beside it.
fn write_page(out: &Path, page: Page, title: &str, body: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let dir = out.join(page.dir());
    fs::create_dir_all(dir.join("logo"))?;
    let html = shell_html(page, title, body);
    fs::write(dir.join("index.html"), &html)?;
    fs::write(dir.join("logo/logo-transparent.svg"), LOGO_SVG)?;
    Ok(html.len())
}

/// Only what the Yew-rendered pages add on top of the toolkit's own styles: the
/// overview's three cards, and prose for the guides.
const DOCS_CSS: &str = r#"/* eona-ui-toolkit documentation pages. Everything else is the toolkit's own CSS. */
.docs-stacks { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 24px; }
.docs-stack { display: grid; gap: 16px; align-content: start; }
.docs-stack__links { display: flex; gap: 10px; flex-wrap: wrap; }
.docs-guide { max-width: 860px; }
.docs-guide h2 { margin-top: 2.2em; font-size: var(--eona-size-h3); }
.docs-guide h2:first-child { margin-top: 0; }
.docs-guide p, .docs-guide li { line-height: 1.65; }
.docs-guide a { text-decoration: underline; }
.docs-guide pre {
  margin: 0 0 1.4em; padding: 16px 18px; overflow: auto; border-radius: 12px;
  background: #0b0c3f; color: #e6e7ff; font-size: 13px; line-height: 1.55;
}
.docs-guide :not(pre) > code {
  padding: .1em .35em; border-radius: 6px; background: rgba(4,5,83,.07); font-size: .9em;
}
.docs-guide table { border-collapse: collapse; margin: 0 0 1.4em; width: 100%; font-size: 14px; }
.docs-guide th, .docs-guide td { text-align: left; padding: 8px 10px; border-bottom: 1px solid rgba(4,5,83,.12); vertical-align: top; }
.docs-guide blockquote {
  margin: 0 0 1.4em; padding: 12px 16px; border-left: 4px solid var(--eona-blue-action);
  background: rgba(0,112,240,.06); border-radius: 0 10px 10px 0;
}
.docs-guide blockquote p { margin: 0; }
"#;

/// The overview replaces what used to be the gallery at the root: an old link to
/// `/#snippet-Button` is forwarded to the gallery's new home rather than left on
/// a page that has no such anchor.
const LEGACY_ANCHORS: &str = r#"<script>
if (location.hash.length > 1 && !document.getElementById(location.hash.slice(1))) {
  location.replace('yew/' + location.hash);
}
</script>"#;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out: PathBuf = env::args().nth(1).unwrap_or_else(|| "dist".into()).into();
    let guides = Path::new(env!("CARGO_MANIFEST_DIR")).join("site/guides");
    fs::create_dir_all(&out)?;

    let mut bytes = 0;
    let body = ServerRenderer::<Overview>::new().render().await;
    bytes += write_page(&out, Page::Overview, "eona-ui-toolkit", &format!("{body}\n{LEGACY_ANCHORS}"))?;
    let body = ServerRenderer::<YewComponents>::new().render().await;
    bytes += write_page(&out, Page::Components(Stack::Yew), "Components — Yew — eona-ui-toolkit", &body)?;
    for s in Stack::ALL {
        bytes += write_guide(
            &out,
            &guides,
            s.slug(),
            Page::Guide(s),
            format!("Getting started with {}", s.name()),
            format!("Install {}, render a first component, build a static site, theme it.", s.package()),
        )
        .await?;
    }
    bytes += write_guide(
        &out,
        &guides,
        "menu",
        Page::Menu,
        "Menu".into(),
        "@eona-x/ui-toolkit-menu: install, configure in Turtle or JSON-LD, render at build time, mount at runtime, behaviour without JavaScript.".into(),
    )
    .await?;
    bytes += write_guide(
        &out,
        &guides,
        "create-app",
        Page::CreateApp,
        "Create app".into(),
        "npm create @eona-x/app: what it generates, its options, package-manager detection, version pinning.".into(),
    )
    .await?;

    // Order matters at load time, not on disk: `tokens.css` declares the custom
    // properties the other two consume.
    fs::write(out.join("tokens.css"), TOKENS_CSS)?;
    fs::write(out.join("components.css"), COMPONENTS_CSS)?;
    fs::write(out.join("ontology.css"), ONTOLOGY_CSS)?;
    fs::write(out.join("docs.css"), DOCS_CSS)?;
    fs::write(out.join("demo-page.js"), DEMO_JS)?;

    // Pages runs the upload through Jekyll unless told not to; `_`-prefixed paths
    // would be dropped silently. Nothing here starts with `_` today, so this is a
    // guard against a future asset that does.
    fs::write(out.join(".nojekyll"), "")?;

    println!("wrote 7 pages ({} KB of HTML) to {}", bytes / 1024, out.display());
    Ok(())
}
