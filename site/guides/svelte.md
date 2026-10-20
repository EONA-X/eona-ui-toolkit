## Install

```sh
npm install @eona-x/ui-toolkit-svelte@{{version}}
```

Svelte 5 is a peer dependency. The stylesheet, `@eona-x/ui-toolkit-css`, comes with the package at
the same version. The components are `.svelte` files generated from the Yew source, compiled by
your own Svelte, with no WebAssembly unless you opt into the [`/wasm` entry](#mounting-a-yew-component).

## Your first component

Import the stylesheet once, and render the components inside an element with the class `eona-ui`
— every rule in the stylesheet is scoped under it, so nothing outside is touched.

```svelte
<script>
  import '@eona-x/ui-toolkit-css';
  import { Button, MetricTile, Pill } from '@eona-x/ui-toolkit-svelte';
</script>

<div class="eona-ui">
  <MetricTile label="Datasets" value="12" help="Unchanged since the last run" />
  <Pill tone="ok">Mobility</Pill>
  <Button label="Explore the catalogue" href="https://eona-x.eu/" />
</div>
```

A Rust `Children` prop is a Svelte snippet: pass `children` as element content, and any other slot
— `ShapeCard`'s `demo` — as a `{#snippet demo()}…{/snippet}` block. Every component is
presentational; interaction belongs to your app. The [Components](../) page renders every one with
its call.

## A static site with SvelteKit

SvelteKit with `adapter-static` renders every route to HTML at build time — on Vite underneath.

```sh
npm install @eona-x/ui-toolkit-svelte@{{version}}
npm install -D @sveltejs/kit @sveltejs/adapter-static @sveltejs/vite-plugin-svelte svelte vite
```

`package.json`:

```json
{
  "type": "module",
  "scripts": { "dev": "vite dev", "build": "vite build" }
}
```

`svelte.config.js`:

```js
import adapter from '@sveltejs/adapter-static';

export default { kit: { adapter: adapter() } };
```

`vite.config.js`:

```js
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({ plugins: [sveltekit()] });
```

`src/app.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    %sveltekit.head%
  </head>
  <body>
    <div style="display: contents">%sveltekit.body%</div>
  </body>
</html>
```

`src/routes/+layout.js` — every route is rendered at build time:

```js
export const prerender = true;
```

`src/routes/+layout.svelte` — the stylesheet and the scope, once for the whole site:

```svelte
<script>
  import '@eona-x/ui-toolkit-css';

  let { children } = $props();
</script>

<div class="eona-ui">
  {@render children()}
</div>
```

`src/routes/+page.svelte`:

```svelte
<script>
  import { Button, MetricTile, Pill } from '@eona-x/ui-toolkit-svelte';
</script>

<MetricTile label="Datasets" value="12" help="Unchanged since the last run" />
<Pill tone="ok">Mobility</Pill>
<Button label="Explore the catalogue" href="https://eona-x.eu/" />
```

`npm run build` writes `build/`. The prerender crawls every link and fails on one to a route that
does not exist — link to real pages, or to absolute URLs as here.

## Mounting a Yew component

Everything in the toolkit is presentational, so the default entry covers it all. For an interactive
Yew component — one with state and callbacks, which cannot be generated — the `/wasm` entry mounts
the real thing, compiled to WebAssembly:

```svelte
<script>
  import { YewMount } from '@eona-x/ui-toolkit-svelte/wasm';

  // A function, so the WebAssembly loads only when the component mounts.
  const load = () => import('my-yew-widgets').then(async (m) => (await m.default(), m));
</script>

<YewMount module={load} component="Counter" props={{ start: 3, label: 'Clicks' }} />
```

The Rust side exports one function, built with `wasm-bindgen` and Yew's `csr` feature:

```rust
#[wasm_bindgen]
pub fn mount(target: web_sys::Element, component: &str, props: JsValue) -> Result<Handle, JsValue>
```

where `Handle` has `update(props)` and `destroy()`. [`examples/svelte-wasm`](https://github.com/EONA-X/eona-ui-toolkit/tree/main/examples/svelte-wasm)
is a complete one — the toolkit's `Button` and a stateful `Counter` — built with
`wasm-pack build --target web`. `use:yew` is the same thing as an action, for an element you
already have.

It has a cost: one component over WebAssembly is about 75 KB gzip against about 100 bytes
pre-rendered, it renders nothing on the server, and a Content-Security-Policy must allow
`wasm-unsafe-eval`. Reach for it only for what the default entry does not export.

## Theming

`@eona-x/ui-toolkit-css` has a semantic layer named after the job rather than the ink —
`--eona-surface-page`, `--eona-text-primary`, `--eona-border-subtle`, `--eona-accent` — and a dark
variant of it, switched on the document:

```html
<html data-theme="dark">
```

Use those tokens for your own surfaces and they follow the theme. **The components do not yet:**
their rules still read the raw palette, so they render the same in both modes.

## The page shell

`@eona-x/ui-toolkit-css/page.css` is **opt-in and global**: the EONA-X page shell, for a site whose
whole page is an EONA-X page. It resets `body`, headings and links, and pads `body` by 156px so the
fixed `SiteHeader` does not cover the content. Leave it out when the components live inside your
own layout.

`SiteHeader` loads its logo from `logo/logo-transparent.svg` relative to the page. Serve it there —
the file ships in the stylesheet package, at
`node_modules/@eona-x/ui-toolkit-css/logo/logo-transparent.svg` (in SvelteKit, copy it to
`static/logo/`).
