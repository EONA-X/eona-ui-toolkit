import { MenuError } from './errors.js';
// Normalized menu model: coercion, validation, label and href helpers.

const CTRL = /[\u0000-\u0020\u007f]/g;

export function safeHref(href) {
  if (typeof href !== 'string') return null;
  const m = /^[a-z][a-z0-9+.-]*:/.exec(href.replace(CTRL, '').toLowerCase());
  return m && !['http:', 'https:', 'mailto:', 'tel:'].includes(m[0]) ? null : href.trim();
}

export function pickLabel(map = {}, lang, fallbackLang) {
  const l = String(lang || '').toLowerCase();
  const k = [l, l.split('-')[0], String(fallbackLang || '').toLowerCase(), ''].find((x) => x in map);
  return k === undefined ? Object.values(map)[0] ?? '' : map[k];
}

const isObj = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const arr = (v) => (Array.isArray(v) ? v : []);
const bool = (v) => v === true || v === 'true' || v === 1 || v === '1';
const num = (v) => (v == null ? null : Number(v));
const href = (v) => (v == null || String(v).trim() === '' ? null : String(v));
const orElse = (map, d) => (Object.keys(map).length ? map : d);

function langMap(v) {
  if (typeof v === 'string' || typeof v === 'number') return { '': String(v) };
  const out = {};
  if (isObj(v)) for (const k of Object.keys(v)) out[k.toLowerCase()] = v[k] == null ? '' : String(v[k]);
  return out;
}

function coerceItem(x, id) {
  const o = isObj(x) ? x : {};
  return { id, label: langMap(o.label), href: href(o.href), current: bool(o.current), external: bool(o.external), children: arr(o.children).map((c, i) => coerceItem(c, id + '-' + i)) };
}

function coerceAction(x, i) {
  const o = isObj(x) ? x : {};
  const id = 'a' + i;
  const kind = o.kind === undefined ? 'link' : o.kind;
  if (kind !== 'language') return { id, kind, label: langMap(o.label), href: href(o.href), external: bool(o.external), variant: o.variant === undefined ? 'glass' : o.variant };
  const alternates = arr(o.alternates).map((a) => {
    const p = isObj(a) ? a : {};
    return { lang: p.lang == null ? '' : String(p.lang), label: langMap(p.label), href: href(p.href), current: bool(p.current) };
  });
  return { id, kind, alternates };
}

function coerceLogo(x) {
  if (!isObj(x)) return null;
  const src = href(x.src);
  return { href: href(x.href), src, srcCompact: href(x.srcCompact) || src, alt: orElse(langMap(x.alt), { '': 'Home' }), width: num(x.width), height: num(x.height) };
}

function coerce(input) {
  const o = isObj(input) ? input : {};
  const opt = isObj(o.options) ? o.options : {};
  return {
    version: o.version === undefined ? 1 : o.version,
    lang: o.lang ? String(o.lang) : 'en',
    ariaLabel: orElse(langMap(o.ariaLabel), { '': 'Main' }),
    options: { breakpoint: num(opt.breakpoint) ?? 1024, collapseOffset: num(opt.collapseOffset) ?? 200 },
    logo: coerceLogo(o.logo),
    items: arr(o.items).map((x, i) => coerceItem(x, 'i' + i)),
    actions: arr(o.actions).map(coerceAction),
  };
}

export function validate(model) {
  const errors = [];
  const m = model || {};
  const err = (path, message) => errors.push({ path, message });
  const label = (map, path) => {
    const v = Object.values(map || {});
    if (!v.length || v.some((x) => String(x).trim() === '')) err(path, 'empty label');
  };
  const url = (v, path, required) => {
    if (v == null) required && err(path, 'href required');
    else if (safeHref(v) === null) err(path, 'unsafe href');
  };
  const items = (list, base, depth) =>
    list.forEach((it, i) => {
      const p = base + '[' + i + ']';
      label(it.label, p + '.label');
      url(it.href, p + '.href', !it.children.length);
      if (it.children.length && depth) err(p + '.children', 'too deep');
      else items(it.children, p + '.children', depth + 1);
    });
  if (m.version !== 1) err('version', 'unsupported version');
  const o = m.options || {};
  for (const k of ['breakpoint', 'collapseOffset']) if (!(Number.isInteger(o[k]) && o[k] > 0)) err('options.' + k, 'invalid');
  if (m.logo) for (const k of ['href', 'src', 'srcCompact']) url(m.logo[k], 'logo.' + k, true);
  items(m.items || [], 'items', 0);
  (m.actions || []).forEach((a, i) => {
    const p = 'actions[' + i + ']';
    if (a.kind === 'link') {
      label(a.label, p + '.label');
      url(a.href, p + '.href', true);
      if (a.variant !== 'glass' && a.variant !== 'solid') err(p + '.variant', 'bad variant');
    } else if (a.kind === 'language') {
      if (!a.alternates.length) err(p + '.alternates', 'none');
      a.alternates.forEach((x, j) => {
        label(x.label, p + '.alternates[' + j + '].label');
        url(x.href, p + '.alternates[' + j + '].href', true);
      });
    } else err(p + '.kind', 'bad kind');
  });
  return errors;
}

export function normalize(input) {
  const model = coerce(input);
  const errors = validate(model);
  if (errors.length) throw new MenuError('Invalid menu: ' + errors.map((e) => e.path + ': ' + e.message).join('; '), errors);
  return model;
}
