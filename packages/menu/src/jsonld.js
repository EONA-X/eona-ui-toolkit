// JSON-LD subset to triples; never fetches.
import { MenuError } from './errors.js';
import { graphToModel } from './rdf.js';
import { normalize } from './model.js';

export const MENU_CONTEXT_URL = 'https://eona-x.eu/ns/menu/context.jsonld';
function jsonldContext() {
  const c = { menu: 'https://eona-x.eu/ns/menu#', xsd: 'http://www.w3.org/2001/XMLSchema#', id: '@id', type: '@type' };
  const add = (names, extra) => names.split(' ').forEach((k) => {
    const p = 'menu:' + k;
    c[k] = extra ? { '@id': p, ...extra } : p;
  });
  add('Menu Item Logo LinkAction LanguageSwitcher Alternate lang variant src srcCompact href logo item child action alternate');
  add('label ariaLabel', { '@container': '@language' });
  add('items children actions alternates', { '@container': '@list' });
  for (const t of ['order', 'width', 'height', 'breakpoint', 'collapseOffset']) add(t, { '@type': 'xsd:integer' });
  add('current external', { '@type': 'xsd:boolean' });
  return c;
}

export const MENU_CONTEXT = jsonldContext();

const XSD = 'http://www.w3.org/2001/XMLSchema#';
const RDF = 'http://www.w3.org/1999/02/22-rdf-syntax-ns#';
const isNodeObj = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const asArr = (v) => (Array.isArray(v) ? v : [v]);

function applyContext(ctx, c) {
  const next = { terms: { ...ctx.terms }, vocab: ctx.vocab, lang: ctx.lang };
  for (const part of asArr(c)) {
    if (part === MENU_CONTEXT_URL) Object.assign(next.terms, applyContext(next, MENU_CONTEXT).terms);
    else if (typeof part === 'string') throw new MenuError('remote contexts are not fetched: ' + part);
    else if (isNodeObj(part)) {
      for (const k of Object.keys(part)) {
        const v = part[k];
        const d = typeof v === 'string' ? { '@id': v } : v;
        if (k === '@vocab') next.vocab = v;
        else if (k === '@language') next.lang = v ? String(v).toLowerCase() : undefined;
        else if (!k.startsWith('@') && isNodeObj(d) && typeof d['@id'] === 'string') next.terms[k] = { id: d['@id'], type: d['@type'], container: d['@container'] };
      }
    }
  }
  return next;
}

function expand(ctx, v, vocab, depth = 0) {
  if (typeof v !== 'string' || depth > 8) return null;
  if (v.startsWith('@')) return v;
  const def = ctx.terms[v];
  if (vocab && def) return expand(ctx, def.id, true, depth + 1);
  const i = v.indexOf(':');
  if (i > 0) {
    const p = ctx.terms[v.slice(0, i)];
    const base = p && !v.startsWith('//', i + 1) ? expand(ctx, p.id, true, depth + 1) : '';
    return base === null ? null : base + (p ? v.slice(i + 1) : v);
  }
  return vocab ? (ctx.vocab ? ctx.vocab + v : null) : v;
}

export function jsonldToTriples(doc) {
  const triples = [];
  let n = 0;
  const iri = (value) => ({ type: 'iri', value });
  const bnode = () => ({ type: 'bnode', value: '_:b' + n++ });
  const nil = iri(RDF + 'nil');
  const lit = (value, datatype, lang) => (lang ? { type: 'literal', value, datatype: RDF + 'langString', lang: lang.toLowerCase() } : { type: 'literal', value, datatype });
  const typed = (v, dt) => (typeof v === 'number' ? lit(String(v), dt || XSD + (Number.isInteger(v) ? 'integer' : 'double')) : lit(String(v), dt || XSD + 'boolean'));

  const makeList = (items, ctx, def) => {
    const nodes = items.map(bnode);
    items.forEach((it, i) => {
      const o = term(it, ctx, def);
      triples.push({ s: nodes[i], p: RDF + 'first', o }, { s: nodes[i], p: RDF + 'rest', o: nodes[i + 1] || nil });
    });
    return nodes[0] || nil;
  };

  const term = (v, ctx, def) => {
    const dt = def && def.type && def.type !== '@id' ? expand(ctx, def.type, true) : null;
    if (typeof v === 'string') return def && def.type === '@id' ? iri(expand(ctx, v, false)) : lit(v, dt || XSD + 'string', dt ? null : ctx.lang);
    if (typeof v === 'number' || typeof v === 'boolean') return typed(v, dt);
    if (!isNodeObj(v)) return null;
    if ('@list' in v) return makeList(asArr(v['@list']), ctx, def);
    if (!('@value' in v)) return node(v, ctx);
    const raw = v['@value'];
    const t = v['@type'] ? expand(ctx, v['@type'], true) : null;
    if (typeof raw === 'string') return lit(raw, t || XSD + 'string', t ? null : v['@language']);
    return typeof raw === 'number' || typeof raw === 'boolean' ? typed(raw, t) : null;
  };

  const node = (obj, parent) => {
    const ctx = '@context' in obj ? applyContext(parent, obj['@context']) : parent;
    const id = obj['@id'];
    const s = typeof id !== 'string' ? bnode() : id.startsWith('_:') ? { type: 'bnode', value: id } : iri(expand(ctx, id, false));
    for (const k of Object.keys(obj)) {
      const def = ctx.terms[k];
      const kw = k.startsWith('@') ? k : def && def.id.startsWith('@') ? def.id : null;
      const val = obj[k];
      let outs = [];
      let p = null;
      if (kw === '@type') {
        p = RDF + 'type';
        outs = asArr(val).map((t) => expand(ctx, t, true)).filter(Boolean).map(iri);
      } else if (kw === null && (p = expand(ctx, k, true))) {
        if (def && def.container === '@language' && isNodeObj(val)) {
          outs = Object.keys(val).flatMap((lk) => asArr(val[lk]).filter((x) => x !== null).map((x) => (lk === '@none' ? lit(String(x), XSD + 'string') : lit(String(x), XSD + 'string', lk))));
        } else if (def && def.container === '@list') outs = [makeList(asArr(val), ctx, def)];
        else outs = asArr(val).map((x) => term(x, ctx, def));
      }
      outs.filter(Boolean).forEach((o) => triples.push({ s, p, o }));
    }
    return s;
  };

  const top = (d, parent) => {
    if (Array.isArray(d)) return d.forEach((x) => top(x, parent));
    if (!isNodeObj(d)) return;
    const ctx = '@context' in d ? applyContext(parent, d['@context']) : parent;
    if (!('@graph' in d)) return node(d, ctx);
    asArr(d['@graph']).forEach((x) => top(x, ctx));
    if (Object.keys(d).some((k) => !['@context', '@graph', '@id'].includes(k))) {
      const rest = { ...d };
      delete rest['@graph'];
      node(rest, ctx);
    }
  };

  top(doc, { terms: {}, vocab: undefined, lang: undefined });
  return triples;
}

export function fromJsonLd(doc, opts) {
  return normalize(graphToModel(jsonldToTriples(doc), opts));
}
