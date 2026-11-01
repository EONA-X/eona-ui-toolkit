import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { renderMenuHtml, escapeHtml } from '../src/render.js';
import { normalize } from '../src/model.js';

const fixture = JSON.parse(readFileSync(new URL('./fixtures/eona-x.model.json', import.meta.url), 'utf8'));
const dom = (html) => new JSDOM('<body>' + html + '</body>').window.document;
const render = (m = fixture, o) => dom(renderMenuHtml(m, o));

test('escapeHtml', () => {
  assert.equal(escapeHtml(`&<>"'`), '&amp;&lt;&gt;&quot;&#39;');
  assert.equal(escapeHtml(5), '5');
});

test('root structure', () => {
  const d = render();
  const h = d.querySelector('header.site-header.eona-menu[data-eona-menu]');
  assert.ok(h);
  assert.equal(h.dataset.breakpoint, '1024');
  assert.equal(h.dataset.collapseOffset, '200');
  assert.ok(d.querySelector('.wrap.site-header__wrap'));
  assert.equal(d.querySelectorAll('.site-header__logo').length, 2);
  assert.ok(d.querySelector('a.site-header__logo--top img[src="/logo/logo-transparent.svg"][width="38"][height="36"]'));
  assert.equal(d.querySelector('a.site-header__logo--top').getAttribute('aria-label'), 'EONA-X');
  const tg = d.querySelector('button.eona-menu__toggle');
  assert.equal(tg.getAttribute('aria-expanded'), 'false');
  assert.equal(tg.getAttribute('aria-controls'), 'eona-menu-panel');
  assert.equal(tg.getAttribute('aria-label'), 'Menu');
  const nav = d.querySelector('nav.site-header__pill.eona-menu__panel#eona-menu-panel');
  assert.equal(nav.getAttribute('aria-label'), 'Navigation principale');
  assert.ok(nav.querySelector('.site-header__logo--pill'));
  assert.ok(d.querySelector('.site-header__actions--pill'));
  assert.ok(d.querySelector('.site-header__actions--top'));
});

test('null logo', () => {
  const d = render({ ...fixture, logo: null });
  assert.equal(d.querySelectorAll('.site-header__logo').length, 0);
});

test('current leaf', () => {
  const d = render();
  assert.equal(d.querySelector('a[href="/fr/cas-etude/"]').getAttribute('aria-current'), 'page');
  assert.equal(d.querySelector('a[href="/fr/association/"]').hasAttribute('aria-current'), false);
});

test('currentUrl matching and ancestor', () => {
  const d = render(fixture, { currentUrl: 'https://eona-x.eu/fr/actualites?x=1#h' });
  assert.equal(d.querySelector('a[href="/fr/actualites/"]').getAttribute('aria-current'), 'page');
  const li = d.querySelector('.eona-menu__item--parent');
  assert.ok(li.classList.contains('is-current-ancestor'));
  assert.equal(li.querySelector('button').hasAttribute('aria-current'), false);
  const d2 = render(fixture, { currentUrl: '/fr/association' });
  assert.equal(d2.querySelector('a[href="/fr/association/"]').getAttribute('aria-current'), 'page');
});

test('parent without href', () => {
  const d = render();
  const b = d.querySelector('button.site-header__link.eona-menu__disclosure');
  assert.equal(b.getAttribute('aria-expanded'), 'false');
  const ul = d.getElementById(b.getAttribute('aria-controls'));
  assert.equal(ul.tagName, 'UL');
  assert.ok(ul.classList.contains('eona-menu__sub'));
  assert.equal(ul.querySelectorAll('a.eona-menu__sublink').length, 2);
  assert.ok(b.querySelector('.eona-menu__chevron'));
});

test('parent with href', () => {
  const m = normalize({ items: [{ label: 'P', href: '/p', children: [{ label: 'C', href: '/c' }] }] });
  const d = render(m);
  const li = d.querySelector('.eona-menu__item--parent');
  assert.ok(li.querySelector('a.site-header__link[href="/p"]'));
  const b = li.querySelector('button.eona-menu__disclosure--icon');
  assert.equal(b.getAttribute('aria-label'), 'Submenu: P');
  assert.ok(d.getElementById(b.getAttribute('aria-controls')));
});

test('external links', () => {
  const a = render().querySelector('a[href="https://docs.eona-x.eu/"]');
  assert.equal(a.getAttribute('target'), '_blank');
  assert.equal(a.getAttribute('rel'), 'noopener noreferrer');
});

test('actions twice, unique ids', () => {
  const d = render();
  const ids = [...d.querySelectorAll('[id]')].map((e) => e.id);
  assert.equal(new Set(ids).size, ids.length);
  assert.ok(d.querySelector('.site-header__actions--top #eona-menu-sub-a0'));
  assert.ok(d.querySelector('.site-header__actions--pill #eona-menu-sub-a0-pill'));
  assert.equal(d.querySelectorAll('a.pill-btn').length, 2);
});

test('link action variants', () => {
  assert.ok(render().querySelector('a.pill-btn.pill-btn-glass-light[href="https://portal.eona-x.eu/login"]'));
  const m = normalize({ actions: [{ label: 'x', href: '/x', variant: 'solid' }] });
  assert.ok(render(m).querySelector('a.pill-btn.pill-btn-solid'));
});

test('language switcher', () => {
  const d = render();
  const b = d.querySelector('.eona-menu__lang button.site-header__lang.eona-menu__disclosure');
  assert.equal(b.textContent, 'FR');
  const en = d.querySelector('.eona-menu__lang a[hreflang="en"]');
  assert.equal(en.getAttribute('lang'), 'en');
  assert.equal(en.textContent, 'English');
  assert.equal(d.querySelector('.eona-menu__lang a[hreflang="fr"]').getAttribute('aria-current'), 'true');
});

test('lang option and subtag fallback', () => {
  const en = render(fixture, { lang: 'en' });
  assert.ok([...en.querySelectorAll('.site-header__link')].some((a) => a.textContent === 'Association'));
  assert.equal(en.querySelector('nav').getAttribute('aria-label'), 'Main navigation');
  const ca = render(fixture, { lang: 'fr-CA' });
  assert.ok([...ca.querySelectorAll('.site-header__link')].some((a) => a.textContent === "L'association"));
});

test('xss', () => {
  const m = normalize({ items: [{ label: '<img src=x onerror=alert(1)>', href: '/a" onmouseover="x' }] });
  const d = render(m);
  assert.equal(d.querySelectorAll('img').length, 0);
  const a = d.querySelector('.site-header__link');
  assert.equal(a.textContent, '<img src=x onerror=alert(1)>');
  assert.equal(a.getAttribute('href'), '/a" onmouseover="x');
  assert.equal(a.hasAttribute('onmouseover'), false);
});

test('deterministic, no inter-tag whitespace', () => {
  const a = renderMenuHtml(fixture);
  assert.equal(a, renderMenuHtml(JSON.parse(JSON.stringify(fixture))));
  assert.equal(/>\s+</.test(a), false);
  assert.ok(a.startsWith('<header class="site-header eona-menu" data-eona-menu data-breakpoint="1024" data-collapse-offset="200"><div class="wrap site-header__wrap"><a class="site-header__logo site-header__logo--top" href="/fr/" aria-label="EONA-X"><img src="/logo/logo-transparent.svg" alt="" width="38" height="36"></a>'));
});
