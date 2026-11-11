// assets/demo-page.js against the CodeTabs markup (src/molecules/code_tabs.rs).
// tests/demo_page.rs pins the markup from the Rust side; this pins the script.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { JSDOM } from 'jsdom';

const here = dirname(fileURLToPath(import.meta.url));
const script = readFileSync(join(here, '../../assets/demo-page.js'), 'utf8');

const STACKS = [
  ['yew', 'YEW'],
  ['svelte', 'SVELTE'],
  ['react', 'REACT'],
];

// One block, as CodeTabs renders it. `absent` maps a stack to the note that replaces its code.
function block(id, { absent = {} } = {}) {
  const tabs = STACKS.map(
    ([s, label], i) =>
      `<button type="button" role="tab" class="code-tabs-tab" id="${id}-tab-${s}" aria-controls="${id}-panel-${s}" aria-selected="${i === 0}" tabindex="${i === 0 ? 0 : -1}" data-stack="${s}">${label}</button>`,
  ).join('');
  const panels = STACKS.map(([s], i) => {
    const hidden = i === 0 ? '' : ' hidden';
    const body = absent[s]
      ? `<div class="snippet-absent"><p>${absent[s]}</p></div>`
      : `<pre class="code-block-pre"><code class="language-${s}">code of ${id} ${s}</code></pre>`;
    return `<div role="tabpanel" id="${id}-panel-${s}" aria-labelledby="${id}-tab-${s}" data-stack="${s}"${hidden}>${body}</div>`;
  }).join('');
  return `<div class="code-block code-tabs"><div class="code-block-bar"><div class="code-tabs-list" role="tablist" aria-label="Stack" hidden>${tabs}</div><span class="code-block-lang code-tabs-lang">rust</span><button type="button" class="code-block-copy" aria-label="Copy this snippet">Copy</button></div>${panels}</div>`;
}

function page({ storage = {}, throwing = false, blocks } = {}) {
  const dom = new JSDOM(`<!doctype html><body>${blocks ?? block('a') + block('b', { absent: { react: 'no React version' } })}</body>`, {
    url: 'https://example.test/',
    runScripts: 'outside-only',
    pretendToBeVisual: true,
  });
  const { window } = dom;
  if (throwing) {
    Object.defineProperty(window, 'localStorage', {
      get() {
        throw new Error('storage is blocked');
      },
    });
  } else {
    for (const [k, v] of Object.entries(storage)) window.localStorage.setItem(k, v);
  }
  const copied = [];
  Object.defineProperty(window.navigator, 'clipboard', {
    value: { writeText: (t) => (copied.push(t), Promise.resolve()) },
    configurable: true,
  });
  window.eval(script);
  const $ = (sel, root = window.document) => root.querySelector(sel);
  const tab = (id, stack) => window.document.getElementById(`${id}-tab-${stack}`);
  const panel = (id, stack) => window.document.getElementById(`${id}-panel-${stack}`);
  const key = (el, k) =>
    el.dispatchEvent(new window.KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
  const selected = (id) => STACKS.map(([s]) => s).filter((s) => tab(id, s).getAttribute('aria-selected') === 'true');
  const visible = (id) => STACKS.map(([s]) => s).filter((s) => !panel(id, s).hidden);
  return { window, $, tab, panel, key, selected, visible, copied };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

test('the script unhides the tablists and shows Yew by default', () => {
  const p = page();
  for (const list of p.window.document.querySelectorAll('[role=tablist]')) assert.equal(list.hidden, false);
  assert.deepEqual(p.selected('a'), ['yew']);
  assert.deepEqual(p.visible('a'), ['yew']);
});

test('clicking SVELTE on one block switches every block', () => {
  const p = page();
  p.tab('a', 'svelte').click();
  for (const id of ['a', 'b']) {
    assert.deepEqual(p.selected(id), ['svelte'], id);
    assert.deepEqual(p.visible(id), ['svelte'], id);
    assert.equal(p.tab(id, 'svelte').getAttribute('tabindex'), '0');
    assert.equal(p.tab(id, 'yew').getAttribute('tabindex'), '-1');
  }
});

test('ArrowRight, ArrowLeft, Home and End move focus and selection', () => {
  const p = page();
  p.tab('a', 'yew').focus();
  p.key(p.tab('a', 'yew'), 'ArrowRight');
  assert.deepEqual(p.selected('a'), ['svelte']);
  assert.equal(p.window.document.activeElement, p.tab('a', 'svelte'));
  p.key(p.tab('a', 'svelte'), 'End');
  assert.deepEqual(p.selected('a'), ['react']);
  assert.equal(p.window.document.activeElement, p.tab('a', 'react'));
  p.key(p.tab('a', 'react'), 'ArrowRight');
  assert.deepEqual(p.selected('a'), ['yew'], 'wraps around');
  p.key(p.tab('a', 'yew'), 'ArrowLeft');
  assert.deepEqual(p.selected('a'), ['react'], 'wraps around backwards');
  p.key(p.tab('a', 'react'), 'Home');
  assert.deepEqual(p.selected('a'), ['yew']);
  assert.equal(p.window.document.activeElement, p.tab('a', 'yew'));
  // The other block follows the selection but does not steal focus.
  assert.deepEqual(p.selected('b'), ['yew']);
});

test('the choice is remembered under eona-ui-toolkit:stack and restored', () => {
  const p = page();
  p.tab('a', 'react').click();
  assert.equal(p.window.localStorage.getItem('eona-ui-toolkit:stack'), 'react');

  const q = page({ storage: { 'eona-ui-toolkit:stack': 'svelte' } });
  assert.deepEqual(q.selected('a'), ['svelte']);
  assert.deepEqual(q.visible('b'), ['svelte']);

  const bogus = page({ storage: { 'eona-ui-toolkit:stack': 'cobol' } });
  assert.deepEqual(bogus.selected('a'), ['yew'], 'an unknown value falls back to Yew');
});

test('a localStorage that throws leaves the tabs working, defaulting to Yew', () => {
  const p = page({ throwing: true });
  assert.deepEqual(p.selected('a'), ['yew']);
  p.tab('a', 'svelte').click();
  assert.deepEqual(p.selected('b'), ['svelte']);
});

test('Copy writes the active panel text, and is disabled on an Unavailable panel', async () => {
  const p = page();
  const copy = (id) => p.window.document.querySelectorAll('.code-block-copy')[id === 'a' ? 0 : 1];

  copy('a').click();
  await tick();
  assert.deepEqual(p.copied, ['code of a yew']);

  p.tab('a', 'svelte').click();
  copy('a').click();
  await tick();
  assert.deepEqual(p.copied, ['code of a yew', 'code of a svelte']);

  p.tab('a', 'react').click();
  // Block b's React panel is a note: nothing to copy.
  assert.equal(copy('b').getAttribute('aria-disabled'), 'true');
  assert.equal(copy('a').hasAttribute('aria-disabled'), false);
  copy('b').click();
  await tick();
  assert.equal(p.copied.length, 2, 'a disabled Copy writes nothing');

  p.tab('a', 'yew').click();
  assert.equal(copy('b').hasAttribute('aria-disabled'), false);
});

test('a plain CodeBlock still copies its own code', async () => {
  const blocks = `<div class="code-block"><div class="code-block-bar"><span class="code-block-lang">rust</span><button type="button" class="code-block-copy">Copy</button></div><pre class="code-block-pre"><code class="language-rust">plain</code></pre></div>`;
  const p = page({ blocks });
  p.$('.code-block-copy').click();
  await tick();
  assert.deepEqual(p.copied, ['plain']);
});
