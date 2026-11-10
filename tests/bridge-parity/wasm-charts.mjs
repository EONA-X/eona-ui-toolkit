// End-to-end check of `<CubeChart>` (`@eona-x/ui-toolkit-svelte/wasm`) against the
// real `wasm/charts` build: fetch, mount, update without remount, error states,
// teardown. `fetch` is stubbed with the toolkit's chart fixtures.
//
// Usage (after `wasm-pack build --target web --release wasm/charts`):
//   node --conditions=browser wasm-charts.mjs

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';

import { transform } from 'esbuild';
import { JSDOM } from 'jsdom';
import { compile } from 'svelte/compiler';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const out = join(here, '.out', 'wasm-charts');
mkdirSync(out, { recursive: true });

// --- a browser, before anything that looks for one is imported.
const dom = new JSDOM('<!doctype html><html><body></body></html>', { pretendToBeVisual: true });
for (const key of Object.getOwnPropertyNames(dom.window)) {
  if (key in globalThis) continue;
  try {
    globalThis[key] = dom.window[key];
  } catch {}
}
globalThis.window = dom.window;
globalThis.document = dom.window.document;

// --- the package's wasm entry, as a consumer's bundler would compile it.
const src = join(root, 'packages/svelte/src/wasm');
const action = await transform(readFileSync(join(src, 'action.ts'), 'utf8'), { loader: 'ts', format: 'esm' });
writeFileSync(join(out, 'action.js'), action.code);
const { js } = compile(readFileSync(join(src, 'CubeChart.svelte'), 'utf8'), {
  generate: 'client',
  filename: 'CubeChart.svelte',
});
writeFileSync(join(out, 'CubeChart.mjs'), js.code.replace(/from '\.\/action'/g, "from './action.js'"));

const { mount, unmount, flushSync } = await import('svelte');
const CubeChart = (await import(pathToFileURL(join(out, 'CubeChart.mjs')).href)).default;
const index = readFileSync(join(src, 'index.ts'), 'utf8');
assert.match(index, /CubeChart/, '/wasm exports CubeChart');
assert.doesNotMatch(index, /wasm\/charts|eona_ui_toolkit_charts/, '/wasm alone does not reference the charts module');

// --- the Rust side.
const pkg = join(root, 'wasm/charts/pkg');
const wasm = await import(pathToFileURL(join(pkg, 'eona_ui_toolkit_charts.js')).href);
wasm.initSync({ module: readFileSync(join(pkg, 'eona_ui_toolkit_charts_bg.wasm')) });

const fixture = (kind, name) => readFileSync(join(root, 'tests/fixtures/cube', kind, `${name}.jsonld`), 'utf8');
const FIRST = fixture('conforming', 'bar-category');
const SECOND = FIRST.replace('Visitors per airport', 'Second dataset');

// --- fetch: a table of responses; a deferred one lets a test order the replies.
const calls = [];
const routes = new Map();
globalThis.fetch = (url, init) => {
  calls.push({ url: String(url), signal: init?.signal });
  const route = routes.get(String(url));
  if (!route) return Promise.resolve(new Response('missing', { status: 404, statusText: 'Not Found' }));
  return typeof route === 'function' ? route() : Promise.resolve(new Response(route, { status: 200 }));
};
const deferred = () => {
  let resolveIt;
  const promise = new Promise((r) => (resolveIt = r));
  return { promise, reply: (body) => resolveIt(new Response(body, { status: 200 })) };
};

