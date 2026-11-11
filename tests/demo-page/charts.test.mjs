// assets/charts-page.js against the live-charts markup (the `live-charts` section of
// src/organisms/components_gallery.rs). The WebAssembly module and `fetch` are stubs:
// what is pinned here is when the script loads, fetches and mounts, and what it hands
// to `mount`. The real module is exercised by tests/bridge-parity/wasm-charts.mjs.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { JSDOM } from 'jsdom';

const here = dirname(fileURLToPath(import.meta.url));
const script = readFileSync(join(here, '../../assets/charts-page.js'), 'utf8');

const BAR = 'samples/bar-airports.jsonld';
const ALT = 'samples/bar-stations.jsonld';

const MARKUP = `
  <div class="chart-switch" role="group" aria-label="Dataset" hidden>
    <button type="button" data-chart-for="live-bar" data-chart-src="${BAR}" aria-pressed="true">Airports</button>
    <button type="button" data-chart-for="live-bar" data-chart-src="${ALT}" aria-pressed="false">Stations</button>
  </div>
  <div id="live-bar" data-cube-chart="BarChart" data-src="${BAR}" style="min-height:240px"></div>
  <div id="live-donut" data-cube-chart="CubeScoreDonut" data-src="samples/donut-completeness.jsonld"
       data-select="Completeness" data-lang="fr" style="min-height:200px"></div>
  <div id="live-metric" data-cube-chart="CubeMetricTile" data-src="samples/metric-records.jsonld"
       data-help="per month"></div>`;

/**
 * A page with the script run in it.
 * `responses` maps a src to a Response-like; a function value is called to get one (for ordering).
 */
function page({ observer = true, loader = 'ok', responses = {} } = {}) {
  const dom = new JSDOM(`<!doctype html><body>${MARKUP}</body>`, {
    url: 'https://example.test/yew/',
    runScripts: 'outside-only',
    pretendToBeVisual: true,
  });
  const { window } = dom;
  const log = { fetches: [], mounts: [], updates: [], loads: 0, destroyed: 0 };

  window.fetch = (url, init) => {
    log.fetches.push({ url, signal: init && init.signal });
    const r = responses[url];
    if (r === 'reject') return Promise.reject(new window.TypeError('network down'));
    const value = typeof r === 'function' ? r() : r;
    if (value === undefined) {
      return Promise.resolve({ ok: true, status: 200, text: () => Promise.resolve(`doc of ${url}`) });
    }
    return Promise.resolve(value);
  };

  window.eonaChartsLoader = () => {
    log.loads += 1;
    if (loader === 'fail') return Promise.reject(new window.Error('wasm refused'));
    return Promise.resolve({
      mount(target, component, props) {
        log.mounts.push({ target, component, props });
        target.innerHTML = props.fetchError
          ? `<div class="chart chart--error" role="alert">${props.fetchError}</div>`
          : `<figure class="chart">${props.jsonld}</figure>`;
        return {
          update(next) {
            log.updates.push(next);
            target.innerHTML = next.fetchError
              ? `<div class="chart chart--error" role="alert">${next.fetchError}</div>`
              : `<figure class="chart">${next.jsonld}</figure>`;
          },
          destroy() {
            log.destroyed += 1;
          },
        };
      },
    });
  };

  const watching = new Map(); // element -> callback entry
  if (observer) {
    window.IntersectionObserver = class {
      constructor(cb, options) {
        this.cb = cb;
        this.options = options;
        this.disconnected = false;
      }
      observe(el) {
        watching.set(el, this);
      }
      unobserve(el) {
        watching.delete(el);
      }
      disconnect() {
        this.disconnected = true;
      }
    };
  }
  window.eval(script);
  const doc = window.document;
  const intersect = (id) => {
    const el = doc.getElementById(id);
    const o = watching.get(el);
    assert.ok(o, `${id} is being observed`);
    o.cb([{ target: el, isIntersecting: true, intersectionRatio: 1 }], o);
  };
  return { window, doc, log, watching, intersect, el: (id) => doc.getElementById(id) };
}

// Objects built inside jsdom have its realm's prototypes; compare them as plain data.
const plain = (o) => JSON.parse(JSON.stringify(o));

// Enough turns for fetch -> text -> loader -> mount to run.
const settle = async () => {
  for (let i = 0; i < 6; i++) await new Promise((r) => setTimeout(r, 0));
};

test('nothing is loaded, fetched or mounted until a chart nears the viewport', async () => {
  const p = page();
  await settle();
  assert.equal(p.log.loads, 0);
  assert.equal(p.log.fetches.length, 0);
  assert.equal(p.log.mounts.length, 0);
  assert.equal(p.watching.size, 3, 'every container is observed');
});

test('a chart that intersects is fetched and mounted; the others stay put', async () => {
  const p = page();
  p.intersect('live-bar');
  await settle();
  assert.deepEqual(p.log.fetches.map((f) => f.url), [BAR]);
  assert.equal(p.log.mounts.length, 1);
  const m = p.log.mounts[0];
  assert.equal(m.target, p.el('live-bar'), 'mounted into the container itself');
  assert.equal(m.component, 'BarChart');
  assert.deepEqual(plain(m.props), { jsonld: `doc of ${BAR}` });
  assert.equal(p.watching.has(p.el('live-bar')), false, 'no longer observed');
  assert.equal(p.watching.size, 2);
  assert.equal(p.el('live-donut').innerHTML, '');
});

