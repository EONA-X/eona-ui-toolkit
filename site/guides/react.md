> **Supported, not recommended for new projects.** The React package is maintained for existing
> React apps and has the same 41 components; for a new site, prefer
> [Svelte](../../svelte/getting-started/) (`npm create @eona-x/app`) or
> [Yew](../../yew/getting-started/).

## Install

```sh
npm install @eona-x/ui-toolkit-react@{{version}}
```

React 19 is a peer dependency. The stylesheet, `@eona-x/ui-toolkit-css`, comes with the package at
the same version. There is no WebAssembly in it: the components are generated from the Yew source
as plain TSX, server-render, and tree-shake.

## Your first component

Import the stylesheet once, and render the components inside an element with the class `eona-ui`
— every rule in the stylesheet is scoped under it, so nothing outside is touched.

```tsx
import '@eona-x/ui-toolkit-css';
import { Button, MetricTile, Pill } from '@eona-x/ui-toolkit-react';

export function App() {
  return (
    <div className="eona-ui">
      <MetricTile label="Datasets" value="12" help="Unchanged since the last run" />
      <Pill tone="ok">Mobility</Pill>
      <Button label="Explore the catalogue" href="https://eona-x.eu/" />
    </div>
  );
}
```

Every component is presentational: no state, no handlers, no `'use client'` — they work in React
Server Components as they are. Interaction belongs to your app. The [Components](../) page renders
every one with its call.

## A static site with Vite

Render the page to HTML at build time and hydrate it in the browser: Vite's own server-rendering
build, plus a ten-line script.

```sh
npm install @eona-x/ui-toolkit-react@{{version}} react react-dom
npm install -D vite @vitejs/plugin-react
```

`package.json`:

```json
{
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build --outDir dist/client && vite build --ssr src/entry-server.tsx --outDir dist/server && node prerender.js"
  }
}
```

`vite.config.js`:

```js
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({ plugins: [react()] });
```

`index.html` — `<!--app-->` is where the rendered page goes:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>My EONA-X app</title>
  </head>
  <body>
    <div id="app"><!--app--></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

`src/App.tsx` is the component above, without the CSS import. Then:

`src/main.tsx`, the browser entry:

```tsx
import '@eona-x/ui-toolkit-css';

import { hydrateRoot } from 'react-dom/client';

import { App } from './App';

hydrateRoot(document.getElementById('app')!, <App />);
```

`src/entry-server.tsx`, the build-time entry:

```tsx
import { renderToString } from 'react-dom/server';

import { App } from './App';

export function render(): string {
  return renderToString(<App />);
}
```

`prerender.js`:

```js
import { readFileSync, writeFileSync } from 'node:fs';

import { render } from './dist/server/entry-server.js';

const page = 'dist/client/index.html';
writeFileSync(page, readFileSync(page, 'utf8').replace('<!--app-->', render()));
```

`npm run build` writes `dist/client/`, a static site with the components already in the HTML.
Next.js, Astro or Remix work the same way: the components need nothing beyond React.

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

The stylesheet has five entry points:

| Import | What |
| --- | --- |
| `@eona-x/ui-toolkit-css` | tokens, components and ontology badges — what you normally want |
| `…/tokens.css` | the custom properties only |
| `…/components.css` | component rules, scoped under `.eona-ui` |
| `…/ontology.css` | `OntoBadge` and `OntoAnnotation`, scoped |
| `…/page.css` | **opt-in, global**: the EONA-X page shell |

`page.css` is for an app whose whole page is an EONA-X page: it resets `body`, headings and links,
and pads `body` by 156px so the fixed `SiteHeader` does not cover the content. Leave it out when
the components live inside your own layout.

`SiteHeader` loads its logo from `logo/logo-transparent.svg` relative to the page. Serve it there —
the file ships in the stylesheet package, at
`node_modules/@eona-x/ui-toolkit-css/logo/logo-transparent.svg`.
