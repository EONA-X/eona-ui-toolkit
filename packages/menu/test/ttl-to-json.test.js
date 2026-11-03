import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { main } from '../scripts/ttl-to-json.mjs';

const fx = (n) => fileURLToPath(new URL('./fixtures/' + n, import.meta.url));
const expected = JSON.parse(readFileSync(fx('eona-x.model.json'), 'utf8'));
const sink = () => { let s = ''; return { write: (x) => { s += x; }, get text() { return s; } }; };
const run = async (argv, stdin = '') => {
  const out = sink(); const errs = sink();
  const code = await main(argv, { stdin: async () => stdin, stdout: out, stderr: errs });
  return { code, out: out.text, err: errs.text };
};

test('20 writes JSON to stdout', async () => {
  const r = await run([fx('eona-x.ttl')]);
  assert.equal(r.code, 0);
  assert.deepStrictEqual(JSON.parse(r.out), expected);
  assert.ok(r.out.endsWith('\n'));
  assert.ok(r.out.includes('\n  "version"'));
});

test('21 -o and --compact', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'menu-'));
  const o = join(dir, 'out.json');
  const r = await run([fx('eona-x.ttl'), '-o', o, '--compact']);
  assert.equal(r.code, 0);
  assert.equal(r.out, '');
  const txt = readFileSync(o, 'utf8');
  assert.equal(txt.trim().split('\n').length, 1);
  assert.deepStrictEqual(JSON.parse(txt), expected);
});

test('22 stdin via -', async () => {
  const r = await run(['-'], readFileSync(fx('eona-x.ttl'), 'utf8'));
  assert.equal(r.code, 0);
  assert.deepStrictEqual(JSON.parse(r.out), expected);
});

test('22b --menu and --base', async () => {
  const r = await run([fx('eona-x.ttl'), '--menu', 'https://eona-x.eu/menu/main', '--base', 'http://x/']);
  assert.equal(r.code, 0);
});

test('23 syntax error returns 1 with file:line:col', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'menu-'));
  const f = join(dir, 'eona-x-bad.ttl');
  writeFileSync(f, '<s> <p> <o>\n<s2> <p> <o> .\n');
  const r = await run([f]);
  assert.equal(r.code, 1);
  assert.match(r.err, /eona-x-bad\.ttl:\d+:\d+: /);
  assert.equal(r.out, '');
});

test('23b validation errors one per line', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'menu-'));
  const f = join(dir, 'v.ttl');
  writeFileSync(f, '@prefix menu: <https://eona-x.eu/ns/menu#> .\n<urn:m> a menu:Menu ; menu:items ( [ menu:label "A" ; menu:href "javascript:x" ] ) .\n');
  const r = await run([f]);
  assert.equal(r.code, 1);
  assert.match(r.err, /v\.ttl: items\[0\]\.href: /);
});

test('24 usage error returns 2', async () => {
  const r = await run([]);
  assert.equal(r.code, 2);
  assert.match(r.err, /Usage:/);
  assert.equal((await run(['x.ttl', '--nope'])).code, 2);
  assert.equal((await run(['x.ttl', '-o'])).code, 2);
});

test('24b unreadable input returns 1', async () => {
  const r = await run(['/nonexistent/x.ttl']);
  assert.equal(r.code, 1);
});
