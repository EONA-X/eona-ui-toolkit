//! A branded static site, rendered by Yew at build time: `cargo run` writes
//! `dist/`, which any static host serves. No WebAssembly reaches the browser.
use std::fs;
use std::path::Path;

use eona_ui_toolkit::{
    HeroFilter, MetricTile, NavLink, Pill, PillButton, PillButtonKind, PillTone, SectionHead,
    SimpleFooter, SiteHeader, SiteNavLink, COMPONENTS_CSS, LOGO_SVG, TOKENS_CSS,
};
use yew::prelude::*;

/// One page of the site: where it is written, and what it is called in the header.
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    About,
}

impl Page {
    const ALL: [Page; 2] = [Page::Home, Page::About];

    /// The directory the page is written to, relative to `dist/`.
    fn dir(self) -> &'static str {
        match self {
            Page::Home => "",
            Page::About => "about/",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::About => "About",
        }
    }

    /// `../` per level, so the site works from any base path.
    fn root(self) -> &'static str {
        match self {
            Page::Home => "",
            Page::About => "../",
        }
    }
}

#[derive(Properties, PartialEq)]
struct LayoutProps {
    page: Page,
    children: Children,
}

/// The EONA-X header and footer around every page.
#[function_component(Layout)]
fn layout(props: &LayoutProps) -> Html {
    let links = Page::ALL
        .map(|p| SiteNavLink {
            href: format!("{}{}", props.page.root(), p.dir()).into(),
            label: p.label().into(),
            current: p == props.page,
        })
        .to_vec();
    html! {
        <>
            <SiteHeader {links} />
            { for props.children.iter() }
            <SimpleFooter text="A static site built with the EONA-X design system"
                link={NavLink::new("https://eona-x.eu/", "eona-x.eu")} />
        </>
    }
}

#[function_component(Home)]
fn home() -> Html {
    html! {
        <Layout page={Page::Home}>
            <HeroFilter title="My EONA-X site"
                description="Rendered by Yew at build time: every page is plain HTML, styled by the toolkit's own stylesheets."
                with_backdrop=true>
                <PillButton label="About this site" href="about/" />
            </HeroFilter>
            <section class="block">
                <div class="wrap">
                    <SectionHead number="01" kicker="at a glance" title="Your first components" />
                    <div class="tiles">
                        <MetricTile label="Datasets" value="12" help="Unchanged since the last run" />
                        <MetricTile label="Participants" value="38" />
                        <MetricTile label="Records analysed" value="128 430" />
                    </div>
                    <p class="pills">
                        <Pill tone={PillTone::Ok}>{ "Mobility" }</Pill>
                        <Pill tone={PillTone::Ok}>{ "Tourism" }</Pill>
                    </p>
                </div>
            </section>
        </Layout>
    }
}

#[function_component(About)]
fn about() -> Html {
    html! {
        <Layout page={Page::About}>
            <HeroFilter title="About" description="How this site is put together." with_backdrop=true>
                <PillButton label="Back home" kind={PillButtonKind::GlassLight} href="../" />
            </HeroFilter>
            <section class="block">
                <div class="wrap">
                    <p>
                        { "Each page is a Yew component, rendered to HTML by " }<code>{ "src/main.rs" }</code>
                        { " with " }<code>{ "ServerRenderer" }</code>{ ". Add a page: a component, a " }
                        <code>{ "Page" }</code>{ " variant, and a line in " }<code>{ "main" }</code>{ "." }
                    </p>
                </div>
            </section>
        </Layout>
    }
}

/// This site's own layout rules; everything else is the toolkit's.
const SITE_CSS: &str = ".tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 18px; }
.pills { display: flex; gap: 8px; margin-top: 24px; }
";

fn page_html(page: Page, title: &str, body: &str) -> String {
    let root = page.root();
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n\
         <link rel=\"icon\" type=\"image/svg+xml\" href=\"{root}logo/logo-transparent.svg\">\n\
         <link href=\"https://fonts.googleapis.com/css2?family=Poppins:wght@300;400;500;600;700&display=swap\" rel=\"stylesheet\">\n\
         <link rel=\"stylesheet\" href=\"{root}tokens.css\">\n\
         <link rel=\"stylesheet\" href=\"{root}components.css\">\n\
         <link rel=\"stylesheet\" href=\"{root}site.css\">\n\
         </head>\n<body>\n{body}\n</body>\n</html>\n"
    )
}

fn write_page(out: &Path, page: Page, title: &str, body: &str) -> std::io::Result<()> {
    let dir = out.join(page.dir());
    // `SiteHeader` loads its mark from `logo/` beside the page.
    fs::create_dir_all(dir.join("logo"))?;
    fs::write(dir.join("logo/logo-transparent.svg"), LOGO_SVG)?;
    fs::write(dir.join("index.html"), page_html(page, title, body))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let out = Path::new("dist");
    // `hydratable(false)`: plain HTML, without the markers a client would hydrate from.
    let home = yew::ServerRenderer::<Home>::new().hydratable(false).render().await;
    write_page(out, Page::Home, "My EONA-X site", &home)?;
    let about = yew::ServerRenderer::<About>::new().hydratable(false).render().await;
    write_page(out, Page::About, "About — My EONA-X site", &about)?;
    fs::write(out.join("tokens.css"), TOKENS_CSS)?;
    fs::write(out.join("components.css"), COMPONENTS_CSS)?;
    fs::write(out.join("site.css"), SITE_CSS)?;
    println!("wrote dist/ (2 pages)");
    Ok(())
}
