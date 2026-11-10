// The documentation site's menu (site/menu.ttl) and the build step that puts it into every page.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { siteModel, renderForPage, injectDist, checkDist, PLACEHOLDER } from '../../../site/scripts/menu-site.mjs';

const labels = (items) => items.map((i) => i.label.en);

test('site/menu.ttl: Overview, Get started, Packages, GitHub', () => {
  const m = siteModel();
  assert.deepEqual(labels(m.items), ['Overview', 'Get started', 'Packages', 'GitHub']);
  assert.deepEqual(labels(m.items[1].children), ['Yew', 'Svelte', 'React', 'Create app']);
  assert.deepEqual(labels(m.items[2].children), ['Components', 'Menu']);
  assert.equal(m.items[1].href, null);
  assert.equal(m.items[3].href, 'https://github.com/EONA-X/eona-ui-toolkit');
  assert.equal(m.items[3].external, true);
});

test('site/menu.ttl: internal hrefs are relative to the site root', () => {
  const all = siteModel().items.flatMap((i) => [i, ...i.children]);
  for (const i of all.filter((x) => x.href && !x.external)) assert.match(i.href, /^(\.\/|[a-z][a-z-]*\/(?:[a-z-]+\/)?)$/, i.href);
});

test('renderForPage: hrefs resolve from the page depth', () => {
  const hrefs = (html) => [...html.matchAll(/href="([^"]*)"/g)].map((m) => m[1]);
  assert.ok(hrefs(renderForPage('')).includes('yew/getting-started/'));
  assert.ok(hrefs(renderForPage('')).includes('./'));
  const deep = hrefs(renderForPage('svelte/getting-started/'));
  assert.ok(deep.includes('../../yew/getting-started/'));
  assert.ok(deep.includes('../../'));
  assert.ok(hrefs(renderForPage('menu/')).includes('../create-app/'));
  assert.ok(deep.includes('https://github.com/EONA-X/eona-ui-toolkit'));
});

test('renderForPage: marks the current page, and only it', () => {
  const cur = (page) => [...renderForPage(page).matchAll(/<a [^>]*aria-current="page"[^>]*>([^<]*)</g)].map((m) => m[1]);
  assert.deepEqual(cur(''), ['Overview']);
  assert.deepEqual(cur('svelte/getting-started/'), ['Svelte']);
  assert.deepEqual(cur('create-app/'), ['Create app']);
  assert.deepEqual(cur('menu/'), ['Menu']);
  assert.deepEqual(cur('yew/'), ['Components']);
  assert.deepEqual(cur('react/'), [], 'Components links to yew/: never the current page on react/');
  assert.deepEqual(cur('svelte/'), [], 'Components links to yew/: never the current page on svelte/');
  assert.match(renderForPage('menu/'), /is-current-ancestor/);
});

test('renderForPage: server-rendered markup the script only enhances, no script', () => {
  const html = renderForPage('');
  assert.match(html, /<header class="site-header eona-menu" data-eona-menu/);
  assert.doesNotMatch(html, /<script/);
  assert.doesNotMatch(html, /is-enhanced/);
});

const page = (extra = '') => `<!doctype html><html><head><title>t</title></head><body>${PLACEHOLDER}${extra}</body></html>`;
function fixture() {
  const dist = mkdtempSync(join(tmpdir(), 'menu-dist-'));
  for (const d of ['', 'yew/', 'yew/getting-started/', 'svelte/getting-started/', 'react/getting-started/', 'create-app/', 'menu/']) {
    mkdirSync(join(dist, d), { recursive: true });
    writeFileSync(join(dist, d, 'index.html'), page());
  }
  mkdirSync(join(dist, 'logo'));
  writeFileSync(join(dist, 'logo/logo-transparent.svg'), '<svg/>');
  writeFileSync(join(dist, 'menu.css'), '');
  writeFileSync(join(dist, 'docs-menu.css'), '');
  writeFileSync(join(dist, 'menu.js'), '');
  return dist;
}

test('injectDist: every index.html gets the menu, its stylesheet and script; checkDist passes', () => {
  const dist = fixture();
  try {
    const n = injectDist(dist);
    assert.equal(n, 7);
    const html = readFileSync(join(dist, 'svelte/getting-started/index.html'), 'utf8');
    assert.doesNotMatch(html, new RegExp(PLACEHOLDER));
    assert.match(html, /data-eona-menu/);
    assert.match(html, /<link rel="stylesheet" href="\.\.\/\.\.\/menu\.css">/);
    assert.match(html, /<script src="\.\.\/\.\.\/menu\.js"><\/script>/);
    assert.deepEqual(checkDist(dist), []);
  } finally {
    rmSync(dist, { recursive: true });
  }
});

test('checkDist: reports a page without the menu and a href with no built file', () => {
  const dist = fixture();
  try {
    injectDist(dist);
    rmSync(join(dist, 'create-app'), { recursive: true });
    writeFileSync(join(dist, 'menu/index.html'), page());
    const problems = checkDist(dist).join('\n');
    assert.match(problems, /menu\/index\.html: no menu/);
    assert.match(problems, /index\.html: href .*create-app\/ has no built file/);
  } finally {
    rmSync(dist, { recursive: true });
  }
});
