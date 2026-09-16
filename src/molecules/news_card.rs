use yew::prelude::*;

use crate::atoms::{TagLabel, TagLabelTone};

/// `.card-news` on eona-x.eu — an image placeholder (real markup uses a
/// photo; this crate doesn't hotlink eona-x.eu's media, so a labeled color
/// block stands in), a tag, and a title, wrapped in one link.
#[derive(Properties, PartialEq)]
pub struct NewsCardProps {
    pub href: AttrValue,
    pub tag: AttrValue,
    pub title: AttrValue,
}

#[function_component(NewsCard)]
pub fn news_card(props: &NewsCardProps) -> Html {
    html! {
        <article class="news-card">
            <a class="news-card__link" href={props.href.clone()}>
                <div class="news-card__image">
                    <TagLabel text={props.tag.clone()} tone={TagLabelTone::Blue} />
                    { "16:9 image" }
                </div>
                <div class="news-card__content">
                    <h3 class="news-card__title">{ &props.title }</h3>
                </div>
            </a>
        </article>
    }
}
