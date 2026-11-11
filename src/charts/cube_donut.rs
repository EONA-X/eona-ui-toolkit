use yew::prelude::*;

use super::frame;
use crate::chart::donut_of;
use crate::molecules::ScoreDonut;

/// A [`ScoreDonut`] fed from a dataset: the value of one observation read as a
/// fraction in `0..=1` (clamped, see [`crate::chart::donut_arc`]), captioned with
/// the dataset's label.
#[derive(Properties, PartialEq)]
pub struct CubeScoreDonutProps {
    /// The JSON-LD document.
    pub jsonld: AttrValue,
    /// A fetch failure from the host, shown instead of the donut.
    #[prop_or_default]
    pub fetch_error: Option<AttrValue>,
    /// Language tag for the label, e.g. `"fr"`.
    #[prop_or_default]
    pub lang: Option<AttrValue>,
    /// The axis key to show. Without it: the latest date, or the only observation.
    #[prop_or_default]
    pub select: Option<AttrValue>,
}

#[function_component(CubeScoreDonut)]
pub fn cube_score_donut(props: &CubeScoreDonutProps) -> Html {
    let loaded = use_memo((props.jsonld.clone(), props.fetch_error.clone()), |(j, e)| {
        frame::load(j, e.as_ref())
    });
    frame::guarded(&loaded, props.lang.as_deref(), |cube| {
        let arc = donut_of(cube, props.select.as_deref())?;
        let caption = cube.title.get(props.lang.as_deref()).unwrap_or_default().to_owned();
        Ok(html! {
            <ScoreDonut dash_array={arc.dash_array} band={arc.band} label={arc.label}
                        caption={caption} />
        })
    })
}
