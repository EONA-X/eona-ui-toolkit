// Shared by the bridge suites: compile the generated components and derive
// prop sets from the ABI. See parity.mjs for what is compared and why.

import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { transform } from 'esbuild';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRawSnippet } from 'svelte';
import { compile } from 'svelte/compiler';
import { render } from 'svelte/server';

const here = dirname(fileURLToPath(import.meta.url));
export const root = resolve(here, '../..');
const out = join(here, '.out');

// ------------------------------------------------------------------ build

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'react'), { recursive: true });
mkdirSync(join(out, 'svelte'), { recursive: true });

export async function buildReact(name) {
  const src = readFileSync(join(root, 'packages/react/src', `${name}.tsx`), 'utf8');
  const { code } = await transform(src, { loader: 'tsx', jsx: 'automatic', format: 'esm' });
  const file = join(out, 'react', `${name}.mjs`);
  writeFileSync(file, code);
  return (await import(pathToFileURL(file).href))[name];
}

export async function buildSvelte(name) {
  const filename = join(root, 'packages/svelte/src', `${name}.svelte`);
  const { js, warnings } = compile(readFileSync(filename, 'utf8'), { generate: 'server', filename });
  if (warnings.length) throw new Error(`${name}.svelte: ${warnings.map((w) => w.message).join('; ')}`);
  const file = join(out, 'svelte', `${name}.mjs`);
  writeFileSync(file, js.code);
  return (await import(pathToFileURL(file).href)).default;
}

// ------------------------------------------------------------------ props

export const abi = JSON.parse(readFileSync(join(root, 'abi/toolkit.abi.json'), 'utf8'));
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
export function propSets(component) {
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

export function renderReact(C, props) {
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

export function renderSvelte(C, props) {
  const p = {};
  for (const [k, v] of Object.entries(props)) {
    if (v === undefined) continue;
    p[k] = v === CHILD ? createRawSnippet(() => ({ render: () => CHILD_HTML })) : v;
  }
  return render(C, { props: p }).body;
}

