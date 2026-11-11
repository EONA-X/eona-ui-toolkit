import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { jsonldToTriples, fromJsonLd, MENU_CONTEXT, MENU_CONTEXT_URL } from '../src/jsonld.js';
import { MenuError } from '../src/errors.js';

const rd = (f) => readFileSync(new URL('./fixtures/' + f, import.meta.url), 'utf8');
const M = 'https://eona-x.eu/ontology/ui-toolkit-menu#';
const XSD = 'http://www.w3.org/2001/XMLSchema#';
const RDF = 'http://www.w3.org/1999/02/22-rdf-syntax-ns#';
const find = (ts, p) => ts.filter((t) => t.p === p);
const doc = { '@type': 'Menu', lang: 'fr', items: [{ label: { '@none': 'A', fr: 'Af' }, href: '/a' }] };

test('context url, inline, unknown', () => {
  const a = jsonldToTriples({ '@context': MENU_CONTEXT_URL, ...doc });
  const b = jsonldToTriples({ '@context': MENU_CONTEXT, ...doc });
  const c = jsonldToTriples({ '@context': [MENU_CONTEXT_URL], ...doc });
  assert.deepEqual(a, b);
  assert.deepEqual(a, c);
  assert.throws(() => jsonldToTriples({ '@context': 'https://other/ctx', ...doc }), (e) => e instanceof MenuError && /remote contexts/.test(e.message));
});

test('language maps with @none', () => {
  const ts = jsonldToTriples({ '@context': MENU_CONTEXT_URL, ...doc });
  const labels = find(ts, M + 'label').map((t) => t.o);
  assert.deepEqual(labels.map((l) => [l.value, l.lang, l.datatype]), [
    ['A', undefined, XSD + 'string'], ['Af', 'fr', RDF + 'langString']]);
});

test('@list becomes rdf:List', () => {
  const ts = jsonldToTriples({ '@context': MENU_CONTEXT_URL, ...doc });
  assert.equal(find(ts, M + 'items').length, 1);
  assert.equal(find(ts, RDF + 'first').length, 1);
  assert.equal(find(ts, RDF + 'rest')[0].o.value, RDF + 'nil');
  const e = jsonldToTriples({ '@context': MENU_CONTEXT_URL, '@type': 'Menu', items: [] });
  assert.equal(find(e, M + 'items')[0].o.value, RDF + 'nil');
});

test('typed terms and json typing', () => {
  const ts = jsonldToTriples({ '@context': MENU_CONTEXT_URL, '@type': 'Menu', breakpoint: 900, collapseOffset: '50',
    logo: { current: true, width: 3 }, 'menu:x': 1.5, 'menu:y': true, 'menu:z': 2 });
  const get = (p) => find(ts, p)[0].o;
  assert.equal(get(M + 'breakpoint').datatype, XSD + 'integer');
  assert.equal(get(M + 'collapseOffset').datatype, XSD + 'integer');
  assert.equal(get(M + 'current').datatype, XSD + 'boolean');
  assert.equal(get(M + 'x').datatype, XSD + 'double');
  assert.equal(get(M + 'y').datatype, XSD + 'boolean');
  assert.equal(get(M + 'z').datatype, XSD + 'integer');
});

test('@graph, arrays, @id references', () => {
  const g = jsonldToTriples({ '@context': MENU_CONTEXT_URL, '@graph': [
    { '@id': 'urn:m', '@type': 'Menu', item: { '@id': 'urn:i' } }, { '@id': 'urn:i', label: 'I', href: '/i' }] });
  const item = find(g, M + 'item')[0];
  assert.deepEqual(item.o, { type: 'iri', value: 'urn:i' });
  assert.equal(g.find((t) => t.s.value === 'urn:i' && t.p === M + 'href').o.value, '/i');
  const arr = jsonldToTriples([{ '@context': MENU_CONTEXT_URL, '@id': 'urn:a', '@type': ['Menu'] }]);
  assert.equal(arr.length, 1);
  assert.equal(arr[0].o.value, M + 'Menu');
});

test('prefixed iris and @vocab', () => {
  const ts = jsonldToTriples({ '@context': { menu: M, '@vocab': 'http://v/', foo: 'menu:foo' }, '@id': 'urn:x', foo: 'a', bar: 'b', 'menu:baz': 'c' });
  const ps = ts.map((t) => t.p).sort();
  assert.deepEqual(ps, ['http://v/bar', M + 'baz', M + 'foo']);
});

test('unknown terms dropped', () => {
  const ts = jsonldToTriples({ '@context': MENU_CONTEXT_URL, '@id': 'urn:x', nope: 'x', '@weird': 1 });
  assert.deepEqual(ts, []);
});

test('fromJsonLd(eona-x.jsonld) equals model fixture', () => {
  assert.deepStrictEqual(fromJsonLd(JSON.parse(rd('eona-x.jsonld'))), JSON.parse(rd('eona-x.model.json')));
});
