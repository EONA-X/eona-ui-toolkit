// Turtle subset parser: text -> triples. See README for the supported subset.
import { MenuError } from '../src/errors.js';

const RDF = 'http://www.w3.org/1999/02/22-rdf-syntax-ns#';
const XSD = 'http://www.w3.org/2001/XMLSchema#';
const RDF_NIL = RDF + 'nil';
const ESC = { t: '\t', b: '\b', n: '\n', r: '\r', f: '\f', '"': '"', "'": "'", '\\': '\\' };
const LOCAL_ESC = '_~.-!$&\'()*+,;=/?#@%';

export class TurtleError extends MenuError {
  constructor(message, line, col) {
    super(message);
    this.name = 'TurtleError';
    this.line = line;
    this.col = col;
  }
}

const isDigit = (c) => c >= '0' && c <= '9';
const isHex = (c) => isDigit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
const isAlpha = (c) => (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z');
const isWs = (c) => c === ' ' || c === '\t' || c === '\n' || c === '\r' || c === '\f';
const isNameChar = (c) =>
  isAlpha(c) || isDigit(c) || c === '_' || c === '-' || c === ':' || c === '%' || c === '\\' || c > '\x7f';

function tokenize(src, fail) {
  const toks = [];
  const n = src.length;
  let i = 0;

  const hex = (len, at) => {
    const h = src.slice(i, i + len);
    const cp = parseInt(h, 16);
    if (h.length !== len || ![...h].every(isHex) || cp > 0x10ffff) fail('invalid unicode escape', at);
    i += len;
    return String.fromCodePoint(cp);
  };

  const readIri = () => {
    const at = i;
    i++;
    let v = '';
    for (;;) {
      if (i >= n || src[i] === '\n') fail('unterminated IRI', at);
      const c = src[i];
      if (c === '>') { i++; return { t: 'iri', v, at }; }
      if (c === '\\') {
        const k = src[i + 1];
        if (k === 'u') { i += 2; v += hex(4, at); }
        else if (k === 'U') { i += 2; v += hex(8, at); }
        else fail('invalid escape in IRI', i);
      } else { v += c; i++; }
    }
  };

  const readString = () => {
    const at = i;
    const q = src[i];
    const long = src[i + 1] === q && src[i + 2] === q;
    i += long ? 3 : 1;
    let v = '';
    for (;;) {
      if (i >= n) fail('unterminated string', at);
      const c = src[i];
      if (c === q) {
        if (!long) { i++; break; }
        if (src[i + 1] === q && src[i + 2] === q) {
          if (src[i + 3] === q) { v += q; i++; continue; }
          i += 3;
          break;
        }
        v += c; i++;
      } else if (c === '\\') {
        const k = src[i + 1];
        i += 2;
        if (k === 'u') v += hex(4, at);
        else if (k === 'U') v += hex(8, at);
        else if (k !== undefined && Object.hasOwn(ESC, k)) v += ESC[k];
        else fail('invalid escape in string', i - 2);
      } else if (!long && (c === '\n' || c === '\r')) {
        fail('unterminated string', at);
      } else { v += c; i++; }
    }
    const tok = { t: 'str', v, at };
    if (src[i] === '@') {
      let j = i + 1;
      while (j < n && (isAlpha(src[j]) || isDigit(src[j]) || src[j] === '-')) j++;
      if (j === i + 1) fail('invalid language tag', i);
      tok.lang = src.slice(i + 1, j).toLowerCase();
      i = j;
    } else if (src[i] === '^' && src[i + 1] === '^') {
      i += 2;
      if (src[i] === '<') tok.dt = readIri();
      else {
        const w = readWord();
        if (!w || w.t !== 'pname') fail('expected datatype IRI', i);
        tok.dt = w;
      }
    }
    return tok;
  };

  // Bare word: prefixed name or keyword. Returns null if nothing readable.
  const readWord = () => {
    const at = i;
    let j = i;
    let raw = '';
    for (;;) {
      const c = src[j];
      if (c === undefined) break;
      if (c === '\\') {
        const k = src[j + 1];
        if (k === undefined || !LOCAL_ESC.includes(k)) fail('invalid escape in name', j);
        raw += '\u0000' + k;
        j += 2;
      } else if (isNameChar(c)) { raw += c; j++; }
      else if (c === '.' && (isNameChar(src[j + 1] ?? ' ') || src[j + 1] === '.')) {
        if (src[j + 1] === '.') {
          let m = j;
          while (src[m] === '.') m++;
          if (!isNameChar(src[m] ?? ' ')) break;
        }
        raw += c; j++;
      } else break;
    }
    if (j === i) return null;
    i = j;
    const colon = raw.indexOf(':');
    if (colon < 0) return { t: 'word', v: raw, at };
    const local = raw.slice(colon + 1).replaceAll('\u0000', '');
    return { t: 'pname', prefix: raw.slice(0, colon), local, at };
  };

  const readNumber = () => {
    const at = i;
    let j = i;
    if (src[j] === '+' || src[j] === '-') j++;
    while (isDigit(src[j] ?? '')) j++;
    let kind = 'integer';
    if (src[j] === '.' && isDigit(src[j + 1] ?? '')) {
      kind = 'decimal';
      j++;
      while (isDigit(src[j] ?? '')) j++;
    }
    if (src[j] === 'e' || src[j] === 'E') {
      let m = j + 1;
      if (src[m] === '+' || src[m] === '-') m++;
      if (isDigit(src[m] ?? '')) {
        kind = 'double';
        while (isDigit(src[m] ?? '')) m++;
        j = m;
      }
    }
    const v = src.slice(i, j);
    i = j;
    return { t: 'num', v, dt: XSD + kind, at };
  };

  while (i < n) {
    const c = src[i];
    if (isWs(c)) { i++; continue; }
    if (c === '#') { while (i < n && src[i] !== '\n') i++; continue; }
    if (c === '<') {
      if (src[i + 1] === '<') fail('RDF-star is not supported', i);
      toks.push(readIri());
    } else if (c === '"' || c === "'") toks.push(readString());
    else if (c === '@') {
      const at = i;
      let j = i + 1;
      while (isAlpha(src[j] ?? '')) j++;
      const w = src.slice(i, j);
      if (w !== '@prefix' && w !== '@base') fail(`unexpected '${w}'`, at);
      i = j;
      toks.push({ t: 'dir', v: w.slice(1), at });
    } else if (c === '_' && src[i + 1] === ':') {
      const at = i;
      let j = i + 2;
      while (j < n && (isAlpha(src[j]) || isDigit(src[j]) || src[j] === '_' || src[j] === '-' || src[j] > '\x7f' ||
        (src[j] === '.' && (isAlpha(src[j + 1] ?? ' ') || isDigit(src[j + 1] ?? ' '))))) j++;
      if (j === i + 2) fail('empty blank node label', at);
      toks.push({ t: 'bnode', v: src.slice(i, j), at });
      i = j;
    } else if (isDigit(c) || ((c === '+' || c === '-' || c === '.') &&
      (isDigit(src[i + 1] ?? '') || (c !== '.' && src[i + 1] === '.' && isDigit(src[i + 2] ?? ''))))) {
      toks.push(readNumber());
    } else if ('.;,[]()'.includes(c)) { toks.push({ t: c, at: i }); i++; }
    else if (c === '^') fail("unexpected '^'", i);
    else {
      const w = readWord();
      if (!w) fail(`unexpected '${c}'` + (c === '{' || c === '}' ? ' (TriG is not supported)' : ''), i);
      toks.push(w);
    }
  }
  toks.push({ t: 'eof', at: n });
  return toks;
}

/**
 * @param {string} text
 * @param {{base?: string}} [opts]
 * @returns {{triples: object[], prefixes: Record<string,string>, base: string|null}}
 */
export function parseTurtle(text, opts = {}) {
  const src = String(text);
  const posAt = (idx) => {
    let line = 1;
    let ls = 0;
    for (let k = 0; k < idx && k < src.length; k++) if (src[k] === '\n') { line++; ls = k + 1; }
    return [line, idx - ls + 1];
  };
  const fail = (msg, idx) => { const [l, c] = posAt(idx); throw new TurtleError(msg, l, c); };

  const toks = tokenize(src, fail);
  const prefixes = {};
  const triples = [];
  let base = opts.base ?? null;
  let p = 0;
  let bn = 0;

  const peek = () => toks[p];
  const next = () => toks[p++];
  const unexpected = (t) => fail(t.t === 'eof' ? 'unexpected end of input' : `unexpected '${describe(t)}'`, t.at);
  const describe = (t) => (t.v !== undefined ? t.v : t.t === 'pname' ? `${t.prefix}:${t.local}` : t.t);
  const expect = (type, msg) => {
    const t = next();
    if (t.t !== type) {
      if (t.t === 'eof') unexpected(t);
      fail(msg || `expected '${type}'`, t.at);
    }
    return t;
  };

  const resolve = (v) => {
    if (base === null || /^[A-Za-z][A-Za-z0-9+.-]*:/.test(v)) return v;
    try { return new URL(v, base).href; } catch { return v; }
  };
  const iriOf = (t) => resolve(t.v);
  const pnameOf = (t) => {
    if (!Object.hasOwn(prefixes, t.prefix)) fail(`unknown prefix '${t.prefix}'`, t.at);
    return prefixes[t.prefix] + t.local;
  };
  const termIri = (t) => (t.t === 'iri' ? iriOf(t) : pnameOf(t));
  const emit = (s, pr, o) => triples.push({ s, p: pr, o });
  // Anonymous nodes are _:b0, _:b1, ...; skip any number the document already uses as a label.
  const labels = new Set(toks.filter((t) => t.t === 'bnode').map((t) => t.v));
  const newB = () => {
    let v;
    do v = '_:b' + bn++; while (labels.has(v));
    return { type: 'bnode', value: v };
  };

  const directive = () => {
    const d = next();
    if (d.t === 'dir' || d.v.toLowerCase() === 'prefix') {
      const isPrefix = d.t === 'dir' ? d.v === 'prefix' : true;
      if (isPrefix) {
        const name = next();
        if (name.t !== 'pname' || name.local !== '') fail('expected prefix name', name.at);
        const iri = next();
        if (iri.t !== 'iri') fail('expected IRI', iri.at);
        prefixes[name.prefix] = iriOf(iri);
      } else {
        const iri = next();
        if (iri.t !== 'iri') fail('expected IRI', iri.at);
        base = iriOf(iri);
      }
    } else {
      const iri = next();
      if (iri.t !== 'iri') fail('expected IRI', iri.at);
      base = iriOf(iri);
    }
    if (d.t === 'dir') expect('.');
  };

  const literal = (t) => {
    if (t.t === 'num') return { type: 'literal', value: t.v, datatype: t.dt };
    if (t.lang) return { type: 'literal', value: t.v, datatype: RDF + 'langString', lang: t.lang };
    return { type: 'literal', value: t.v, datatype: t.dt ? termIri(t.dt) : XSD + 'string' };
  };

  // Collection after '('. Returns head term; emits outer link via `link` first.
  const collection = (link) => {
    const open = next();
    if (peek().t === ')') { next(); const nil = { type: 'iri', value: RDF_NIL }; link(nil); return nil; }
    let cell = newB();
    const head = cell;
    link(head);
    for (;;) {
      if (peek().t === 'eof') unexpected(peek());
      object(cell, RDF + 'first');
      if (peek().t === ')') { next(); emit(cell, RDF + 'rest', { type: 'iri', value: RDF_NIL }); break; }
      const nxt = newB();
      emit(cell, RDF + 'rest', nxt);
      cell = nxt;
    }
    void open;
    return head;
  };

  const propertyList = (s) => {
    for (;;) {
      const v = next();
      let pr;
      if (v.t === 'word' && v.v === 'a') pr = RDF + 'type';
      else if (v.t === 'iri' || v.t === 'pname') pr = termIri(v);
      else unexpected(v);
      for (;;) {
        object(s, pr);
        if (peek().t === ',') { next(); continue; }
        break;
      }
      if (peek().t !== ';') return;
      while (peek().t === ';') next();
      const t = peek().t;
      if (t === '.' || t === ']' || t === 'eof') return;
    }
  };

  const blankNodeProps = (link) => {
    next();
    const b = newB();
    link(b);
    if (peek().t !== ']') propertyList(b);
    expect(']');
    return b;
  };

  function object(s, pr) {
    const t = peek();
    const link = (o) => emit(s, pr, o);
    switch (t.t) {
      case 'iri': case 'pname': next(); link({ type: 'iri', value: termIri(t) }); return;
      case 'bnode': next(); link({ type: 'bnode', value: t.v }); return;
      case 'str': case 'num': next(); link(literal(t)); return;
      case 'word':
        if (t.v === 'true' || t.v === 'false') { next(); link({ type: 'literal', value: t.v, datatype: XSD + 'boolean' }); return; }
        unexpected(t); return;
      case '[': blankNodeProps(link); return;
      case '(': collection(link); return;
      default: unexpected(t);
    }
  }

  const statement = () => {
    const t = peek();
    if (t.t === 'dir' || (t.t === 'word' && /^(prefix|base)$/i.test(t.v))) return directive();
    if (t.t === 'word' && /^graph$/i.test(t.v)) fail('GRAPH/TriG is not supported', t.at);
    let s;
    let bare = false;
    if (t.t === 'iri' || t.t === 'pname') { next(); s = { type: 'iri', value: termIri(t) }; }
    else if (t.t === 'bnode') { next(); s = { type: 'bnode', value: t.v }; }
    else if (t.t === '[') { bare = peek().t === '[' && toks[p + 1].t !== ']'; s = blankNodeProps(() => {}); }
    else if (t.t === '(') s = collection(() => {});
    else unexpected(t);
    if (!(bare && peek().t === '.')) propertyList(s);
    expect('.');
  };

  while (peek().t !== 'eof') statement();
  return { triples, prefixes, base };
}
