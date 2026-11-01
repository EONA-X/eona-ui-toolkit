# @eona-x/ui-toolkit-menu

Fixed top menu in the style of the eona-x.eu header: standalone logo, glass pill with links and dropdown submenus, actions on the right. It collapses into the pill on scroll and becomes an off-canvas drawer below 1024px. Vanilla JS, no runtime dependencies. The menu is described in Turtle (resolved at build time) or JSON-LD (loaded at runtime).

Styling comes from `@eona-x/ui-toolkit-css` (`.site-header*`, `.pill-btn*`) plus `menu.css` from this package.

```html
<link rel="stylesheet" href="@eona-x/ui-toolkit-css/tokens.css">
<link rel="stylesheet" href="@eona-x/ui-toolkit-css/components.css">
<link rel="stylesheet" href="@eona-x/ui-toolkit-menu/menu.css">
```

`components.css` scopes every rule under `.eona-ui`, so the menu must sit inside an element with that class. The simplest way is `<body class="eona-ui">`, which also covers the `div` that auto-mount inserts at the top of `body`.

## Runtime: JSON-LD

```html
<script src="menu.js" data-src="/menu.jsonld"></script>
```

`dist/menu.js` is a classic script (about 8 KB gzipped). Attributes on its `<script>` tag:

| attribute | meaning |
|---|---|
| `data-src` | URL of the JSON or JSON-LD |
| `data-target` | CSS selector of the mount point. Default `[data-eona-menu-root]`, else a `div` is inserted first in `body` |
| `data-lang` | label language. Default `<html lang>`, then the menu's `lang` |
| `data-current="auto"` | mark the link matching `location.pathname` as current (origin, query and hash are ignored; items marked `external` never match) |
| `data-manual` | do not auto-mount, use `window.EonaMenu` |

Without `data-src`, the first `<script type="application/ld+json" data-eona-menu>` in the page is used, then an inlined config, then existing `[data-eona-menu]` markup (enhance only, for server-rendered output).

As a module:

```js
import { mountMenu } from '@eona-x/ui-toolkit-menu';
await mountMenu('#nav', '/menu.jsonld', { lang: 'fr', currentUrl: location.pathname });
```

Exports: `mountMenu`, `loadMenu`, `renderMenuHtml`, `enhance`, `normalize`, `validate`, `fromJsonLd`, `jsonldToTriples`, `graphToModel`, `safeHref`, `pickLabel`, `escapeHtml`, `MENU_CONTEXT`.

## Build time: Turtle

```sh
npx eona-menu-ttl menu.ttl -o menu.json        # Turtle to normalized JSON
npx eona-menu-build --config menu.ttl --name eona-x   # dist/menu.eona-x.js, model inlined
```

`dist/menu.<name>.js` needs no fetch: it renders the menu on load. `--config` also accepts `.json` and `.jsonld`. Write site-relative hrefs as string literals (`menu:href "/fr/"`): an IRI such as `</fr/>` is resolved against the base, which is a `file:` URL when building from a file.

## Vocabulary

Namespace `https://eona-x.eu/ns/menu#` (`vocab/menu.ttl`). The JSON-LD context is built in (`https://eona-x.eu/ns/menu/context.jsonld`, also `vocab/context.jsonld`) and is never fetched.

- Classes: `Menu`, `Item`, `Logo`, `LinkAction`, `LanguageSwitcher`, `Alternate`.
- Menu: `lang`, `ariaLabel`, `breakpoint` (1024), `collapseOffset` (200), `logo`, `items`, `actions`.
- Item: `label` (one per language), `href`, `current`, `external`, `children`. Maximum depth 2.
- Logo: `href`, `src`, `srcCompact`, `label`, `width`, `height`.
- Action: `LinkAction` (`label`, `href`, `variant` = `glass` or `solid`) or `LanguageSwitcher` (`alternates`, each with `lang`, `label`, `href`, `current`).
- Order: `items`, `children`, `actions`, `alternates` take an `rdf:List`. The singular forms (`item`, `child`, `action`, `alternate`) are unordered and sorted by `menu:order`.

```json
{
  "@context": "https://eona-x.eu/ns/menu/context.jsonld",
  "@type": "Menu",
  "lang": "en",
  "items": [
    {"label": {"en": "News"}, "href": "/news/"},
    {"label": {"en": "Data space"}, "children": [{"label": {"en": "Docs"}, "href": "https://docs.eona-x.eu/", "external": true}]}
  ]
}
```

A plain JSON object in the normalized shape (`version: 1`) is accepted too.

## Behaviour and accessibility

- Disclosure buttons use `aria-expanded` and `aria-controls`. Escape closes and returns focus. Arrow keys, Home and End move between sublinks. Hover opens on pointer devices only.
- Drawer: focus moves into the panel, Tab is trapped between the toggle and the panel, Escape and link clicks close it, and `html` gets `eona-menu-lock`.
- Without JavaScript, server-rendered submenus open on hover and focus.
- Reduced motion is honoured in CSS.

## Security

Every text node and attribute is HTML-escaped. Hrefs are limited to `http`, `https`, `mailto`, `tel` and scheme-less values; anything else (`javascript:`, `data:`, ...) fails validation. Remote JSON-LD contexts are never fetched.
