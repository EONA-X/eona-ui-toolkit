import test from 'node:test';
import assert from 'node:assert/strict';
import { graphToModel } from '../src/rdf.js';
import { normalize } from '../src/model.js';

const M = 'https://eona-x.eu/ontology/ui-toolkit-menu#';
const RDF = 'http://www.w3.org/1999/02/22-rdf-syntax-ns#';
const XSD = 'http://www.w3.org/2001/XMLSchema#';
const iri = (value) => ({ type: 'iri', value });
const bn = (value) => ({ type: 'bnode', value });
const lit = (value, extra = {}) => ({ type: 'literal', value, datatype: XSD + 'string', ...extra });
const lang = (value, l) => lit(value, { lang: l, datatype: RDF + 'langString' });
const t = (s, p, o) => ({ s, p, o });
const menuNode = (n = 'm') => t(iri('urn:' + n), RDF + 'type', iri(M + 'Menu'));
let seq = 0;
const list = (nodes) => {
  const out = []; let head = iri(RDF + 'nil');
  for (let i = nodes.length - 1; i >= 0; i--) {
    const b = bn('_:l' + seq++);
    out.push(t(b, RDF + 'first', nodes[i]), t(b, RDF + 'rest', head));
    head = b;
  }
  return { head, triples: out };
};
const leafT = (n, label, href, extra = []) => [t(bn(n), M + 'label', lit(label)), t(bn(n), M + 'href', lit(href)), ...extra];
const labels = (m) => m.items.map((i) => i.label['']);

test('finds single menu, errors on 0 or 2', () => {
  assert.throws(() => graphToModel([]), /menu/i);
  assert.throws(() => graphToModel([menuNode('a'), menuNode('b')]), /menu/i);
  const m = graphToModel([menuNode('a'), menuNode('b')], { menu: 'urn:b' });
  assert.ok(m);
});

test('items list order respected', () => {
  const l = list([bn('_:x'), bn('_:y')]);
  const m = normalize(graphToModel([menuNode(), t(iri('urn:m'), M + 'items', l.head), ...l.triples, ...leafT('_:x', 'X', '/x'), ...leafT('_:y', 'Y', '/y')]));
  assert.deepEqual(labels(m), ['X', 'Y']);
});

test('singular + order, ties by document order', () => {
  const s = iri('urn:m');
  const g = [menuNode(),
    t(s, M + 'item', iri('urn:c')), t(s, M + 'item', iri('urn:b')), t(s, M + 'item', iri('urn:a')), t(s, M + 'item', iri('urn:d')),
    ...['c', 'b', 'a', 'd'].flatMap((n, i) => [t(iri('urn:' + n), M + 'label', lit(n)), t(iri('urn:' + n), M + 'href', lit('/' + n))]),
    t(iri('urn:a'), M + 'order', lit('1', { datatype: XSD + 'integer' })),
    t(iri('urn:b'), M + 'order', lit('2', { datatype: XSD + 'integer' }))];
  assert.deepEqual(labels(normalize(graphToModel(g))), ['a', 'b', 'c', 'd']);
});

test('list wins over singular', () => {
  const l = list([bn('_:x')]);
  const g = [menuNode(), t(iri('urn:m'), M + 'items', l.head), ...l.triples, ...leafT('_:x', 'X', '/x'),
    t(iri('urn:m'), M + 'item', iri('urn:z')), t(iri('urn:z'), M + 'label', lit('Z')), t(iri('urn:z'), M + 'href', lit('/z'))];
  assert.deepEqual(labels(normalize(graphToModel(g))), ['X']);
});

