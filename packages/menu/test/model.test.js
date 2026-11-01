import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalize, validate, safeHref, pickLabel } from '../src/model.js';
import { MenuError } from '../src/errors.js';

const fixture = JSON.parse(readFileSync(new URL('./fixtures/eona-x.model.json', import.meta.url), 'utf8'));
const leaf = (extra = {}) => ({ label: 'A', href: '/a', ...extra });

test('safeHref allows and rejects', () => {
  for (const h of ['/x', '#a', 'https://x', 'mailto:a@b', 'tel:+1', '//cdn', '?q', 'x']) assert.equal(safeHref(h), h);
  for (const h of ['javascript:alert(1)', ' JAVA\tSCRIPT:x', 'data:text/html,x', 'vbscript:x', 'file:///x']) assert.equal(safeHref(h), null);
  assert.equal(safeHref('  /x '), '/x');
});

test('pickLabel order', () => {
  assert.equal(pickLabel({ fr: 'F', en: 'E' }, 'en', 'fr'), 'E');
  assert.equal(pickLabel({ fr: 'F', en: 'E' }, 'fr-CA', 'en'), 'F');
  assert.equal(pickLabel({ fr: 'F', en: 'E' }, 'de', 'en'), 'E');
  assert.equal(pickLabel({ fr: 'F', '': 'N' }, 'de', 'en'), 'N');
  assert.equal(pickLabel({ fr: 'F', es: 'S' }, 'de', 'en'), 'F');
});

test('normalize defaults', () => {
  const m = normalize({ items: [leaf()] });
  assert.equal(m.version, 1);
  assert.equal(m.lang, 'en');
  assert.deepEqual(m.ariaLabel, { '': 'Main' });
  assert.deepEqual(m.options, { breakpoint: 1024, collapseOffset: 200 });
  assert.equal(m.logo, null);
  assert.deepEqual(m.actions, []);
  assert.equal(m.items[0].current, false);
  assert.equal(m.items[0].external, false);
  const l = normalize({ logo: { href: '/', src: '/s.svg' }, items: [] }).logo;
  assert.equal(l.srcCompact, '/s.svg');
  assert.deepEqual(l.alt, { '': 'Home' });
  assert.equal(l.width, null);
});

test('string label and lowercased keys', () => {
  const m = normalize({ items: [leaf({ label: 'Hi' }), leaf({ label: { 'FR-CA': 'x' } })] });
  assert.deepEqual(m.items[0].label, { '': 'Hi' });
  assert.deepEqual(m.items[1].label, { 'fr-ca': 'x' });
});

test('positional ids, input ids ignored', () => {
  const m = normalize({ items: [leaf({ id: 'zz' }), { label: 'P', children: [leaf({ id: 'q' })] }], actions: [{ href: '/x', label: 'L' }, { href: '/y', label: 'M' }] });
  assert.equal(m.items[0].id, 'i0');
  assert.equal(m.items[1].children[0].id, 'i1-0');
  assert.deepEqual(m.actions.map((a) => a.id), ['a0', 'a1']);
  assert.equal(m.actions[0].kind, 'link');
});

const errPaths = (input) => {
  try { normalize(input); } catch (e) { assert.ok(e instanceof MenuError); return e.errors; }
  return [];
};

test('validate errors', () => {
  const deep = { label: 'a', children: [{ label: 'b', children: [leaf()] }] };
  assert.ok(errPaths({ items: [deep] }).some((e) => e.path === 'items[0].children[0].children'));
  assert.ok(errPaths({ items: [{ label: 'x' }] }).some((e) => e.path === 'items[0].href'));
  assert.ok(errPaths({ items: [{ label: '', href: '/a' }] }).some((e) => e.path === 'items[0].label'));
  assert.deepEqual(errPaths({ items: [leaf({ href: 'javascript:alert(1)' })] })[0].path, 'items[0].href');
  assert.ok(errPaths({ items: [leaf({ children: [leaf({ href: 'data:x' })] })] }).some((e) => e.path === 'items[0].children[0].href' && /unsafe href/.test(e.message)));
  assert.ok(errPaths({ actions: [{ kind: 'language', alternates: [] }] }).some((e) => e.path === 'actions[0].alternates'));
  assert.ok(errPaths({ actions: [{ kind: 'wat' }] }).some((e) => e.path === 'actions[0].kind'));
  assert.ok(errPaths({ actions: [{ href: '/x', label: 'x', variant: 'red' }] }).some((e) => e.path === 'actions[0].variant'));
  assert.ok(errPaths({ version: 2 }).some((e) => e.path === 'version'));
  assert.ok(errPaths({ options: { breakpoint: 0 } }).some((e) => e.path === 'options.breakpoint'));
  assert.ok(errPaths({ logo: { href: '/', src: 'javascript:x' } }).some((e) => e.path === 'logo.src'));
});

test('validate on a good model is empty', () => {
  assert.deepEqual(validate(fixture), []);
});

test('idempotent and fixture stable', () => {
  const once = normalize(fixture);
  assert.deepStrictEqual(once, fixture);
  assert.deepStrictEqual(normalize(once), once);
});
