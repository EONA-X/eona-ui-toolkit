import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { ttlToModel } from '../lib/ttl.js';
import { fromJsonLd, MENU_CONTEXT_URL } from '../src/jsonld.js';
import { MenuError } from '../src/errors.js';

const rd = (p) => readFileSync(new URL(p, import.meta.url), 'utf8');
const NEW_NS = 'https://eona-x.eu/ontology/ui-toolkit-menu#';
const OLD_NS = 'https://eona-x.eu/ns/menu#';
const NEW_CTX = 'https://eona-x.eu/ontology/ui-toolkit-menu/context.jsonld';
const OLD_CTX = 'https://eona-x.eu/ns/menu/context.jsonld';

test('built-in context URL is the conventional one', () => {
  assert.equal(MENU_CONTEXT_URL, NEW_CTX);
});

test('Turtle with legacy prefix and new prefix produce the same model', () => {
  const src = rd('./fixtures/eona-x.ttl');
  assert.ok(src.includes(NEW_NS));
  const legacy = src.replaceAll(NEW_NS, OLD_NS);
  assert.ok(legacy.includes(OLD_NS) && !legacy.includes(NEW_NS));
  assert.deepEqual(ttlToModel(legacy), ttlToModel(src));
});

test('Turtle mixing both namespaces is read', () => {
  const t = (a, b) => `@prefix a: <${a}> . @prefix b: <${b}> . <urn:m> a a:Menu ; b:items ( [ a:label "A" ; a:href "/a" ] ) .`;
  assert.deepEqual(ttlToModel(t(OLD_NS, NEW_NS)), ttlToModel(t(NEW_NS, NEW_NS)));
});

test('JSON-LD with legacy context URL and new context URL produce the same model', () => {
  const doc = JSON.parse(rd('./fixtures/eona-x.jsonld'));
  assert.equal(doc['@context'], NEW_CTX);
  assert.deepEqual(fromJsonLd({ ...doc, '@context': OLD_CTX }), fromJsonLd(doc));
  assert.deepEqual(fromJsonLd({ ...doc, '@context': [OLD_CTX] }), fromJsonLd(doc));
});

test('JSON-LD with an inline legacy menu prefix reads like the new one', () => {
  const d = (ns) => ({ '@context': { menu: ns, items: { '@id': 'menu:items', '@container': '@list' }, label: 'menu:label' }, '@id': 'urn:m', '@type': 'menu:Menu', items: [{ label: 'A', 'menu:href': '/a' }] });
  assert.deepEqual(fromJsonLd(d(OLD_NS)), fromJsonLd(d(NEW_NS)));
});

test('unknown remote context still throws', () => {
  assert.throws(() => fromJsonLd({ '@context': 'https://other.example/ctx', '@type': 'Menu' }), (e) => e instanceof MenuError && /remote contexts are not fetched/.test(e.message));
});

test('vocab/menu.ttl local names equal the B3 ontology', () => {
  const names = (s) => [...s.matchAll(/^menu:([A-Za-z]+) a /gm)].map((m) => m[1]).sort();
  const vocab = rd('../vocab/menu.ttl');
  assert.ok(names(vocab).length === 29);
  assert.deepEqual(names(vocab), names(rd('./fixtures/b3-ontology.ttl')));
  assert.equal(vocab, rd('./fixtures/b3-ontology.ttl'));
});

test('vocab/context.jsonld uses the new namespace and matches the built-in context', async () => {
  const ctx = JSON.parse(rd('../vocab/context.jsonld'))['@context'];
  assert.equal(ctx.menu, NEW_NS);
  const { MENU_CONTEXT } = await import('../src/jsonld.js');
  assert.deepEqual(ctx, MENU_CONTEXT);
});
