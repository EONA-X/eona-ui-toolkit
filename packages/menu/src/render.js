// Model to HTML string. Every text node and attribute value is escaped.
import { pickLabel } from './model.js';

const ESC = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

export function escapeHtml(v) {
  return String(v).replace(/[&<>"']/g, (c) => ESC[c]);
}

const tag = (name, a, inner) =>
  '<' + name + Object.keys(a).filter((k) => a[k] != null).map((k) => ' ' + k + (a[k] === true ? '' : '="' + escapeHtml(a[k]) + '"')).join('') + '>' + (inner === undefined ? '' : inner + '</' + name + '>');

const urlPath = (u) => {
  const s = String(u).replace(/^[a-z][a-z0-9+.-]*:\/\/[^/?#]*/i, '').replace(/[?#].*$/, '');
  return s === '' ? '/' : s.endsWith('/') || /\.\w+$/.test(s.slice(s.lastIndexOf('/') + 1)) ? s : s + '/';
};

export function renderMenuHtml(model, opts) {
  const o = opts || {};
  const lang = o.lang === undefined ? model.lang : o.lang;
  const p = o.idPrefix || 'eona-menu';
  const lb = o.labels || {};
  const pick = (map) => pickLabel(map, lang, model.lang);
  const here = o.currentUrl ? urlPath(o.currentUrl) : null;
  const isCurrent = (it) => it.current || (here !== null && it.href !== null && !/^[a-z][a-z0-9+.-]*:/i.test(it.href) && urlPath(it.href) === here);
  const ext = (x) => (x ? { target: '_blank', rel: 'noopener noreferrer' } : {});
  const link = (cls, it, current) => tag('a', { class: cls, href: it.href, ...ext(it.external), 'aria-current': current ? 'page' : null }, escapeHtml(pick(it.label)));
  const chevron = tag('span', { class: 'eona-menu__chevron', 'aria-hidden': 'true' }, '');
  const button = (cls, controls, inner, extra) => tag('button', { class: cls, type: 'button', 'aria-expanded': 'false', 'aria-controls': controls, ...extra }, inner);
  const sub = (id, lis) => tag('ul', { class: 'eona-menu__sub', id }, lis);

  const item = (it) => {
    if (!it.children.length) return tag('li', { class: 'eona-menu__item' }, link('site-header__link', it, isCurrent(it)));
    const sid = p + '-sub-' + it.id;
    const head = it.href === null
      ? button('site-header__link eona-menu__disclosure', sid, escapeHtml(pick(it.label)) + chevron)
      : link('site-header__link', it, isCurrent(it)) + button('eona-menu__disclosure eona-menu__disclosure--icon', sid, chevron, { 'aria-label': (lb.submenu || 'Submenu') + ': ' + pick(it.label) });
    const cls = 'eona-menu__item eona-menu__item--parent' + (it.children.some(isCurrent) ? ' is-current-ancestor' : '');
    return tag('li', { class: cls }, head + sub(sid, it.children.map((c) => tag('li', {}, link('eona-menu__sublink', c, isCurrent(c)))).join('')));
  };

  const action = (a, suffix) => {
    if (a.kind === 'link') return tag('a', { class: 'pill-btn pill-btn-' + (a.variant === 'solid' ? 'solid' : 'glass-light'), href: a.href, ...ext(a.external) }, escapeHtml(pick(a.label)));
    const cur = a.alternates.find((x) => x.current) || a.alternates.find((x) => x.lang.toLowerCase() === String(lang).toLowerCase());
    const sid = p + '-sub-' + a.id + suffix;
    const lis = a.alternates.map((x) => tag('li', {}, tag('a', { class: 'eona-menu__sublink', href: x.href, hreflang: x.lang, lang: x.lang, 'aria-current': x === cur ? 'true' : null }, escapeHtml(pick(x.label))))).join('');
    return tag('div', { class: 'eona-menu__lang' }, button('site-header__lang eona-menu__disclosure', sid, escapeHtml((cur ? cur.lang : lang).toUpperCase())) + sub(sid, lis));
  };
  const actions = (where, suffix) => tag('div', { class: 'site-header__actions site-header__actions--' + where }, model.actions.map((a) => action(a, suffix)).join(''));
  const g = model.logo;
  const logo = (kind, src, width, height) => (g ? tag('a', { class: 'site-header__logo site-header__logo--' + kind, href: g.href, 'aria-label': pick(g.alt) }, tag('img', { src, alt: '', width, height })) : '');
  const panel = p + '-panel';

  return tag('header', { class: 'site-header eona-menu', 'data-eona-menu': true, 'data-breakpoint': model.options.breakpoint, 'data-collapse-offset': model.options.collapseOffset },
    tag('div', { class: 'wrap site-header__wrap' },
      logo('top', g && g.src, g && g.width, g && g.height)
      + tag('button', { class: 'eona-menu__toggle', type: 'button', 'aria-expanded': 'false', 'aria-controls': panel, 'aria-label': lb.menu || 'Menu' }, tag('span', { class: 'eona-menu__bars', 'aria-hidden': 'true' }, ''))
      + tag('nav', { class: 'site-header__pill eona-menu__panel', id: panel, 'aria-label': pick(model.ariaLabel) },
        logo('pill', g && g.srcCompact, 28, 27) + tag('ul', { class: 'eona-menu__list' }, model.items.map(item).join('')) + actions('pill', '-pill'))
      + actions('top', '')));
}
