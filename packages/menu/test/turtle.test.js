import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseTurtle, TurtleError } from '../lib/turtle.js';
import { MenuError } from '../src/errors.js';

const XSD = 'http://www.w3.org/2001/XMLSchema#';
const RDF = 'http://www.w3.org/1999/02/22-rdf-syntax-ns#';
const P = (t) => parseTurtle(t).triples;
const iri = (value) => ({ type: 'iri', value });
const bn = (value) => ({ type: 'bnode', value });
const lit = (value, datatype = XSD + 'string', lang) =>
  lang ? { type: 'literal', value, datatype: RDF + 'langString', lang } : { type: 'literal', value, datatype };
const err = (t, opts) => { try { parseTurtle(t, opts); } catch (e) { return e; } assert.fail('no throw'); };

test('1 empty and comments only', () => {
  assert.deepEqual(P(''), []);
  assert.deepEqual(P('# hi\n  # there\n'), []);
});

test('2 full IRIs', () => {
  assert.deepEqual(P('<http://a/s> <http://a/p> <http://a/o> .'),
    [{ s: iri('http://a/s'), p: 'http://a/p', o: iri('http://a/o') }]);
});

test('3 prefixes', () => {
  const r = parseTurtle('@prefix ex: <http://e/> .\nex:a ex:b ex:c .');
  assert.equal(r.triples[0].o.value, 'http://e/c');
  assert.equal(r.prefixes.ex, 'http://e/');
  assert.equal(P('PREFIX ex: <http://e/>\nprefix x: <http://x/>\nex:a ex:b x:c .')[0].o.value, 'http://x/c');
  assert.equal(P('@prefix : <http://e/> . :a :b :c-d.e1 .')[0].o.value, 'http://e/c-d.e1');
  assert.equal(P('@prefix : <http://e/> . :a :b :c\\/d\\.x .')[0].o.value, 'http://e/c/d.x');
  const e = err('@prefix ex: <http://e/> .\nex:a zz:b ex:c .');
  assert.ok(e instanceof TurtleError && e instanceof MenuError);
  assert.equal(e.line, 2); assert.equal(e.col, 6);
  assert.match(e.message, /unknown prefix 'zz'/);
});

test('4 base resolution', () => {
  assert.equal(P('@base <http://b/dir/> . <x> <p> <../y> .')[0].s.value, 'http://b/dir/x');
  assert.equal(P('@base <http://b/dir/> . <x> <p> <../y> .')[0].o.value, 'http://b/y');
  assert.equal(P('BASE <http://b/> <x> <p> <y> .')[0].o.value, 'http://b/y');
  assert.equal(P('<x> <p> <y> .')[0].o.value, 'y');
  assert.equal(parseTurtle('<x> <p> <y> .', { base: 'http://o/' }).triples[0].s.value, 'http://o/x');
  assert.equal(parseTurtle('<x> <p> <y> .', { base: 'http://o/' }).base, 'http://o/');
  assert.equal(parseTurtle('<x> <p> <y> .').base, null);
});

test('5 a is rdf:type', () => {
  assert.equal(P('<s> a <C> .')[0].p, RDF + 'type');
});

test('6 ; and , expansion', () => {
  const t = P('<s> <p> <a>, <b> ; <q> <c> ; .');
  assert.deepEqual(t.map((x) => [x.p, x.o.value]), [['p', 'a'], ['p', 'b'], ['q', 'c']]);
  const u = P('<s> <p> [ <q> <c> ; ] .');
  assert.equal(u.length, 2);
});

test('7 literals', () => {
  assert.deepEqual(P('<s> <p> "x" .')[0].o, lit('x'));
  assert.deepEqual(P('<s> <p> "x"@fr-CA .')[0].o, lit('x', null, 'fr-ca'));
  assert.deepEqual(P('@prefix xsd: <' + XSD + '> . <s> <p> "true"^^xsd:boolean .')[0].o, lit('true', XSD + 'boolean'));
  assert.deepEqual(P('<s> <p> "v"^^<http://dt/x> .')[0].o, lit('v', 'http://dt/x'));
  assert.deepEqual(P("<s> <p> 'single' .")[0].o, lit('single'));
});

test('8 long strings and escapes', () => {
  assert.equal(P('<s> <p> """a\nb "q" c""" .')[0].o.value, 'a\nb "q" c');
  assert.equal(P("<s> <p> '''a\nb 'q' c''' .")[0].o.value, "a\nb 'q' c");
  assert.equal(P('<s> <p> "\\t\\b\\n\\r\\f\\"\\\'\\\\" .')[0].o.value, '\t\b\n\r\f"\'\\');
  assert.equal(P('<s> <p> "\\u00e9\\U0001F600" .')[0].o.value, 'é\u{1F600}');
  assert.equal(P('<s> <p> """x\\ny"""@en .')[0].o.lang, 'en');
  assert.equal(P('<http://a/\\u0062> <p> <o> .')[0].s.value, 'http://a/b');
});

test('9 numeric and boolean shorthands', () => {
  const o = (t) => P('<s> <p> ' + t + ' .')[0].o;
  assert.deepEqual(o('42'), lit('42', XSD + 'integer'));
  assert.deepEqual(o('-7'), lit('-7', XSD + 'integer'));
  assert.deepEqual(o('+7'), lit('+7', XSD + 'integer'));
  assert.deepEqual(o('3.14'), lit('3.14', XSD + 'decimal'));
  assert.deepEqual(o('.5'), lit('.5', XSD + 'decimal'));
  assert.deepEqual(o('1e3'), lit('1e3', XSD + 'double'));
  assert.deepEqual(o('1.5E-2'), lit('1.5E-2', XSD + 'double'));
  assert.deepEqual(o('true'), lit('true', XSD + 'boolean'));
  assert.deepEqual(o('false'), lit('false', XSD + 'boolean'));
  const t = P('<s> <p> 1.');
  assert.deepEqual(t[0].o, lit('1', XSD + 'integer'));
});