test('labels map, untagged key, rdfs:label fallback', () => {
  const l = list([bn('_:x'), bn('_:y')]);
  const g = [menuNode(), t(iri('urn:m'), M + 'items', l.head), ...l.triples,
    t(bn('_:x'), M + 'label', lang('Bonjour', 'fr')), t(bn('_:x'), M + 'label', lit('Hello')), t(bn('_:x'), M + 'href', lit('/x')),
    t(bn('_:y'), 'http://www.w3.org/2000/01/rdf-schema#label', lit('Fallback')), t(bn('_:y'), M + 'href', lit('/y'))];
  const m = normalize(graphToModel(g));
  assert.deepEqual(m.items[0].label, { fr: 'Bonjour', '': 'Hello' });
  assert.deepEqual(m.items[1].label, { '': 'Fallback' });
});

test('href as IRI, booleans', () => {
  const l = list([bn('_:x'), bn('_:y')]);
  const g = [menuNode(), t(iri('urn:m'), M + 'items', l.head), ...l.triples,
    t(bn('_:x'), M + 'label', lit('X')), t(bn('_:x'), M + 'href', iri('https://a.example/x')),
    t(bn('_:x'), M + 'current', lit('true', { datatype: XSD + 'boolean' })), t(bn('_:x'), M + 'external', lit('1', { datatype: XSD + 'boolean' })),
    ...leafT('_:y', 'Y', '/y', [t(bn('_:y'), M + 'current', lit('false', { datatype: XSD + 'boolean' }))])];
  const m = normalize(graphToModel(g));
  assert.equal(m.items[0].href, 'https://a.example/x');
  assert.equal(m.items[0].current, true);
  assert.equal(m.items[0].external, true);
  assert.equal(m.items[1].current, false);
});

test('language switcher and link action', () => {
  const al = list([bn('_:sw'), bn('_:ln')]);
  const alt = list([bn('_:e'), bn('_:f')]);
  const g = [menuNode(), t(iri('urn:m'), M + 'actions', al.head), ...al.triples,
    t(bn('_:sw'), RDF + 'type', iri(M + 'LanguageSwitcher')), t(bn('_:sw'), M + 'alternates', alt.head), ...alt.triples,
    t(bn('_:e'), M + 'lang', lit('en')), t(bn('_:e'), M + 'label', lit('English')), t(bn('_:e'), M + 'href', lit('/en/')),
    t(bn('_:f'), M + 'lang', lit('fr')), t(bn('_:f'), M + 'label', lit('Francais')), t(bn('_:f'), M + 'href', lit('/fr/')),
    t(bn('_:ln'), M + 'label', lit('Go')), t(bn('_:ln'), M + 'href', lit('/go')), t(bn('_:ln'), M + 'variant', lit('solid'))];
  const m = normalize(graphToModel(g));
  assert.equal(m.actions[0].kind, 'language');
  assert.deepEqual(m.actions[0].alternates.map((a) => a.lang), ['en', 'fr']);
  assert.equal(m.actions[1].kind, 'link');
  assert.equal(m.actions[1].variant, 'solid');
});

test('logo ints and options', () => {
  const g = [menuNode(), t(iri('urn:m'), M + 'logo', bn('_:lg')), t(iri('urn:m'), M + 'breakpoint', lit('900', { datatype: XSD + 'integer' })),
    t(iri('urn:m'), M + 'collapseOffset', lit('50', { datatype: XSD + 'integer' })), t(iri('urn:m'), M + 'lang', lit('fr')),
    t(bn('_:lg'), M + 'href', lit('/')), t(bn('_:lg'), M + 'src', lit('/l.svg')), t(bn('_:lg'), M + 'srcCompact', lit('/c.svg')),
    t(bn('_:lg'), M + 'label', lit('Home')), t(bn('_:lg'), M + 'width', lit('38', { datatype: XSD + 'integer' })), t(bn('_:lg'), M + 'height', lit('36', { datatype: XSD + 'integer' }))];
  const m = normalize(graphToModel(g));
  assert.deepEqual(m.options, { breakpoint: 900, collapseOffset: 50 });
  assert.equal(m.lang, 'fr');
  assert.deepEqual(m.logo, { href: '/', src: '/l.svg', srcCompact: '/c.svg', alt: { '': 'Home' }, width: 38, height: 36 });
});
