// End-to-end check of `@eona-x/ui-toolkit-svelte/wasm` against a real Yew build.
//
// Mounts `<YewMount>` (compiled for the client) in jsdom with the module
// `examples/svelte-wasm` builds, and checks the contract both sides rely on:
// the component renders, Yew state survives a props update through `use:yew`,
// an unknown component name is reported rather than thrown into Svelte, and
// unmounting removes what Yew rendered.
//
// Usage (after `wasm-pack build --target web examples/svelte-wasm`):
//   node --conditions=browser wasm.mjs

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';

import { transform } from 'esbuild';
import { JSDOM } from 'jsdom';
import { compile } from 'svelte/compiler';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const out = join(here, '.out', 'wasm');
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
const mountSrc = readFileSync(join(src, 'YewMount.svelte'), 'utf8');
const { js } = compile(mountSrc, { generate: 'client', filename: 'YewMount.svelte' });
writeFileSync(join(out, 'YewMount.mjs'), js.code.replace(/from '\.\/action'/g, "from './action.js'"));

const { mount, unmount, flushSync } = await import('svelte');
const { yew } = await import(pathToFileURL(join(out, 'action.js')).href);
const YewMount = (await import(pathToFileURL(join(out, 'YewMount.mjs')).href)).default;

// --- the Rust side.
const pkg = join(root, 'examples/svelte-wasm/pkg');
const wasm = await import(pathToFileURL(join(pkg, 'eona_ui_toolkit_svelte_wasm_example.js')).href);
wasm.initSync({ module: readFileSync(join(pkg, 'eona_ui_toolkit_svelte_wasm_example_bg.wasm')) });

const settle = async () => {
  for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

// 1. <YewMount> renders a Yew component, from a lazily-resolved module.
const target = document.createElement('main');
document.body.append(target);
const app = mount(YewMount, {
  target,
  props: { module: () => Promise.resolve(wasm), component: 'Counter', props: { start: 3, label: 'Clicks' }, class: 'host' },
});
await settle();
const host = target.querySelector('div.host[data-yew-component="Counter"]');
assert.ok(host, 'YewMount renders its host element');
assert.equal(host.querySelector('button')?.textContent, 'Clicks 3', 'Yew rendered the Counter');

// 2. Yew owns its events and state.
host.querySelector('button').dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
await settle();
assert.equal(host.querySelector('button').textContent, 'Clicks 4', 'a click reached the Yew callback');

// 3. Unmounting the Svelte component destroys the Yew app.
unmount(app);
await settle();
assert.equal(target.innerHTML.includes('Clicks'), false, 'unmount removed the Yew render');

// 4. The action: update keeps state, a component change remounts, destroy is clean.
const el = document.createElement('div');
document.body.append(el);
const act = yew(el, { module: wasm, component: 'Counter', props: { start: 0, label: 'n' } });
await settle();
el.querySelector('button').dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
await settle();
act.update({ module: wasm, component: 'Counter', props: { start: 0, label: 'm' } });
await settle();
assert.equal(el.querySelector('button').textContent, 'm 1', 'update re-rendered with new props and kept state');
act.update({ module: wasm, component: 'Button', props: { label: 'Go', variant: 'ghost' } });
await settle();
// Attribute order is the renderer's business (Yew's CSR writes `type` first).
const b = el.querySelector('button');
assert.deepEqual(
  [el.children.length, b?.className, b?.getAttribute('type'), b?.textContent],
  [1, 'btn btn-ghost', 'button', 'Go'],
  'a component change remounts; markup is the toolkit Button',
);
act.destroy();
await settle();
assert.equal(el.innerHTML, '', 'destroy removed the Yew render');

// 5. A name the module does not know is logged, not thrown into the host app.
const errors = [];
const original = console.error;
console.error = (...a) => errors.push(a.join(' '));
const bad = document.createElement('div');
yew(bad, { module: wasm, component: 'Nope', props: {} });
await settle();
console.error = original;
assert.equal(bad.innerHTML, '', 'nothing was mounted');
assert.ok(errors.some((e) => e.includes('Nope')), 'the failure was reported');

console.log('wasm bridge: YewMount + use:yew against examples/svelte-wasm — 5 checks passed');
