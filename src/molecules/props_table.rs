use yew::prelude::*;

/// One row of a [`PropsTable`] — a single field of a component's `Properties`
/// struct.
#[derive(Clone, PartialEq)]
pub struct PropSpec {
    pub name: AttrValue,
    pub ty: AttrValue,
    /// `None` marks the prop required; `Some` carries the literal default, which
    /// is what `#[prop_or(...)]`/`#[prop_or_default]` resolve to.
    pub default: Option<AttrValue>,
    pub doc: AttrValue,
}

impl PropSpec {
    pub fn required(name: &'static str, ty: &'static str, doc: &'static str) -> Self {
        Self { name: name.into(), ty: ty.into(), default: None, doc: doc.into() }
    }

    pub fn optional(
        name: &'static str,
        ty: &'static str,
        default: &'static str,
        doc: &'static str,
    ) -> Self {
        Self { name: name.into(), ty: ty.into(), default: Some(default.into()), doc: doc.into() }
    }
}

/// The props a component accepts, as a table. Hand-maintained: rustdoc is the
/// generated reference, this is the one a designer reads next to the rendering.
#[derive(Properties, PartialEq)]
pub struct PropsTableProps {
    /// Component name, e.g. `"PillButton"`.
    pub component: AttrValue,
    pub props: Vec<PropSpec>,
}

#[function_component(PropsTable)]
pub fn props_table(props: &PropsTableProps) -> Html {
    html! {
        <div class="props-table-wrap">
            <table class="props-table">
                <caption>{ format!("{} props", props.component) }</caption>
                <thead>
                    <tr>
                        <th scope="col">{ "Prop" }</th>
                        <th scope="col">{ "Type" }</th>
                        <th scope="col">{ "Default" }</th>
                        <th scope="col">{ "Notes" }</th>
                    </tr>
                </thead>
                <tbody>
                    { for props.props.iter().map(|p| html! {
                        <tr>
                            <td><code>{ &p.name }</code></td>
                            <td><code class="props-type">{ &p.ty }</code></td>
                            <td>
                                if let Some(default) = &p.default {
                                    <code>{ default }</code>
                                } else {
                                    <span class="props-required">{ "required" }</span>
                                }
                            </td>
                            <td>{ &p.doc }</td>
                        </tr>
                    }) }
                </tbody>
            </table>
        </div>
    }
}
