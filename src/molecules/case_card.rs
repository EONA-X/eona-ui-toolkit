use yew::prelude::*;

use crate::atoms::{TagLabel, TagLabelTone};

/// `.card-case` on eona-x.eu — same shape as `NewsCard` but with a dark
/// content panel, used for case studies.
#[derive(Properties, PartialEq)]
pub struct CaseCardProps {
    pub href: AttrValue,
    pub tag: AttrValue,
    pub title: AttrValue,
}

#[function_component(CaseCard)]
pub fn case_card(props: &CaseCardProps) -> Html {
    html! {
        <article class="case-card">
            <a class="case-card__link" href={props.href.clone()}>
                <div class="case-card__image">
                    <TagLabel text={props.tag.clone()} tone={TagLabelTone::Blue} />
                    { "16:9 image" }
                </div>
                <div class="case-card__content">
                    <h3 class="case-card__title">{ &props.title }</h3>
                </div>
            </a>
        </article>
    }
}
