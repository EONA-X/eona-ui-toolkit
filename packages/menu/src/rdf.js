// Triples to authoring-shape model (un-normalized).
import { MenuError } from './errors.js';

const NS = 'https://eona-x.eu/ns/menu#';
const RDF_NS = 'http://www.w3.org/1999/02/22-rdf-syntax-ns#';
const RDFS_LABEL = 'http://www.w3.org/2000/01/rdf-schema#label';
const key = (n) => n.type + n.value;

export function graphToModel(triples, opts) {
  const by = new Map();
  for (const t of triples) {
    if (!by.has(key(t.s))) by.set(key(t.s), []);
    by.get(key(t.s)).push(t);
  }
  const objs = (n, p) => (by.get(key(n)) || []).filter((t) => t.p === (p.includes(':') ? p : NS + p)).map((t) => t.o);
  const one = (n, p) => objs(n, p)[0];
  const text = (n, p) => (one(n, p) || {}).value;
  const num = (n, p) => (text(n, p) === undefined ? undefined : Number(text(n, p)));
  const flag = (n, p) => ['true', '1'].includes(text(n, p));
  const isType = (n, t) => objs(n, RDF_NS + 'type').some((o) => o.value === NS + t);
  const readList = (head) => {
    const out = [];
    for (let c = head, i = 0; c && c.value !== RDF_NS + 'nil' && i < 1e4; i++) {
      out.push(one(c, RDF_NS + 'first'));
      c = one(c, RDF_NS + 'rest');
    }
    return out.filter(Boolean);
  };
  const ordered = (n, list, single) => {
    const head = one(n, list);
    if (head) return readList(head);
    return objs(n, single).map((o, i) => [num(o, 'order') ?? Infinity, i, o]).sort((a, b) => a[0] - b[0] || a[1] - b[1]).map((x) => x[2]);
  };
  const langs = (n, p) => {
    const out = {};
    for (const o of objs(n, p)) out[o.lang || ''] ??= o.value;
    return out;
  };
  const labels = (n) => {
    const l = langs(n, 'label');
    return Object.keys(l).length ? l : langs(n, RDFS_LABEL);
  };
  const item = (n, depth) => ({
    label: labels(n),
    href: text(n, 'href'),
    current: flag(n, 'current'),
    external: flag(n, 'external'),
    children: depth > 4 ? [] : ordered(n, 'children', 'child').map((c) => item(c, depth + 1)),
  });
  const action = (n) =>
    isType(n, 'LanguageSwitcher')
      ? { kind: 'language', alternates: ordered(n, 'alternates', 'alternate').map((a) => ({ lang: text(a, 'lang'), label: labels(a), href: text(a, 'href'), current: flag(a, 'current') })) }
      : { kind: 'link', label: labels(n), href: text(n, 'href'), external: flag(n, 'external'), variant: text(n, 'variant') };

  const menus = opts && opts.menu ? [{ type: 'iri', value: opts.menu }] : [...new Map(triples.filter((t) => t.p === RDF_NS + 'type' && t.o.value === NS + 'Menu').map((t) => [key(t.s), t.s])).values()];
  if (menus.length !== 1) throw new MenuError(menus.length ? 'Multiple menu:Menu nodes found; pass {menu} to choose one' : 'No menu:Menu found in graph');
  const root = menus[0];
  if (!by.has(key(root))) throw new MenuError('Menu node not found: ' + root.value);
  const logo = one(root, 'logo');
  return {
    lang: text(root, 'lang'),
    ariaLabel: langs(root, 'ariaLabel'),
    options: { breakpoint: num(root, 'breakpoint'), collapseOffset: num(root, 'collapseOffset') },
    logo: logo && { href: text(logo, 'href'), src: text(logo, 'src'), srcCompact: text(logo, 'srcCompact'), alt: labels(logo), width: num(logo, 'width'), height: num(logo, 'height') },
    items: ordered(root, 'items', 'item').map((n) => item(n, 1)),
    actions: ordered(root, 'actions', 'action').map(action),
  };
}
