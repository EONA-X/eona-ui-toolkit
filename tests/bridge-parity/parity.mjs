// Render every generated component through both bridges and compare the HTML.
//
// `@eona-x/ui-toolkit-react` is held to Yew's own render by the verify gate
// (`xtask/src/verify.rs`); nothing held `@eona-x/ui-toolkit-svelte` to anything
// but `svelte-check`. Both are spelled from the same `splice` tree, so a
// difference between them is a bug in one spelling — this finds it.
//
// Prop sets come from `abi/toolkit.abi.json`, not from a hand-written fixture:
// one base set, then one variation per prop per value its type admits (every
// union member, `true`/`false`, absent/present, lists of 0, 1 and 2). A prop the
// ABI grows is therefore covered without touching this file.
//
// Usage: node parity.mjs   (after `npm ci`; exits 1 on any difference)

import { mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { transform } from 'esbuild';
import { parseFragment } from 'parse5';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRawSnippet } from 'svelte';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const out = join(here, '.out');

// ------------------------------------------------------------------ build

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'react'), { recursive: true });
mkdirSync(join(out, 'svelte'), { recursive: true });

async function buildReact(name) {
  const src = readFileSync(join(root, 'packages/react/src', `${name}.tsx`), 'utf8');
  const { code } = await transform(src, { loader: 'tsx', jsx: 'automatic', format: 'esm' });
  const file = join(out, 'react', `${name}.mjs`);
  writeFileSync(file, code);
  return (await import(pathToFileURL(file).href))[name];
}

async function buildSvelte(name) {
  const filename = join(root, 'packages/svelte/src', `${name}.svelte`);
  const { js, warnings } = compile(readFileSync(filename, 'utf8'), { generate: 'server', filename });
  if (warnings.length) throw new Error(`${name}.svelte: ${warnings.map((w) => w.message).join('; ')}`);
  const file = join(out, 'svelte', `${name}.mjs`);
  writeFileSync(file, js.code);
  return (await import(pathToFileURL(file).href)).default;
}

// ------------------------------------------------------------------ props

const abi = JSON.parse(readFileSync(join(root, 'abi/toolkit.abi.json'), 'utf8'));
const types = new Map(abi.types.map((t) => [t.name, t]));
const camel = (s) => s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());

// Text a renderer has to escape, and a URL a renderer must not mangle.
const STRINGS = ['Plain', 'Quote " amp & lt < gt >', 'https://example.org/a?b=1&c=2'];
const CHILD = Symbol('child');

/** Every value a TypeScript type from the ABI admits, first one the most ordinary. */
function values(ts, depth = 0) {
  const members = splitTop(ts, '|');
  if (members.length > 1) return members.flatMap((m) => values(m, depth));
  ts = ts.trim();
  if (ts === 'undefined') return [undefined];
  if (ts === 'string') return STRINGS;
  if (ts === 'boolean') return [true, false];
  if (ts === 'ReactNode') return [CHILD];
  if (/^"[^"]*"$/.test(ts)) return [ts.slice(1, -1)];
  if (ts.endsWith('[]')) {
    const item = values(ts.slice(0, -2), depth + 1);
    const pick = (i) => item[i % item.length];
    return [[pick(0)], [], [pick(0), pick(1)]];
  }
  if (ts.startsWith('[') && ts.endsWith(']')) {
    const parts = splitTop(ts.slice(1, -1), ',').map((p) => values(p, depth + 1));
    return [parts.map((v, i) => v[i % v.length])];
  }
  const t = types.get(ts);
  if (t?.kind === 'struct') {
    // Base object, then one variation per field.
    const base = Object.fromEntries(t.fields.map((f) => [camel(f.name), values(f.ts, depth + 1)[0]]));
    const out = [base];
    if (depth === 0) {
      for (const f of t.fields) {
        for (const v of values(f.ts, depth + 1).slice(1)) out.push({ ...base, [camel(f.name)]: v });
      }
    }
    return out;
  }
  throw new Error(`no values for the TS type ${ts}`);
}

function splitTop(s, sep) {
  const parts = [];
  let depth = 0;
  let quote = false;
  let start = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '"') quote = !quote;
    else if (!quote && (c === '[' || c === '(')) depth++;
    else if (!quote && (c === ']' || c === ')')) depth--;
    else if (!quote && depth === 0 && c === sep) {
      parts.push(s.slice(start, i));
      start = i + 1;
    }
  }
  parts.push(s.slice(start));
  return parts.map((p) => p.trim()).filter(Boolean);
}

/** Base prop set, then one-factor variations; `undefined` means "leave it out". */
function propSets(component) {
  const props = component.props.map((p) => ({ name: camel(p.name), vals: values(p.ts) }));
  const base = Object.fromEntries(props.map((p) => [p.name, p.vals[0]]));
  const sets = [{ label: 'base', props: base }];
  for (const p of props) {
    p.vals.slice(1).forEach((v, i) => sets.push({ label: `${p.name}#${i + 1}`, props: { ...base, [p.name]: v } }));
    // A prop with a default may also be omitted.
    if (component.props.find((q) => camel(q.name) === p.name).default !== 'required') {
      const { [p.name]: _, ...rest } = base;
      sets.push({ label: `${p.name}=absent`, props: rest });
    }
  }
  return sets;
}

// ------------------------------------------------------------------ render

const CHILD_HTML = '<em class="child">child</em>';

function renderReact(C, props) {
  const p = {};
  for (const [k, v] of Object.entries(props)) {
    if (v === undefined) continue;
    p[k] = v === CHILD ? createElement('em', { className: 'child' }, 'child') : v;
  }
  // React 19 hoists a `<link rel="preload">` for every `<img>` it renders
  // (`SiteHeader`'s logo). That is React's resource loading, not markup the
  // component wrote, and neither Yew nor Svelte emits it.
  return renderToStaticMarkup(createElement(C, p)).replace(/^(?:<link rel="preload"[^>]*>)+/, '');
}

function renderSvelte(C, props) {
  const p = {};
  for (const [k, v] of Object.entries(props)) {
    if (v === undefined) continue;
    p[k] = v === CHILD ? createRawSnippet(() => ({ render: () => CHILD_HTML })) : v;
  }
  return render(C, { props: p }).body;
}

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
