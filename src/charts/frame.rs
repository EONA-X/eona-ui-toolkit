//! What the three SVG charts share: loading, the error and empty states, the
//! figure with its accessible name, the legend and the data table.

use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use yew::prelude::*;

use crate::chart::{coord, format_with_unit, plot, Tick, VIEW};
use crate::cube::{from_jsonld, Axis, Cube, CubeError};

/// A parsed dataset, or the message to show instead.
pub(crate) type Loaded = Result<Cube, String>;

/// A host-side fetch failure wins over the document, which is then empty.
pub(crate) fn load(jsonld: &str, fetch_error: Option<&AttrValue>) -> Loaded {
    if let Some(e) = fetch_error {
        return Err(e.to_string());
    }
    from_jsonld(jsonld).map_err(|e| e.to_string())
}

/// An id that is unique on the page, for `aria-labelledby` and `aria-describedby`.
#[hook]
pub(crate) fn use_chart_id() -> Rc<String> {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    use_memo((), |_| format!("chart-{}", NEXT.fetch_add(1, Ordering::Relaxed)))
}

/// The name an error or empty state carries when no dataset label is known.
const GENERIC_NAME: &str = "Chart";

pub(crate) fn error_state(message: &str, label: Option<&str>) -> Html {
    html! {
        <div class="chart chart--error" role="alert">
            <strong class="chart__state-label">{ label.unwrap_or(GENERIC_NAME).to_owned() }</strong>
            { " " }{ message }
        </div>
    }
}

pub(crate) fn empty_state(label: Option<&str>) -> Html {
    html! {
        <div class="chart chart--empty">
            <strong class="chart__state-label">{ label.unwrap_or(GENERIC_NAME).to_owned() }</strong>
            { " No observations" }
        </div>
    }
}

/// Shows the error and empty states, and otherwise `draw`'s result, for the
/// components that are not an SVG plot (the donut and the tile). The states carry
/// the dataset's label (in `lang`) once the document has been read, `Chart` before.
pub(crate) fn guarded(
    loaded: &Loaded,
    lang: Option<&str>,
    draw: impl FnOnce(&Cube) -> Result<Html, CubeError>,
) -> Html {
    match loaded {
        Err(m) => error_state(m, None),
        Ok(cube) => {
            let label = cube.title.get(lang).filter(|t| !t.is_empty());
            if cube.observations.is_empty() {
                empty_state(label)
            } else {
                draw(cube).unwrap_or_else(|e| error_state(&e.to_string(), label))
            }
        }
    }
}

/// `chart__series--n`, 1-based; a dataset without series draws in the first.
pub(crate) fn series_class(series: Option<usize>) -> String {
    format!("chart__series--{}", series.unwrap_or(0) + 1)
}

/// The accessible name and the description, in `lang`.
pub(crate) fn names(cube: &Cube, lang: Option<&str>, what: &str) -> (String, String) {
    let title = cube.title.get(lang).unwrap_or_default().to_owned();
    let desc = match cube.description.as_ref().and_then(|d| d.get(lang)) {
        Some(d) => d.to_owned(),
        None => {
            let n = cube.observations.len();
            let mut s = format!("{what} of {n} observation{}", if n == 1 { "" } else { "s" });
            if !cube.series.is_empty() {
                s.push_str(&format!(" in {} series", cube.series.len()));
            }
            s
        }
    };
    (title, desc)
}

/// Gridlines, tick labels and the zero baseline.
pub(crate) fn axes(y_ticks: &[Tick], x_labels: &[Tick], zero_y: f64) -> Html {
    let (x0, _, w, _) = plot(VIEW);
    let x1 = x0 + w;
    html! {
        <g class="chart__axes">
            { for y_ticks.iter().map(|t| html! {
                <>
                    <line class="chart__grid" x1={coord(x0)} x2={coord(x1)}
                          y1={coord(t.pos)} y2={coord(t.pos)} />
                    <text class="chart__tick" x={coord(x0 - 8.0)} y={coord(t.pos)}
                          text-anchor="end" dominant-baseline="central">{ &t.label }</text>
                </>
            }) }
            <line class="chart__baseline" x1={coord(x0)} x2={coord(x1)}
                  y1={coord(zero_y)} y2={coord(zero_y)} />
            { for x_labels.iter().map(|t| html! {
                <text class="chart__tick" x={coord(t.pos)} y={coord(VIEW.h - 16.0)}
                      text-anchor="middle">{ &t.label }</text>
            }) }
        </g>
    }
}

/// The figure: the SVG with its title and description, the legend of a multi-series
/// dataset, and the data as a table under a disclosure.
pub(crate) fn figure(
    kind: &str,
    id: &str,
    cube: &Cube,
    lang: Option<&str>,
    what: &str,
    plot_svg: Html,
) -> Html {
    let (title, desc) = names(cube, lang, what);
    let title_id = format!("{id}-title");
    let desc_id = format!("{id}-desc");
    let labelled = title_id.clone();
    let described = desc_id.clone();
    let view_box = format!("0 0 {} {}", VIEW.w, VIEW.h);
    html! {
        <figure class={classes!("chart", format!("chart--{kind}"))}>
            <svg class="chart__svg" viewBox={view_box} role="img" aria-labelledby={labelled} aria-describedby={described}>
                <title id={title_id}>{ title }</title>
                <desc id={desc_id}>{ desc }</desc>
                { plot_svg }
            </svg>
            { legend(cube) }
            { data_table(cube) }
        </figure>
    }
}

fn legend(cube: &Cube) -> Html {
    if cube.series.is_empty() {
        return html! {};
    }
    html! {
        <ul class="chart__legend">
            { for cube.series.iter().enumerate().map(|(i, name)| html! {
                <li class="chart__legend-item">
                    <span class={classes!("chart__swatch", series_class(Some(i)))}
                          aria-hidden="true"></span>
                    { name }
                </li>
            }) }
        </ul>
    }
}

fn data_table(cube: &Cube) -> Html {
    let axis = match cube.axis {
        Axis::Category => "Category",
        Axis::Date(_) => "Date",
    };
    let has_series = !cube.series.is_empty();
    html! {
        <details class="chart__data">
            <summary>{ "Data" }</summary>
            <table>
                <thead>
                    <tr>
                        <th scope="col">{ axis }</th>
                        if has_series { <th scope="col">{ "Series" }</th> }
                        <th scope="col">{ "Value" }</th>
                    </tr>
                </thead>
                <tbody>
                    { for cube.observations.iter().map(|o| html! {
                        <tr>
                            <th scope="row">{ o.key.as_str() }</th>
                            if has_series {
                                <td>{ o.series.and_then(|i| cube.series.get(i)).map(String::as_str).unwrap_or("") }</td>
                            }
                            <td>{ format_with_unit(o.value, cube.unit.as_deref()) }</td>
                        </tr>
                    }) }
                </tbody>
            </table>
        </details>
    }
}
