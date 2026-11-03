import test, { before } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, existsSync, mkdtempSync, rmSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { gzipSync } from 'node:zlib';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { JSDOM } from 'jsdom';

const root = new URL('..', import.meta.url).pathname;
const build = (...args) => execFileSync(process.execPath, [join(root, 'scripts/build.mjs'), ...args], { cwd: root, encoding: 'utf8' });
const fixture = (f) => readFileSync(join(root, 'test/fixtures', f), 'utf8');
const model = JSON.parse(fixture('eona-x.model.json'));
const ORDER = ['errors', 'model', 'rdf', 'jsonld', 'render', 'behavior', 'mount', 'auto'];

before(() => build());

test('source constraints', () => {
  const names = new Map();
  for (const f of ORDER) {
    const src = readFileSync(join(root, 'src', f + '.js'), 'utf8');
    let inBlock = false;
    src.split('\n').forEach((line, i) => {
      const where = `${f}.js:${i + 1}`;
      if (inBlock) { if (/\*\/\s*$/.test(line)) inBlock = false; return; }
      const t = line.trim();
      if (/^\/\*/.test(t)) { if (!/\*\/\s*$/.test(t)) inBlock = true; return; }
      if (t.startsWith('//')) return;
      if (/^import\b/.test(t)) assert.match(t, /^import \{ [\w, ]+ \} from '\.\/\w+\.js';$/, where);
      assert.equal(/^export\b/.test(t) && !/^export (function|const|class) /.test(t), false, where + ' export form');
      assert.equal(/(^|[^:'"`\\])\/\/\s/.test(t.replace(/(['"`]).*?\1/g, '""').replace(/\/(?:\\.|[^/\n])+\/[gimsuy]*/g, '')), false, where + ' trailing comment');
      const m = /^(?:export )?(?:function|const|class|let|var) (\w+)/.exec(line);
      if (m) { assert.equal(names.has(m[1]), false, `${where} duplicate ${m[1]} (also ${names.get(m[1])})`); names.set(m[1], where); }
    });
  }
});

const bundle = () => readFileSync(join(root, 'dist/menu.js'), 'utf8');

test('dist is plain script', () => {
  const s = bundle();
  assert.ok(s.startsWith('var __EONA_MENU_CONFIG__ = null;'));
  assert.equal(/^\s*(import|export)\s/m.test(s), false);
  assert.doesNotThrow(() => new Function(s));
});

const run = (script, { html = '<!doctype html><html lang="fr"><body></body></html>', fetch, url = 'https://eona-x.eu/fr/' } = {}) => {
  const dom = new JSDOM(html, { runScripts: 'outside-only', url, pretendToBeVisual: false });
  const w = dom.window;
  w.matchMedia = () => ({ matches: true, addEventListener() {}, removeEventListener() {} });
  if (fetch) w.fetch = fetch;
  const el = w.document.createElement('script');
  for (const [k, v] of Object.entries(script.attrs || {})) el.setAttribute(k, v);
  w.document.body.appendChild(el);
  Object.defineProperty(w.document, 'currentScript', { configurable: true, get: () => el });
  w.eval(script.src);
  return w;
};
const tick = () => new Promise((r) => setTimeout(r, 20));
const stubFetch = (body) => async () => ({ ok: true, status: 200, json: async () => body });

test('window.EonaMenu members', () => {
  const w = run({ src: bundle(), attrs: { 'data-manual': '' } });
  for (const k of ['mountMenu', 'loadMenu', 'renderMenuHtml', 'normalize', 'fromJsonLd', 'enhance', 'version']) assert.ok(w.EonaMenu[k] !== undefined, k);
  assert.equal(w.document.querySelector('[data-eona-menu]'), null);
});

test('auto-mount via data-src, default root inserted', async () => {
  const w = run({ src: bundle(), attrs: { 'data-src': '/m.jsonld' } }, { fetch: stubFetch(JSON.parse(fixture('eona-x.jsonld'))) });
  await tick();
  assert.ok(w.document.body.firstElementChild.hasAttribute('data-eona-menu-root'));
  assert.ok(w.document.querySelector('[data-eona-menu].is-enhanced'));
  assert.equal(w.document.querySelector('nav').getAttribute('aria-label'), 'Navigation principale');
});

test('auto-mount via inline ld+json and data-target', async () => {
  const html = '<!doctype html><html lang="en"><body><div id="here"></div><script type="application/ld+json" data-eona-menu>' + fixture('eona-x.jsonld') + '</script></body></html>';
  const w = run({ src: bundle(), attrs: { 'data-target': '#here', 'data-lang': 'en' } }, { html });
  await tick();
  assert.ok(w.document.querySelector('#here [data-eona-menu]'));
  assert.equal(w.document.querySelector('nav').getAttribute('aria-label'), 'Main navigation');
});

test('data-current auto', async () => {
  const w = run({ src: bundle(), attrs: { 'data-src': '/m', 'data-current': 'auto' } }, { fetch: stubFetch(model), url: 'https://eona-x.eu/fr/actualites/' });
  await tick();
  assert.equal(w.document.querySelector('a[href="/fr/actualites/"]').getAttribute('aria-current'), 'page');
});

test('--config jsonld inlines the model', async () => {
  build('--config', 'test/fixtures/eona-x.jsonld', '--name', 't');
  const s = readFileSync(join(root, 'dist/menu.t.js'), 'utf8');
  assert.doesNotThrow(() => new Function(s));
  const w = run({ src: s, attrs: {} });
  await tick();
  assert.ok(w.document.querySelector('[data-eona-menu].is-enhanced'));
  assert.equal(w.document.querySelector('nav').getAttribute('aria-label'), 'Navigation principale');
});

test('--config ttl equals jsonld', async (t) => {
  if (!existsSync(join(root, 'lib/ttl.js'))) return t.skip('lib/ttl.js missing');
  build('--config', 'test/fixtures/eona-x.ttl', '--name', 'ttl');
  const a = readFileSync(join(root, 'dist/menu.ttl.js'), 'utf8');
  build('--config', 'test/fixtures/eona-x.jsonld', '--name', 'ld');
  const b = readFileSync(join(root, 'dist/menu.ld.js'), 'utf8');
  assert.equal(a, b);
});

test('size budgets', () => {
  assert.ok(gzipSync(bundle()).length < 8192, 'js ' + gzipSync(bundle()).length);
  const css = gzipSync(readFileSync(join(root, 'menu.css'))).length;
  assert.ok(css < 2048, 'css ' + css);
});
