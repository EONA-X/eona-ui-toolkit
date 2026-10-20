// Does `@eona-x/ui-toolkit-css` style the components exactly as the Yew crate's
// own stylesheets do?
//
// The published sheets are not the crate's: `css.rs` scopes every rule under
// `.eona-ui` and moves the page shell out to `page.css`. This renders every ABI
// prop set (lib.mjs) twice in a real browser —
//
//   A. an EONA-X page: tokens + the crate's `assets/` sheets, unscoped;
//   B. a host app:     tokens + the published sheets, inside `.eona-ui`;
//
// and compares every computed style of every element the components render.
// Both bodies get `margin: 0; padding: 0` so that layout-derived values
// (`width`, an absolutely positioned `top`) start from the same box. Page A's
// body would otherwise carry the shell's 156px offset — exactly the rule the
// published sheets leave to the opt-in `page.css` — and every `position:
// absolute` element (`EvolutionBadge`'s `.sr-only`) would differ by 156px.
//
// Usage: node css.mjs   (Chromium from Playwright, or set CHROMIUM=/path)

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { chromium } from 'playwright-core';

import { abi, buildReact, propSets, renderReact, root } from './lib.mjs';

const read = (p) => readFileSync(join(root, p), 'utf8');

let markup = '';
let cases = 0;
const generated = new Set(
  read('packages/react/src/index.ts')
    .split('\n')
    .map((l) => l.match(/^export \{ (\w+),/)?.[1])
    .filter(Boolean),
);
for (const component of abi.components.filter((c) => generated.has(c.name))) {
  const C = await buildReact(component.name);
  for (const { label, props } of propSets(component)) {
    markup += `<div data-case="${component.name} [${label}]">${renderReact(C, props)}</div>\n`;
    cases++;
  }
}

const page = (css, wrapClass) => `<!doctype html><html><head><meta charset="utf-8">
<style>${css}</style><style>html > body { margin: 0; padding: 0 }</style></head>
<body><div id="root"${wrapClass ? ` class="${wrapClass}"` : ''}>${markup}</div></body></html>`;

const original = page(read('assets/tokens.css') + read('assets/components.css') + read('assets/ontology.css'), '');
const published = page(
  ['tokens.css', 'components.css', 'ontology.css'].map((f) => read(`packages/css/${f}`)).join('\n'),
  'eona-ui',
);

const browser = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
async function styles(html) {
  const p = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await p.setContent(html, { waitUntil: 'load' });
  const out = await p.evaluate(() =>
    [...document.querySelectorAll('[data-case]')].map((c) => ({
      name: c.dataset.case,
      els: [c, ...c.querySelectorAll('*')].map((el) => {
        const cs = getComputedStyle(el);
        const o = {};
        for (const k of cs) o[k] = cs.getPropertyValue(k);
        return { tag: el.tagName, cls: el.getAttribute('class'), style: o };
      }),
    })),
  );
  await p.close();
  return out;
}
const [a, b] = [await styles(original), await styles(published)];
await browser.close();

const failures = [];
a.forEach((ca, i) => {
  const cb = b[i];
  ca.els.forEach((ea, j) => {
    const eb = cb.els[j];
    const diff = Object.keys(ea.style).filter((k) => ea.style[k] !== eb.style[k]);
    if (diff.length) {
      failures.push(
        `${ca.name} <${ea.tag.toLowerCase()} class="${ea.cls ?? ''}">\n` +
          diff.slice(0, 6).map((k) => `    ${k}: ${ea.style[k]}  ->  ${eb.style[k]}`).join('\n'),
      );
    }
  });
});

console.log(`css parity: ${cases} prop sets, every computed style, crate sheets vs published sheets`);
const shown = failures.slice(0, 15);
for (const f of shown) console.log(`\nFAIL ${f}`);
if (failures.length) {
  console.log(`\n${failures.length} element(s) differ`);
  process.exit(1);
}
console.log('identical');
