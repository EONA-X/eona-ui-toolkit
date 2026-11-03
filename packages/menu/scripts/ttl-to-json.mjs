#!/usr/bin/env node
// eona-menu-ttl: Turtle menu description -> normalized JSON model.
import { readFileSync, writeFileSync, realpathSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { ttlToModel } from '../lib/ttl.js';
import { TurtleError } from '../lib/turtle.js';
import { MenuError } from '../src/errors.js';

const USAGE = 'Usage: eona-menu-ttl <input.ttl|-> [-o out.json] [--base IRI] [--menu IRI] [--compact]\n';

const readStdin = async () => {
  let s = '';
  process.stdin.setEncoding('utf8');
  for await (const chunk of process.stdin) s += chunk;
  return s;
};

/**
 * @param {string[]} argv
 * @param {{stdin?: () => Promise<string>, stdout?: {write(s:string):any}, stderr?: {write(s:string):any}}} [io]
 * @returns {Promise<number>}
 */
export async function main(argv, io = {}) {
  const stdin = io.stdin ?? readStdin;
  const stdout = io.stdout ?? process.stdout;
  const stderr = io.stderr ?? process.stderr;
  const usage = (msg) => { stderr.write((msg ? msg + '\n' : '') + USAGE); return 2; };

  let input = null;
  let out = null;
  let base;
  let menu;
  let compact = false;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '-o' || a === '--base' || a === '--menu') {
      const v = argv[++i];
      if (v === undefined) return usage(`missing value for ${a}`);
      if (a === '-o') out = v; else if (a === '--base') base = v; else menu = v;
    } else if (a === '--compact') compact = true;
    else if (a === '-h' || a === '--help') { stdout.write(USAGE); return 0; }
    else if (a !== '-' && a.startsWith('-')) return usage(`unknown option ${a}`);
    else if (input === null) input = a;
    else return usage('too many inputs');
  }
  if (input === null) return usage();

  const name = input === '-' ? '<stdin>' : input;
  try {
    let text;
    if (input === '-') text = await stdin();
    else {
      try { text = readFileSync(input, 'utf8'); }
      catch (e) { stderr.write(`${name}: ${e.message}\n`); return 1; }
      base ??= 'file://' + resolve(input);
    }
    const model = ttlToModel(text, { base, menu });
    const json = (compact ? JSON.stringify(model) : JSON.stringify(model, null, 2)) + '\n';
    if (out) writeFileSync(out, json); else stdout.write(json);
    return 0;
  } catch (e) {
    if (e instanceof TurtleError) stderr.write(`${name}:${e.line}:${e.col}: ${e.message}\n`);
    else if (e instanceof MenuError && e.errors.length) {
      for (const x of e.errors) stderr.write(`${name}: ${x.path}: ${x.message}\n`);
    } else if (e instanceof MenuError) stderr.write(`${name}: ${e.message}\n`);
    else throw e;
    return 1;
  }
}

const entry = process.argv[1];
if (entry && import.meta.url === pathToFileURL(realpathSync(entry)).href) {
  main(process.argv.slice(2)).then((c) => { process.exitCode = c; });
}
