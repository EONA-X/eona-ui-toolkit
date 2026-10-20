// Render every generated component through both bridges and compare the HTML.
//
// `@eona-x/ui-toolkit-react` is held to Yew's own render by the verify gate
// (`xtask/src/verify.rs`); nothing held `@eona-x/ui-toolkit-svelte` to anything
// but `svelte-check`. Both are spelled from the same `splice` tree, so a
// difference between them is a bug in one spelling — this finds it.
//
// Prop sets come from `abi/toolkit.abi.json` (lib.mjs), not from a hand-written
// fixture: one base set, then one variation per prop per value its type admits.
//
// Usage: node parity.mjs   (after `npm ci`; exits 1 on any difference)

import { readdirSync } from 'node:fs';
import { join } from 'node:path';

import { parseFragment } from 'parse5';

import { abi, buildReact, buildSvelte, propSets, renderReact, renderSvelte, root } from './lib.mjs';

// ---------------------------------------------------------------- compare

/**
 * A renderer-neutral form of an HTML fragment: comments dropped (Svelte's SSR
 * writes hydration markers), adjacent text merged, attributes sorted, `style`
 * parsed into declarations (React writes `a:b`, Svelte `a: b`).
 *
 * A declaration with no `:` is dropped, and a `style` left with none is treated
 * as absent. That is what a browser does with it, and it is where the two ports
 * legitimately differ: `LogoDownloadCard`'s `previewStyle` is raw CSS text,
 * which Svelte writes verbatim as Yew does, while React has to parse it into an
 * object (`cssText`) and so cannot carry a declaration that does not parse.
 */
function canon(html) {
  const lines = [];
  const walk = (node, depth) => {
    let text = '';
    const flush = () => {
      if (text !== '') lines.push(`${'  '.repeat(depth)}#text ${JSON.stringify(text)}`);
      text = '';
    };
    for (const n of node.childNodes ?? []) {
      if (n.nodeName === '#comment') continue;
      if (n.nodeName === '#text') {
        text += n.value;
        continue;
      }
      flush();
      const attrs = n.attrs
        .filter(({ name, value }) => name !== 'style' || style(value) !== '[]')
        .map(({ name, value }) => (name === 'style' ? `style=${style(value)}` : `${name}=${JSON.stringify(value)}`))
        .sort();
      lines.push(`${'  '.repeat(depth)}<${n.nodeName}${attrs.length ? ' ' + attrs.join(' ') : ''}>`);
      walk(n.content ?? n, depth + 1);
    }
    flush();
  };
  walk(parseFragment(html), 0);
  return lines.join('\n');
}

function style(css) {
  return JSON.stringify(
    css
      .split(';')
      .map((d) => d.trim())
      .filter((d) => d.includes(':'))
      .map((d) => {
        const i = d.indexOf(':');
        return `${d.slice(0, i).trim().toLowerCase()}:${d.slice(i + 1).trim()}`;
      }),
  );
}

// ------------------------------------------------------------------ main

const generated = readdirSync(join(root, 'packages/svelte/src'))
  .filter((f) => f.endsWith('.svelte'))
  .map((f) => f.slice(0, -'.svelte'.length));

let checked = 0;
const failures = [];
for (const component of abi.components.filter((c) => generated.includes(c.name))) {
  const [R, S] = await Promise.all([buildReact(component.name), buildSvelte(component.name)]);
  for (const { label, props } of propSets(component)) {
    checked++;
    let react;
    let svelte;
    try {
      react = canon(renderReact(R, props));
      svelte = canon(renderSvelte(S, props));
    } catch (err) {
      failures.push({ name: component.name, label, detail: String(err?.stack ?? err) });
      continue;
    }
    if (react !== svelte) failures.push({ name: component.name, label, detail: diff(react, svelte) });
  }
}

function diff(a, b) {
  const x = a.split('\n');
  const y = b.split('\n');
  const i = x.findIndex((l, k) => l !== y[k]);
  const at = i === -1 ? Math.min(x.length, y.length) : i;
  return [`first difference at line ${at + 1}:`, `  react : ${x[at] ?? '(end)'}`, `  svelte: ${y[at] ?? '(end)'}`].join('\n');
}

const missing = generated.filter((n) => !abi.components.some((c) => c.name === n));
console.log(`bridge parity: ${generated.length} components, ${checked} prop sets rendered through React and Svelte`);
for (const n of missing) failures.push({ name: n, label: '-', detail: 'a .svelte file the ABI does not know' });
for (const f of failures) console.log(`\nFAIL ${f.name} [${f.label}]\n${f.detail}`);
if (failures.length) {
  console.log(`\n${failures.length} difference(s)`);
  process.exit(1);
}
console.log('identical');
