//! The Rust half of `@eona-x/ui-toolkit-svelte/wasm`.
//!
//! The Svelte side (`use:yew`, `<YewMount>`) calls exactly one export:
//!
//! ```text
//! mount(target: Element, component: &str, props: JsValue) -> Result<Handle, JsValue>
//! ```
//!
//! and later `handle.update(props)` / `handle.destroy()`. Everything else here
//! is how *this* crate chooses to meet that contract, and a consumer crate is
//! expected to copy the shape and replace the registry in [`mount`].
//!
//! Props cross the boundary as plain JavaScript values. The toolkit's
//! `Properties` structs are neither `Deserialize` (the crate depends on `yew`
//! and nothing else, on purpose) nor exported, so a toolkit component is
//! mounted through a thin host component that renders it with `html!`, as any
//! Yew caller would; the host's props come from a small serde mirror. That
//! conversion is also where an unknown variant becomes an error instead of a
//! silent default.
//!
//! Build: `wasm-pack build --target web examples/svelte-wasm`.

use serde::Deserialize;
use wasm_bindgen::prelude::*;
use yew::prelude::*;
use yew::{AppHandle, BaseComponent, Renderer};

use eona_ui_toolkit::{Button, ButtonVariant};

/// Mount `component` into `target` with `props`.
///
/// `target` must be empty and stay owned by the caller: Svelte created it and
/// will remove it; Yew only manages what it renders inside.
#[wasm_bindgen]
pub fn mount(target: web_sys::Element, component: &str, props: JsValue) -> Result<Handle, JsValue> {
    match component {
        "Button" => start::<ButtonHost>(target, props, button_props),
        "Counter" => start::<Counter>(target, props, counter_props),
        other => Err(JsValue::from_str(&format!(
            "this module mounts Button and Counter, not {other:?}"
        ))),
    }
}

/// A mounted component, handed back to JavaScript.
#[wasm_bindgen]
pub struct Handle {
    inner: Option<Box<dyn Mounted>>,
}

#[wasm_bindgen]
impl Handle {
    /// Re-render with new props. Yew diffs, so component state survives.
    pub fn update(&mut self, props: JsValue) -> Result<(), JsValue> {
        match &mut self.inner {
            Some(m) => m.update(props),
            None => Err(JsValue::from_str("update() after destroy()")),
        }
    }

    /// Unmount. Idempotent, because Svelte may tear down twice on a fast
    /// navigation and a second call must not be a panic across the boundary.
    pub fn destroy(&mut self) {
        if let Some(m) = self.inner.take() {
            m.destroy();
        }
    }
}

/// Erases `AppHandle<C>`'s component type so one `Handle` can hold any of them.
trait Mounted {
    fn update(&mut self, props: JsValue) -> Result<(), JsValue>;
    fn destroy(self: Box<Self>);
}

struct App<C: BaseComponent> {
    app: AppHandle<C>,
    read: fn(JsValue) -> Result<C::Properties, JsValue>,
}

impl<C: BaseComponent> Mounted for App<C> {
    fn update(&mut self, props: JsValue) -> Result<(), JsValue> {
        let props = (self.read)(props)?;
        self.app.update(props);
        Ok(())
    }

    fn destroy(self: Box<Self>) {
        self.app.destroy();
    }
}

fn start<C: BaseComponent>(
    target: web_sys::Element,
    props: JsValue,
    read: fn(JsValue) -> Result<C::Properties, JsValue>,
) -> Result<Handle, JsValue> {
    let app = Renderer::<C>::with_root_and_props(target, read(props)?).render();
    Ok(Handle { inner: Some(Box::new(App { app, read })) })
}

fn bad(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

// ------------------------------------------------------------ Button (toolkit)

#[derive(Deserialize)]
struct ButtonIn {
    label: String,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    href: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct ButtonHostProps {
    pub label: AttrValue,
    pub variant: ButtonVariant,
    pub href: Option<AttrValue>,
}

#[function_component(ButtonHost)]
pub fn button_host(p: &ButtonHostProps) -> Html {
    html! { <Button label={p.label.clone()} variant={p.variant} href={p.href.clone()} /> }
}

fn button_props(v: JsValue) -> Result<ButtonHostProps, JsValue> {
    let i: ButtonIn = serde_wasm_bindgen::from_value(v).map_err(bad)?;
    // The same spellings `@eona-x/ui-toolkit-svelte`'s `ButtonProps` uses.
    let variant = match i.variant.as_deref() {
        None | Some("primary") => ButtonVariant::Primary,
        Some("ghost") => ButtonVariant::Ghost,
        Some(other) => return Err(bad(format!("Button: unknown variant {other:?}"))),
    };
    Ok(ButtonHostProps { label: i.label.into(), variant, href: i.href.map(Into::into) })
}

// -------------------------------------------------- Counter (interactive demo)

/// Stands in for the interactive tier, which the toolkit does not ship (it
/// lives in `eona-vocabulary-ui`): a component with state and a callback,
/// i.e. exactly what the generated package cannot express.
#[derive(Properties, PartialEq)]
pub struct CounterProps {
    pub start: i32,
    #[prop_or_default]
    pub label: AttrValue,
}

#[function_component(Counter)]
pub fn counter(props: &CounterProps) -> Html {
    let n = use_state(|| props.start);
    let onclick = {
        let n = n.clone();
        Callback::from(move |_| n.set(*n + 1))
    };
    html! {
        <button class="btn btn-ghost" type="button" {onclick}>
            { format!("{} {}", props.label, *n) }
        </button>
    }
}

#[derive(Deserialize)]
struct CounterIn {
    #[serde(default)]
    start: i32,
    #[serde(default)]
    label: String,
}

fn counter_props(v: JsValue) -> Result<CounterProps, JsValue> {
    let i: CounterIn = serde_wasm_bindgen::from_value(v).map_err(bad)?;
    Ok(CounterProps { start: i.start, label: i.label.into() })
}