test('the module is loaded once for all charts', async () => {
  const p = page();
  p.intersect('live-bar');
  p.intersect('live-donut');
  p.intersect('live-metric');
  await settle();
  assert.equal(p.log.loads, 1);
  assert.equal(p.log.mounts.length, 3);
});

test('data-select, data-help and data-lang become select, help and lang', async () => {
  const p = page();
  p.intersect('live-donut');
  p.intersect('live-metric');
  await settle();
  const by = Object.fromEntries(p.log.mounts.map((m) => [m.component, m.props]));
  assert.equal(by.CubeScoreDonut.select, 'Completeness');
  assert.equal(by.CubeScoreDonut.lang, 'fr');
  assert.equal(by.CubeMetricTile.help, 'per month');
  assert.equal('select' in by.CubeMetricTile, false);
});

test('an HTTP failure is handed to the chart as fetchError and renders chart--error', async () => {
  const p = page({ responses: { [BAR]: { ok: false, status: 404, text: () => Promise.resolve('nope') } } });
  p.intersect('live-bar');
  await settle();
  const { props } = p.log.mounts[0];
  assert.match(props.fetchError, /404/);
  assert.match(props.fetchError, new RegExp(BAR));
  assert.equal('jsonld' in props, false);
  assert.ok(p.el('live-bar').querySelector('.chart--error[role=alert]'));
});

test('a network failure is handed to the chart as fetchError', async () => {
  const p = page({ responses: { [BAR]: 'reject' } });
  p.intersect('live-bar');
  await settle();
  assert.match(p.log.mounts[0].props.fetchError, /network down/);
  assert.ok(p.el('live-bar').querySelector('.chart--error'));
});

test('a module that fails to load leaves an alert in the container, on every chart', async () => {
  const p = page({ loader: 'fail' });
  p.intersect('live-bar');
  p.intersect('live-donut');
  await settle();
  assert.equal(p.log.mounts.length, 0);
  for (const id of ['live-bar', 'live-donut']) {
    const alert = p.el(id).querySelector('.chart--error[role=alert]');
    assert.ok(alert, id);
    assert.match(alert.textContent, /wasm refused/);
  }
});

test('the dataset switcher is unhidden and marks the first dataset pressed', () => {
  const p = page();
  assert.equal(p.doc.querySelector('.chart-switch').hidden, false);
  const pressed = [...p.doc.querySelectorAll('[data-chart-src]')].map((b) => b.getAttribute('aria-pressed'));
  assert.deepEqual(pressed, ['true', 'false']);
});

test('switching the dataset of a mounted chart fetches the new one and updates, without a remount', async () => {
  const p = page();
  p.intersect('live-bar');
  await settle();
  const [first, second] = p.doc.querySelectorAll('[data-chart-src]');
  second.click();
  await settle();
  assert.deepEqual(p.log.fetches.map((f) => f.url), [BAR, ALT]);
  assert.deepEqual(plain(p.log.updates), [{ jsonld: `doc of ${ALT}` }]);
  assert.equal(p.log.mounts.length, 1, 'no second mount');
  assert.equal(p.log.destroyed, 0);
  assert.equal(second.getAttribute('aria-pressed'), 'true');
  assert.equal(first.getAttribute('aria-pressed'), 'false');
  assert.equal(p.el('live-bar').getAttribute('data-src'), ALT);
});

test('switching before the chart is visible only changes what it will load', async () => {
  const p = page();
  p.doc.querySelectorAll('[data-chart-src]')[1].click();
  await settle();
  assert.equal(p.log.fetches.length, 0);
  p.intersect('live-bar');
  await settle();
  assert.deepEqual(p.log.fetches.map((f) => f.url), [ALT]);
  assert.equal(p.log.mounts.length, 1);
});

test('a failed fetch on switch goes to the chart through update', async () => {
  const p = page({ responses: { [ALT]: { ok: false, status: 500, text: () => Promise.resolve('') } } });
  p.intersect('live-bar');
  await settle();
  p.doc.querySelectorAll('[data-chart-src]')[1].click();
  await settle();
  assert.match(p.log.updates[0].fetchError, /500/);
  assert.ok(p.el('live-bar').querySelector('.chart--error'));
});

test('the last click wins: an older response that arrives late is dropped', async () => {
  let releaseAlt;
  const late = new Promise((r) => (releaseAlt = r));
  const p = page({
    responses: { [ALT]: () => late.then(() => ({ ok: true, status: 200, text: () => Promise.resolve('late alt') })) },
  });
  p.intersect('live-bar');
  await settle();
  const [first, second] = p.doc.querySelectorAll('[data-chart-src]');
  second.click(); // ALT: slow
  await settle();
  first.click(); // back to BAR: fast
  await settle();
  releaseAlt();
  await settle();
  assert.deepEqual(plain(p.log.updates), [{ jsonld: `doc of ${BAR}` }], 'only the last request is applied');
  assert.equal(p.log.fetches[1].signal.aborted, true, 'the superseded request was aborted');
  assert.equal(first.getAttribute('aria-pressed'), 'true');
});

test('without IntersectionObserver every chart is mounted at once', async () => {
  const p = page({ observer: false });
  await settle();
  assert.equal(p.log.mounts.length, 3);
});
