// One regression test per defect fixed during review.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { renderMenuHtml } from '../src/render.js';
import { enhance } from '../src/behavior.js';
import { mountMenu } from '../src/mount.js';
import { normalize, validate } from '../src/model.js';
import { jsonldToTriples, MENU_CONTEXT_URL, fromJsonLd } from '../src/jsonld.js';
import { parseTurtle, TurtleError } from '../lib/turtle.js';

const fixture = JSON.parse(readFileSync(new URL('./fixtures/eona-x.model.json', import.meta.url), 'utf8'));
const mm = (desktop) => () => ({ matches: desktop, addEventListener() {}, removeEventListener() {} });

test('rAF scheduled before destroy does not re-add is-scrolled', () => {
  const w = new JSDOM('<!doctype html><body>' + renderMenuHtml(fixture)).window;
  w.matchMedia = mm(true);
  const frames = new Map();
  let n = 0;
  w.requestAnimationFrame = (f) => { frames.set(++n, f); return n; };
  w.cancelAnimationFrame = (id) => frames.delete(id);
  const root = w.document.querySelector('[data-eona-menu]');
  const c = enhance(root, { window: w });
  w.scrollY = 500;
  w.dispatchEvent(new w.Event('scroll'));
  c.destroy();
  frames.forEach((f) => f());
  assert.equal(root.classList.contains('is-scrolled'), false);
});

test('drawer Tab trap wraps when the last focusable is an open language sublink', () => {
  const m = normalize({ ...fixture, actions: [fixture.actions[1], fixture.actions[0]] });
  const w = new JSDOM('<!doctype html><body>' + renderMenuHtml(m)).window;
  w.matchMedia = mm(false);
  const root = w.document.querySelector('[data-eona-menu]');
  enhance(root, { window: w });
  const toggle = root.querySelector('.eona-menu__toggle');
  toggle.click();
  root.querySelector('.site-header__actions--pill .eona-menu__lang button').click();
  const subs = root.querySelectorAll('.site-header__actions--pill .eona-menu__sublink');
  const last = subs[subs.length - 1];
  last.focus();
  const e = new w.KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
  last.dispatchEvent(e);
  assert.equal(e.defaultPrevented, true);
  assert.equal(w.document.activeElement, toggle);
});

test('mountMenu on the same target destroys the previous controller', async () => {
  const w = new JSDOM('<!doctype html><body><div id="t"></div>').window;
  w.matchMedia = mm(true);
  const c = console.error; console.error = () => {};
  try {
    await mountMenu('#t', fixture, { window: w });
    const old = w.document.querySelector('[data-eona-menu]');
    await mountMenu('#t', fixture, { window: w });
    assert.equal(old.classList.contains('is-enhanced'), false);
    w.scrollY = 500;
    w.dispatchEvent(new w.Event('scroll'));
    assert.equal(old.classList.contains('is-scrolled'), false);
    assert.ok(w.document.querySelector('[data-eona-menu]').classList.contains('is-scrolled'));
  } finally { console.error = c; }
});

test('turtle: out-of-range \\U escape is a TurtleError, not a RangeError', () => {
  assert.throws(() => parseTurtle('<s> <p> "\\U00110000" .'), (e) => e instanceof TurtleError && e.line === 1);
});

test('turtle: a document label _:b0 does not merge with an anonymous node', () => {
  const { triples } = parseTurtle('_:b0 <p> "x" . <s> <q> [ <r> "y" ] .');
  const anon = triples.find((t) => t.p === 'q').o.value;
  assert.notEqual(anon, '_:b0');
  assert.deepEqual(triples.filter((t) => t.s.value === '_:b0').map((t) => t.p), ['p']);
});

test('jsonld: an explicit _:b0 id does not merge with a generated bnode', () => {
  const tr = jsonldToTriples({ '@context': { p: 'urn:p', q: 'urn:q' }, '@graph': [{ p: 'y' }, { '@id': '_:b0', p: 'x' }] });
  const subjects = new Set(tr.filter((t) => t.p === 'urn:p').map((t) => t.s.value));
  assert.equal(subjects.size, 2);
});

test('jsonld: Object.prototype names as keys are dropped, not a TypeError', () => {
  const doc = JSON.parse(readFileSync(new URL('./fixtures/eona-x.jsonld', import.meta.url), 'utf8'));
  doc.constructor = 'x';
  doc.toString = 'y';
  doc.items[0].hasOwnProperty = 'z';
  assert.deepStrictEqual(fromJsonLd(doc), fixture);
  assert.doesNotThrow(() => jsonldToTriples({ '@context': MENU_CONTEXT_URL, valueOf: { '@id': 'urn:x' } }));
});

test('validate on raw authoring input returns errors instead of throwing', () => {
  assert.doesNotThrow(() => validate({ version: 1, options: { breakpoint: 1, collapseOffset: 1 }, items: [{ label: { '': 'x' }, href: '/' }], actions: [{ kind: 'language' }] }));
  const errs = validate({ version: 1, options: { breakpoint: 1, collapseOffset: 1 }, items: [{ label: { '': 'x' } }], actions: [{ kind: 'language' }] });
  assert.deepEqual(errs.map((e) => e.path), ['items[0].href', 'actions[0].alternates']);
});

test('logo width/height must be positive integers', () => {
  assert.throws(() => normalize({ logo: { href: '/', src: '/l.svg', width: 'abc' } }), (e) => e.errors.some((x) => x.path === 'logo.width'));
  assert.equal(normalize({ logo: { href: '/', src: '/l.svg', width: '38' } }).logo.width, 38);
});

test('hrefs are stored trimmed', () => {
  assert.equal(normalize({ items: [{ label: 'x', href: '  /x/ ' }] }).items[0].href, '/x/');
});

test('currentUrl matches absolute same-site hrefs, not external ones', () => {
  const m = normalize({ items: [{ label: 'A', href: 'https://eona-x.eu/fr/actualites/' }, { label: 'B', href: 'https://docs.eona-x.eu/', external: true }] });
  const d = new JSDOM(renderMenuHtml(m, { currentUrl: '/fr/actualites' })).window.document;
  assert.equal(d.querySelector('a[href="https://eona-x.eu/fr/actualites/"]').getAttribute('aria-current'), 'page');
  const d2 = new JSDOM(renderMenuHtml(m, { currentUrl: '/' })).window.document;
  assert.equal(d2.querySelector('a[href="https://docs.eona-x.eu/"]').getAttribute('aria-current'), null);
});

test('jsonld: a string _:x under an @type:@id term refers to the node with @id _:x', () => {
  const tr = jsonldToTriples({ '@context': { p: 'urn:p', r: { '@id': 'urn:r', '@type': '@id' } }, '@graph': [{ '@id': 'urn:s', r: '_:x' }, { '@id': '_:x', p: 'v' }] });
  const ref = tr.find((t) => t.p === 'urn:r').o;
  const subj = tr.find((t) => t.p === 'urn:p').s;
  assert.deepEqual(ref, subj);
});
