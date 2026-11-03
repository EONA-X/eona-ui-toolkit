import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { loadMenu, mountMenu } from '../src/mount.js';
import { renderMenuHtml } from '../src/render.js';
import { MenuError } from '../src/errors.js';

const rd = (f) => readFileSync(new URL('./fixtures/' + f, import.meta.url), 'utf8');
const model = JSON.parse(rd('eona-x.model.json'));
const ld = JSON.parse(rd('eona-x.jsonld'));
const stub = (body, status = 200) => {
  const calls = [];
  const f = async (url, init) => { calls.push({ url, init }); return { ok: status >= 200 && status < 300, status, json: async () => body }; };
  f.calls = calls;
  return f;
};
const win = (html = '<div id="t"></div>') => {
  const w = new JSDOM('<!doctype html><body>' + html).window;
  w.matchMedia = () => ({ matches: true, addEventListener() {}, removeEventListener() {} });
  globalThis.window = w; globalThis.document = w.document;
  return w;
};

test('loadMenu jsonld and plain json', async () => {
  const f = stub(ld);
  assert.deepStrictEqual(await loadMenu('/m.jsonld', { fetch: f }), model);
  assert.match(f.calls[0].init.headers.Accept, /application\/ld\+json/);
  assert.equal(f.calls[0].init.credentials, 'same-origin');
  assert.deepStrictEqual(await loadMenu('/m.json', { fetch: stub(model) }), model);
});

test('loadMenu non-2xx', async () => {
  await assert.rejects(loadMenu('/x', { fetch: stub({}, 404) }), (e) => e instanceof MenuError && /HTTP 404 loading \/x/.test(e.message));
});

test('mountMenu renders and enhances', async () => {
  const w = win();
  const c = await mountMenu('#t', model, { window: w });
  const root = w.document.querySelector('#t [data-eona-menu]');
  assert.ok(root);
  assert.ok(root.classList.contains('is-enhanced'));
  assert.equal(typeof c.destroy, 'function');
  await mountMenu(w.document.getElementById('t'), ld, { window: w });
  assert.equal(w.document.querySelectorAll('[data-eona-menu]').length, 1);
});

test('mountMenu string source uses fetch', async () => {
  const w = win();
  const f = stub(ld);
  await mountMenu('#t', '/m.jsonld', { window: w, fetch: f });
  assert.equal(f.calls.length, 1);
  assert.ok(w.document.querySelector('[data-eona-menu]'));
});

test('server-rendered: enhance only', async () => {
  const html = renderMenuHtml(model);
  const w = win('<div id="t">' + html + '</div>');
  const before = w.document.getElementById('t').innerHTML;
  await mountMenu('#t', undefined, { window: w });
  assert.equal(w.document.getElementById('t').innerHTML.replace(' is-enhanced', ''), before);
  assert.ok(w.document.querySelector('[data-eona-menu]').classList.contains('is-enhanced'));
});

test('errors', async () => {
  const w = win();
  const c = console.error; console.error = () => {};
  try {
    await assert.rejects(mountMenu('#nope', model, { window: w }), MenuError);
    await assert.rejects(mountMenu('#t', { items: [{ label: 'x' }] }, { window: w }), MenuError);
    assert.equal(w.document.getElementById('t').innerHTML, '');
  } finally { console.error = c; }
});