test('10 # in IRI and string is not a comment', () => {
  const t = P('<http://a/b#c> <p> "x # y" . # real');
  assert.equal(t[0].s.value, 'http://a/b#c');
  assert.equal(t[0].o.value, 'x # y');
});

test('11 blank nodes', () => {
  let t = P('_:x <p> _:y . _:x <q> "z" .');
  assert.deepEqual(t[0], { s: bn('_:x'), p: 'p', o: bn('_:y') });
  assert.equal(t[1].s.value, '_:x');
  t = P('<s> <p> [] .');
  assert.deepEqual(t, [{ s: iri('s'), p: 'p', o: bn('_:b0') }]);
  t = P('<s> <p> [ <q> "1" ; <r> [ <t> "2" ] ] .');
  assert.deepEqual(t.map((x) => [x.s.value, x.p, x.o.value]),
    [['s', 'p', '_:b0'], ['_:b0', 'q', '1'], ['_:b0', 'r', '_:b1'], ['_:b1', 't', '2']]);
  t = P('[ <q> "1" ] <p> "2" .');
  assert.deepEqual(t.map((x) => [x.s.value, x.p, x.o.value]), [['_:b0', 'q', '1'], ['_:b0', 'p', '2']]);
  t = P('[] <p> "2" .');
  assert.deepEqual(t.map((x) => x.s.value), ['_:b0']);
});

test('12 collections', () => {
  assert.deepEqual(P('<s> <p> () .'), [{ s: iri('s'), p: 'p', o: iri(RDF + 'nil') }]);
  const t = P('<s> <p> (<a> "b") .');
  assert.deepEqual(t, [
    { s: iri('s'), p: 'p', o: bn('_:b0') },
    { s: bn('_:b0'), p: RDF + 'first', o: iri('a') },
    { s: bn('_:b0'), p: RDF + 'rest', o: bn('_:b1') },
    { s: bn('_:b1'), p: RDF + 'first', o: lit('b') },
    { s: bn('_:b1'), p: RDF + 'rest', o: iri(RDF + 'nil') },
  ]);
  const n = P('<s> <p> [ <q> ( <a> ) ] .');
  assert.deepEqual(n.map((x) => [x.s.value, x.p.split(/[#/]/).pop(), x.o.value]), [
    ['s', 'p', '_:b0'], ['_:b0', 'q', '_:b1'], ['_:b1', 'first', 'a'], ['_:b1', 'rest', RDF + 'nil']]);
  const nested = P('<s> <p> ((<a>)) .');
  assert.equal(nested.filter((x) => x.p === RDF + 'first').length, 2);
  const subj = P('(<a>) <p> "x" .');
  assert.equal(subj.at(-1).p, 'p');
  assert.equal(subj.at(-1).s.value, '_:b0');
});

test('13 order is stable, bnodes numbered by appearance', () => {
  const t = P('<s> <p> [ <q> [ <r> "1" ] ] , [ <q> "2" ] .');
  assert.deepEqual(t.map((x) => x.o.value), ['_:b0', '_:b1', '1', '_:b2', '2']);
  assert.equal(P('<s> <p> _:a, [] .')[1].o.value, '_:b0');
});

test('14 errors with line/col', () => {
  let e = err('<s> <p> "abc');
  assert.equal(e.message, 'unterminated string'); assert.equal(e.line, 1); assert.equal(e.col, 9);
  e = err('<s> <p> <o>\n<s2> <p> <o> .');
  assert.equal(e.message, "expected '.'"); assert.equal(e.line, 2); assert.equal(e.col, 1);
  e = err('<s> <p>');
  assert.equal(e.message, 'unexpected end of input'); assert.equal(e.line, 1);
  e = err('<s> <p> <o> .\n}');
  assert.equal(e.line, 2); assert.equal(e.col, 1); assert.match(e.message, /unexpected '\}'/);
  assert.match(err('GRAPH <g> { }').message, /GRAPH|TriG/);
  assert.match(err('<< <a> <b> <c> >> <p> <o> .').message, /RDF-star|unexpected/);
  assert.match(err('<s> <p> "a\nb" .').message, /unterminated string/);
});

test('15 vocab and fixture parse; golden', () => {
  const vocab = readFileSync(new URL('../vocab/menu.ttl', import.meta.url), 'utf8');
  assert.doesNotThrow(() => parseTurtle(vocab));
  const src = readFileSync(new URL('./fixtures/eona-x.ttl', import.meta.url), 'utf8');
  const { triples } = parseTurtle(src);
  const M = 'https://eona-x.eu/ns/menu#';
  const main = iri('https://eona-x.eu/menu/main');
  assert.deepEqual(triples.slice(0, 5), [
    { s: main, p: RDF + 'type', o: iri(M + 'Menu') },
    { s: main, p: M + 'lang', o: lit('fr') },
    { s: main, p: M + 'ariaLabel', o: lit('Navigation principale', null, 'fr') },
    { s: main, p: M + 'ariaLabel', o: lit('Main navigation', null, 'en') },
    { s: main, p: M + 'collapseOffset', o: lit('200', XSD + 'integer') },
  ]);
  assert.equal(triples.length, 63);
});
