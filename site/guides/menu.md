`@eona-x/ui-toolkit-menu` is the fixed top menu of eona-x.eu: a standalone logo, a glass pill with links and dropdown submenus, actions on the right. It collapses into the pill on scroll and becomes an off-canvas drawer below 1024px. Vanilla JavaScript, no runtime dependencies. **The menu at the top of this page is one**: it is configured in [`site/menu.ttl`](https://github.com/EONA-X/eona-ui-toolkit/blob/main/site/menu.ttl), rendered at build time, and enhanced by `menu.js`.

## Install

```sh
npm install @eona-x/ui-toolkit-menu
```

It depends on `@eona-x/ui-toolkit-css` at the same version, which provides the `.site-header*` and `.pill-btn*` styles. Link the three stylesheets, and put the menu inside an element with the class `eona-ui`: `components.css` scopes every rule under it, and `<body class="eona-ui">` is the simplest way.

```html
<link rel="stylesheet" href="@eona-x/ui-toolkit-css/tokens.css">
<link rel="stylesheet" href="@eona-x/ui-toolkit-css/components.css">
<link rel="stylesheet" href="@eona-x/ui-toolkit-menu/menu.css">
```

## Configure in Turtle

The menu is described with the vocabulary `https://eona-x.eu/ontology/ui-toolkit-menu#` (`vocab/menu.ttl` in the package). A Turtle file is converted at build time:

```turtle
@prefix menu: <https://eona-x.eu/ontology/ui-toolkit-menu#> .
@prefix ex:   <https://eona-x.eu/menu/example/> .

ex:docs a menu:Menu ;
  menu:lang "en" ;
  menu:items (
    [ menu:label "Overview"@en ; menu:href "./" ]
    [ menu:label "Get started"@en ;
      menu:children (
        [ menu:label "Yew"@en ; menu:href "yew/getting-started/" ]
        [ menu:label "Svelte"@en ; menu:href "svelte/getting-started/" ]
      ) ]
    [ menu:label "GitHub"@en ; menu:href "https://github.com/EONA-X/eona-ui-toolkit" ; menu:external true ]
  ) .
```

- `menu:items`, `menu:children`, `menu:actions` and `menu:alternates` take an `rdf:List`, which keeps the order. The singular forms (`menu:item`, `menu:child`, `menu:action`, `menu:alternate`) are unordered and sorted by `menu:order`.
- An item has `menu:label` (one per language), `menu:href`, `menu:current`, `menu:external` and `menu:children`. An item with children and no `href` is a disclosure button. Maximum depth is 2.
- The menu has `menu:lang`, `menu:ariaLabel`, `menu:breakpoint` (1024), `menu:collapseOffset` (200), `menu:logo`, `menu:items` and `menu:actions`. An action is a `menu:LinkAction` (`label`, `href`, `variant` `glass` or `solid`) or a `menu:LanguageSwitcher`.
- Write site-relative hrefs as string literals (`menu:href "/fr/"`). An IRI such as `</fr/>` is resolved against the base, which is a `file:` URL when building from a file.

## Configure in JSON-LD

The same model as JSON-LD, loaded at runtime. The context is built in and never fetched.

```json
{
  "@context": "https://eona-x.eu/ontology/ui-toolkit-menu/context.jsonld",
  "@type": "Menu",
  "lang": "en",
  "items": [
    {"label": {"en": "News"}, "href": "/news/"},
    {"label": {"en": "Data space"}, "children": [
      {"label": {"en": "Docs"}, "href": "https://docs.eona-x.eu/", "external": true}
    ]}
  ]
}
```

A plain JSON object in the normalized shape (`version: 1`) is accepted too.

## Server-rendered markup, then enhance

At build time, convert the Turtle and render the markup into the page. The CLI `eona-menu-ttl menu.ttl -o menu.json` writes the normalized JSON; from code:

```js
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { renderMenuHtml } from '@eona-x/ui-toolkit-menu';
import { ttlToModel } from '@eona-x/ui-toolkit-menu/ttl';

const file = new URL('menu.ttl', import.meta.url);
const model = ttlToModel(readFileSync(file, 'utf8'), { base: pathToFileURL(file.pathname).href });
const html = renderMenuHtml(model, { currentUrl: '/news/' });   // <header class="site-header eona-menu" data-eona-menu ...>
```

`renderMenuHtml` options: `lang`, `currentUrl` (marks the item whose path matches with `aria-current="page"`; origin, query and hash are ignored, and `external` items never match), `idPrefix` and `labels` (`menu`, `submenu`). Every text node and attribute is escaped.

Put the markup in the page and load `menu.js` (`dist/menu.js`, a classic script of about 8 KB gzipped). With no `data-src`, no inline config and an existing `[data-eona-menu]` element, it only enhances that markup: scroll collapse, disclosure buttons, the drawer.

```html
<body class="eona-ui">
  <header class="site-header eona-menu" data-eona-menu>…</header>
  …
  <script src="menu.js"></script>
</body>
```

This site does exactly that. `site/scripts/inject-menu.mjs` renders `site/menu.ttl` with the code in `packages/menu` into every built page, prefixing each internal href with `../` once per directory level so it resolves from any depth.

To bake the model into a script instead, `npx eona-menu-build --config menu.ttl --name eona-x` writes `dist/menu.eona-x.js`, which needs no fetch and renders the menu on load. `--config` also accepts `.json` and `.jsonld`.

## Runtime mount

```html
<script src="menu.js" data-src="/menu.jsonld"></script>
```

| attribute | meaning |
|---|---|
| `data-src` | URL of the JSON or JSON-LD |
| `data-target` | CSS selector of the mount point. Default `[data-eona-menu-root]`, else a `div` is inserted first in `body` |
| `data-lang` | label language. Default `<html lang>`, then the menu's `lang` |
| `data-current="auto"` | mark the link matching `location.pathname` as current |
| `data-manual` | do not auto-mount; use `window.EonaMenu` |

Without `data-src`, the first `<script type="application/ld+json" data-eona-menu>` in the page is used, then an inlined config, then existing `[data-eona-menu]` markup. As a module:

```js
import { mountMenu } from '@eona-x/ui-toolkit-menu';
await mountMenu('#nav', '/menu.jsonld', { lang: 'fr', currentUrl: location.pathname });
```

Exports: `mountMenu`, `loadMenu`, `renderMenuHtml`, `enhance`, `normalize`, `validate`, `fromJsonLd`, `jsonldToTriples`, `graphToModel`, `safeHref`, `pickLabel`, `escapeHtml`, `MENU_CONTEXT`, `MENU_CONTEXT_URL`, `MenuError`.

## Without JavaScript

The enhancements are added by `menu.js`, which sets `.is-enhanced` on the menu. Without it:

- Above the breakpoint (1024px), submenus open on hover and on keyboard focus.
- Below it, the menu is static: the toggle, the chevrons and the icon and language disclosure buttons are hidden (a parent item without a link keeps its label as an inactive button above its expanded submenu), the panel stays in the header flow, every submenu is expanded inline and the actions stay visible. Every link is reachable by tap and Tab. The header is `position: relative`, so it does not cover the content.
- The drawer rules apply only under `.is-enhanced`.

## Accessibility

- Disclosure buttons carry `aria-expanded` and `aria-controls`. Escape closes the submenu and returns focus; arrow keys, Home and End move between sublinks. Hover opens a submenu on pointer devices only.
- The drawer moves focus into the panel, traps Tab between the toggle and the panel, closes on Escape and on link clicks, and sets `eona-menu-lock` on `html`.
- The current link has `aria-current="page"`. The nav landmark takes `menu:ariaLabel`.
- `prefers-reduced-motion` is honoured in CSS.

## Security

Hrefs are limited to `http`, `https`, `mailto`, `tel` and scheme-less values; anything else (`javascript:`, `data:`, …) fails validation. External items get `target="_blank"` and `rel="noopener noreferrer"`. Remote JSON-LD contexts are never fetched.

## Styling

At the top of a page on desktop the menu sits on a fixed navy strip. Once scrolled, and on mobile, the pill, language chip and hamburger use a navy glass so they stay legible over any background. Override it with `--eona-menu-chip` (default `rgba(9,27,59,.82)`).
