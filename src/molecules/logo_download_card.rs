use yew::prelude::*;

/// One entry in the Logo section's download grid — a small preview of the
/// mark on a given background plus a real, working SVG download link
/// (native `download` attribute, no JS). `is_transparent` swaps the preview
/// tile to a checkerboard pattern so a transparent background actually
/// reads as transparent rather than looking blank.
#[derive(Properties, PartialEq)]
pub struct LogoDownloadCardProps {
    pub label: AttrValue,
    /// Path to the SVG, e.g. "logo/logo-on-white.svg".
    pub file: AttrValue,
    /// Suggested filename for the download (the `download` attribute value).
    pub download_name: AttrValue,
    #[prop_or_default]
    pub preview_style: AttrValue,
    #[prop_or_default]
    pub is_transparent: bool,
}

#[function_component(LogoDownloadCard)]
pub fn logo_download_card(props: &LogoDownloadCardProps) -> Html {
    let preview_class = if props.is_transparent {
        "logo-download__preview is-transparent"
    } else {
        "logo-download__preview"
    };
    html! {
        <a class="logo-download" href={props.file.clone()} download={props.download_name.clone()}>
            <span class={preview_class} style={props.preview_style.clone()}>
                <img src={props.file.clone()} alt="" loading="lazy" />
            </span>
            <span class="logo-download__label">{ &props.label }</span>
            <span class="logo-download__action">{ "\u{2193} Download SVG" }</span>
        </a>
    }
}
