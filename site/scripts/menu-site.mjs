// Puts the documentation site's menu (site/menu.ttl) into every built page.
//
// The menu is rendered at build time with packages/menu (local workspace code, not
// the npm release): the Turtle is converted with `ttlToModel`, hrefs are made
// relative to the page, the current page is marked, and `renderMenuHtml` writes
// the markup that replaces PLACEHOLDER. menu.js (built from packages/menu) only
// enhances it. Both the Yew-rendered pages and the Vite-built live pages carry
// PLACEHOLDER, so one pass over the finished `dist/` covers all of them.
import { readFileSync, writeFileSync, readdirSync, existsSync, copyFileSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { ttlToModel } from '../../packages/menu/lib/ttl.js';
import { renderMenuHtml } from '../../packages/menu/src/render.js';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const MENU_TTL = join(here, '../menu.ttl');

export const PLACEHOLDER = '<!--eona-menu-->';

// The live gallery pages are three pages for one menu entry.
export function siteModel(file = MENU_TTL) {
  return ttlToModel(readFileSync(file, 'utf8'), { base: pathToFileURL(file).href });
}

const external = (href) => /^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('//');
const depth = (page) => page.split('/').filter(Boolean).length;

// "./" is the site root; any other path is `../` per level, then the path.
const toPage = (href, page) => {
  if (href === null || external(href)) return href;
  const up = '../'.repeat(depth(page));
  return href === './' ? up || './' : up + href;
};

/** The menu markup for the page at `page` (a directory under the site root, '' for the root). */
export function renderForPage(page, model = siteModel()) {
  const here = (href) => href !== null && !external(href) && (href === page || (href === './' && page === ''));
  const item = (i) => ({ ...i, href: toPage(i.href, page), current: here(i.href), children: i.children.map(item) });
  const logo = model.logo && { ...model.logo, href: toPage(model.logo.href, page), src: toPage(model.logo.src, page), srcCompact: toPage(model.logo.srcCompact, page) };
  return renderMenuHtml({ ...model, logo, items: model.items.map(item) });
}

function* pages(dist, dir = dist) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* pages(dist, p);
    else if (name === 'index.html') yield p;
  }
}
const pageOf = (dist, file) => relative(dist, dirname(file)).split(sep).filter(Boolean).map((s) => s + '/').join('');

/** Replace PLACEHOLDER in every index.html under `dist`; add menu.css and menu.js. Returns the page count. */
export function injectDist(dist) {
  const model = siteModel();
  let n = 0;
  for (const file of pages(dist)) {
    const page = pageOf(dist, file);
    const up = '../'.repeat(depth(page));
    let html = readFileSync(file, 'utf8');
    if (!html.includes(PLACEHOLDER)) throw new Error(`${relative(dist, file)}: no ${PLACEHOLDER} to replace`);
    html = html
      .replace(PLACEHOLDER, () => `<div class="eona-ui">${renderForPage(page, model)}</div>`)
      .replace('</head>', () => `<link rel="stylesheet" href="${up}menu.css">\n<link rel="stylesheet" href="${up}docs-menu.css">\n</head>`)
      .replace('</body>', () => `<script src="${up}menu.js"></script>\n</body>`);
    writeFileSync(file, html);
    n++;
  }
  return n;
}

/** Problems found in a built site: pages without the menu, and menu links with no built file. */
export function checkDist(dist) {
  const problems = [];
  for (const file of pages(dist)) {
    const name = relative(dist, file);
    const html = readFileSync(file, 'utf8');
    const m = html.match(/<header class="site-header eona-menu" data-eona-menu[\s\S]*?<\/header>/);
    if (!m) {
      problems.push(`${name}: no menu`);
      continue;
    }
    for (const [, href] of m[0].matchAll(/href="([^"]*)"/g)) {
      if (external(href)) continue;
      const target = resolve(dirname(file), href.replace(/[?#].*$/, ''));
      const path = href.replace(/[?#].*$/, '').endsWith('/') ? join(target, 'index.html') : target;
      if (!existsSync(path)) problems.push(`${name}: href ${href} has no built file`);
    }
    for (const asset of ['menu.css', 'docs-menu.css', 'menu.js']) {
      if (!html.includes(`${asset}"`)) problems.push(`${name}: ${asset} not linked`);
    }
  }
  for (const asset of ['menu.css', 'docs-menu.css', 'menu.js']) if (!existsSync(join(dist, asset))) problems.push(`${asset} missing from the site root`);
  return problems;
}

/** Copy menu.css, the site's own docs-menu.css (site/menu.css) and the built script (packages/menu: `npm run build`) next to the pages. */
export function copyAssets(dist) {
  const js = join(root, 'packages/menu/dist/menu.js');
  if (!existsSync(js)) throw new Error('packages/menu/dist/menu.js is missing: run `node scripts/build.mjs` in packages/menu first');
  copyFileSync(join(root, 'packages/menu/menu.css'), join(dist, 'menu.css'));
  copyFileSync(join(root, 'site/menu.css'), join(dist, 'docs-menu.css'));
  copyFileSync(js, join(dist, 'menu.js'));
}
