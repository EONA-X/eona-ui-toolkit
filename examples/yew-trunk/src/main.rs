//! A branded Yew app, compiled to WebAssembly and bundled by Trunk:
//! `trunk serve` to develop, `trunk build --release` for a static `dist/`.
use eona_ui_toolkit::{
    HeroFilter, MetricTile, NavLink, Pill, PillButton, PillTone, SectionHead, SimpleFooter,
    SiteHeader, SiteNavLink, COMPONENTS_CSS, TOKENS_CSS,
};
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    // State lives in the app: the toolkit's components are presentational.
    let datasets = use_state(|| 12);
    let add = {
        let datasets = datasets.clone();
        Callback::from(move |_| datasets.set(*datasets + 1))
    };
    html! {
        <>
            // The crate's stylesheets, embedded: no CSS file to serve or keep in step.
            <style>{ TOKENS_CSS }{ COMPONENTS_CSS }{ SITE_CSS }</style>
            <SiteHeader links={vec![SiteNavLink::new("./", "Home", true)]} />
            <HeroFilter title="My EONA-X app"
                description="A Yew app compiled to WebAssembly by Trunk, on the toolkit's components."
                with_backdrop=true>
                <PillButton label="eona-x.eu" href="https://eona-x.eu/" />
            </HeroFilter>
            <section class="block">
                <div class="wrap">
                    <SectionHead number="01" kicker="at a glance" title="Your first components" />
                    <div class="tiles">
                        <MetricTile label="Datasets" value={datasets.to_string()} help="Click the button to add one" />
                        <MetricTile label="Participants" value="38" />
                    </div>
                    <p class="pills">
                        <Pill tone={PillTone::Ok}>{ "Mobility" }</Pill>
                        <button class="btn btn-primary" type="button" onclick={add}>{ "Add a dataset" }</button>
                    </p>
                </div>
            </section>
            <SimpleFooter text="A Yew app built with the EONA-X design system"
                link={NavLink::new("https://eona-x.eu/", "eona-x.eu")} />
        </>
    }
}

/// This app's own layout rules; everything else is the toolkit's.
const SITE_CSS: &str = ".tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 18px; }
.pills { display: flex; gap: 8px; align-items: center; margin-top: 24px; }
";

fn main() {
    yew::Renderer::<App>::new().render();
}