const settle = async () => {
  for (let i = 0; i < 8; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};
const unhandled = [];
process.on('unhandledRejection', (e) => unhandled.push(e));

// A parent that owns `src`, so a test can change it the way an app would.
const wrapper = compile(
  `<script>
    import CubeChart from './CubeChart.mjs';
    let { ctl } = $props();
    let src = $state(ctl.src);
    ctl.setSrc = (v) => { src = v; };
  </script>
  <CubeChart module={ctl.module} component={ctl.component} {src} class="host" />`,
  { generate: 'client', filename: 'Wrapper.svelte' },
);
writeFileSync(join(out, 'Wrapper.mjs'), wrapper.js.code);
const Wrapper = (await import(pathToFileURL(join(out, 'Wrapper.mjs')).href)).default;

const title = (el) => el.querySelector('svg title')?.textContent;
const module = () => Promise.resolve(wasm);
const hostOf = (target) => target.querySelector('div.host[data-yew-component]');

// 1. src = fixture: fetched, parsed in Rust, rendered with its accessible title.
routes.set('/data/a.jsonld', FIRST);
routes.set('/data/b.jsonld', SECOND);
const target = document.createElement('main');
document.body.append(target);
const ctl = { module, component: 'BarChart', src: '/data/a.jsonld' };
const app = mount(Wrapper, { target, props: { ctl } });
await settle();
assert.equal(calls.length, 1, 'one fetch');
assert.equal(calls[0].url, '/data/a.jsonld');
const host = hostOf(target);
assert.ok(host, 'the host element is rendered');
assert.equal(host.dataset.yewComponent, 'BarChart');
assert.equal(title(host), 'Visitors per airport', 'svg <title> comes from rdfs:label');
assert.ok(host.querySelector('figure.chart--bar'), 'a bar figure');

// 2. A changed src re-fetches and updates the same Yew app: same host, same figure node.
const figure = host.querySelector('figure');
ctl.setSrc('/data/b.jsonld');
await settle();
assert.equal(calls.length, 2, 'a second fetch');
assert.equal(calls[0].signal.aborted, true, 'the previous fetch was aborted');
assert.equal(hostOf(target), host, 'the host element was kept');
assert.equal(host.querySelector('figure'), figure, 'the Yew app was updated, not remounted');
assert.equal(title(host), 'Second dataset');

// 3. A stale response after a src change is ignored.
const slow = deferred();
routes.set('/data/slow.jsonld', () => slow.promise);
ctl.setSrc('/data/slow.jsonld');
await settle();
ctl.setSrc('/data/a.jsonld');
await settle();
assert.equal(title(host), 'Visitors per airport', 'the newest src is shown');
slow.reply(SECOND);
await settle();
assert.equal(title(host), 'Visitors per airport', 'the late reply for the old src changed nothing');

// 4. A 404 is a chart--error carrying the status; the app survives and recovers.
ctl.setSrc('/data/missing.jsonld');
await settle();
const alert = host.querySelector('.chart--error[role="alert"]');
assert.ok(alert, 'a 404 renders chart--error');
assert.match(alert.textContent, /404/, 'the status is in the message');
assert.match(alert.textContent, /\/data\/missing\.jsonld/, 'and the url');
ctl.setSrc('/data/b.jsonld');
await settle();
assert.equal(host.querySelector('.chart--error'), null, 'the error clears on the next good load');
assert.equal(title(host), 'Second dataset');

// 5. A document the profile rejects: chart--error, no unhandled rejection.
routes.set('/data/bad.jsonld', fixture('violating', 'value-nan'));
ctl.setSrc('/data/bad.jsonld');
await settle();
assert.ok(host.querySelector('.chart--error[role="alert"]'), 'a violating document renders chart--error');
// A network failure too.
routes.set('/data/down.jsonld', () => Promise.reject(new TypeError('network down')));
ctl.setSrc('/data/down.jsonld');
await settle();
assert.match(host.querySelector('.chart--error')?.textContent ?? '', /network down/, 'a rejected fetch is shown');
assert.deepEqual(unhandled, [], 'no unhandled rejection');

// 6. An unknown component is reported, not thrown into the host app.
const errors = [];
const original = console.error;
console.error = (...a) => errors.push(a.join(' '));
const t2 = document.createElement('main');
document.body.append(t2);
const bogus = mount(Wrapper, { target: t2, props: { ctl: { module, component: 'PieChart', src: '/data/a.jsonld' } } });
await settle();
console.error = original;
assert.ok(errors.some((e) => e.includes('PieChart')), 'the failure was reported');
unmount(bogus);

// 7. destroy removes the rendered nodes.
unmount(app);
await settle();
assert.equal(target.querySelector('figure'), null, 'unmount removed the chart');
assert.equal(target.innerHTML.includes('Second dataset'), false, 'nothing is left behind');
assert.deepEqual(unhandled, [], 'still no unhandled rejection');

console.log('wasm charts: CubeChart against wasm/charts — 7 checks passed');
