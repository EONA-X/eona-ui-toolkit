#!/usr/bin/env node
// Bundle src/*.js into one classic script: dist/menu.js (+ dist/menu.<name>.js with --config).
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, extname, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { gzipSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = resolve(here, '..');
const ORDER = ['errors', 'model', 'rdf', 'jsonld', 'render', 'behavior', 'mount', 'auto'];

// Drop spaces around punctuation outside strings and regex literals. Sources never divide,
// so any '/' that opens neither a comment nor a closing delimiter starts a regex.
function compact(line) {
  const out = [];
  let code = '';
  const flush = () => {
    out.push(code.replace(/\s*([{}()[\];,:?=<>&|*!])\s*/g, '$1').replace(/(?<!\+)\s+\+\s+(?!\+)/g, '+').replace(/(?<!-)\s+-\s+(?!-)/g, '-'));
    code = '';
  };
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (c !== '"' && c !== "'" && c !== '/') {
      code += c;
      continue;
    }
    let j = i + 1;
    for (let cls = false; j < line.length; j++) {
      if (line[j] === '\\') j++;
      else if (c === '/' && line[j] === '[') cls = true;
      else if (c === '/' && line[j] === ']') cls = false;
      else if (line[j] === c && !cls) break;
    }
    flush();
    out.push(line.slice(i, j + 1));
    i = j;
  }
  flush();
  return out.join('');
}

function transform(src) {
  const out = [];
  let block = false;
  for (const line of src.split('\n')) {
    const t = line.trim();
    if (block) {
      if (t.endsWith('*/')) block = false;
      continue;
    }
    if (t === '' || t.startsWith('//') || /^import\b/.test(t)) continue;
    if (t.startsWith('/*')) {
      block = !t.endsWith('*/');
      continue;
    }
    out.push(compact(t.replace(/^export /, '')));
  }
  return out.join('\n');
}

function args(argv) {
  const a = { config: null, name: 'eona-x', out: null };
  for (let i = 0; i < argv.length; i++) {
    const k = argv[i];
    if (k === '--config') a.config = argv[++i];
    else if (k === '--name') a.name = argv[++i];
    else if (k === '--out') a.out = argv[++i];
    else throw new Error('Usage: eona-menu-build [--config menu.ttl|menu.json|menu.jsonld] [--name eona-x] [--out dir]');
  }
  return a;
}

async function configModel(file) {
  const path = resolve(process.cwd(), file);
  const text = readFileSync(path, 'utf8');
  if (extname(path) === '.ttl') {
    const { ttlToModel } = await import(pathToFileURL(resolve(pkg, 'lib/ttl.js')).href);
    return ttlToModel(text, { base: pathToFileURL(path).href });
  }
  const { normalize } = await import(pathToFileURL(resolve(pkg, 'src/model.js')).href);
  const { fromJsonLd } = await import(pathToFileURL(resolve(pkg, 'src/jsonld.js')).href);
  const body = JSON.parse(text);
  const ld = Array.isArray(body) || '@context' in body || '@graph' in body;
  return ld ? fromJsonLd(body) : normalize(body);
}

function emit(file, config, body) {
  const json = config === null ? 'null' : JSON.stringify(config).replace(/</g, '\\u003c').replace(/[\u2028\u2029]/g, (c) => '\\u' + c.charCodeAt(0).toString(16));
  const src = 'var __EONA_MENU_CONFIG__ = ' + json + ';\n(function(){"use strict";\n' + body + '\n})();\n';
  new Function(src);
  writeFileSync(file, src);
  console.log(file + ': ' + Buffer.byteLength(src) + ' B, ' + gzipSync(src).length + ' B gzip');
}

const a = args(process.argv.slice(2));
const body = ORDER.map((m) => transform(readFileSync(resolve(pkg, 'src', m + '.js'), 'utf8'))).join('\n');
const dir = a.out ? resolve(process.cwd(), a.out) : resolve(pkg, 'dist');
mkdirSync(dir, { recursive: true });
emit(resolve(dir, 'menu.js'), null, body);
if (a.config) emit(resolve(dir, 'menu.' + a.name + '.js'), await configModel(a.config), body);
