import test, { mock } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { renderMenuHtml } from '../src/render.js';
import { enhance } from '../src/behavior.js';

const fixture = JSON.parse(readFileSync(new URL('./fixtures/eona-x.model.json', import.meta.url), 'utf8'));

function setup({ desktop = true, hover = true, scrollY = 0 } = {}) {
  const dom = new JSDOM('<!doctype html><html><body>' + renderMenuHtml(fixture) + '<button id="out">o</button></body></html>', { pretendToBeVisual: false });
  const win = dom.window;
  const lists = new Map();
  const state = { desktop, hover };
  const mk = (q) => {
    const l = { matches: false, ls: new Set(), addEventListener(_, f) { this.ls.add(f); }, removeEventListener(_, f) { this.ls.delete(f); } };
    lists.set(q, l);
    return l;
  };
  win.matchMedia = (q) => {
    const l = lists.get(q) || mk(q);
    l.matches = q.includes('hover') ? state.hover : state.desktop;
    return l;
  };
  win.scrollY = scrollY;
  win.requestAnimationFrame = undefined;
  const root = win.document.querySelector('[data-eona-menu]');
  const setDesktop = (v) => {
    state.desktop = v;
    for (const [q, l] of lists) if (q.includes('min-width')) { l.matches = v; [...l.ls].forEach((f) => f({ matches: v })); }
  };
  return { win, doc: win.document, root, state, setDesktop, enh: (o = {}) => enhance(root, { window: win, ...o }) };
}
const ev = (win, el, type, init = {}) => el.dispatchEvent(new win.Event(type, { bubbles: true, cancelable: true, ...init }));
const key = (win, el, k, extra = {}) => { const e = new win.KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...extra }); el.dispatchEvent(e); return e; };
const disc = (root) => root.querySelector('.eona-menu__item--parent .eona-menu__disclosure');

test('enhanced + destroy', () => {
  const s = setup({ scrollY: 300 });
  const c = s.enh();
  assert.ok(s.root.classList.contains('is-enhanced'));
  assert.ok(s.root.classList.contains('is-scrolled'));
  disc(s.root).click();
  c.destroy();
  assert.equal(s.root.classList.contains('is-enhanced'), false);
  assert.equal(s.root.classList.contains('is-scrolled'), false);
  const b = disc(s.root);
  b.setAttribute('aria-expanded', 'false');
  b.click();
  assert.equal(b.getAttribute('aria-expanded'), 'false');
  s.win.scrollY = 0;
  ev(s.win, s.win, 'scroll');
});

test('scroll collapse', () => {
  const s = setup({ scrollY: 200 });
  s.enh();
  assert.equal(s.root.classList.contains('is-scrolled'), false);
  s.win.scrollY = 201; ev(s.win, s.win, 'scroll');
  assert.ok(s.root.classList.contains('is-scrolled'));
  s.setDesktop(false);
  assert.equal(s.root.classList.contains('is-scrolled'), false);
  ev(s.win, s.win, 'scroll');
  assert.equal(s.root.classList.contains('is-scrolled'), false);
  s.setDesktop(true);
  assert.ok(s.root.classList.contains('is-scrolled'));
  const m = setup({ desktop: false, scrollY: 500 });
  m.enh();
  assert.equal(m.root.classList.contains('is-scrolled'), false);
});

test('click toggles and closes siblings', () => {
  const s = setup();
  s.enh();
  const b = disc(s.root);
  b.click();
  assert.equal(b.getAttribute('aria-expanded'), 'true');
  assert.ok(b.closest('li').classList.contains('is-open'));
  const sw = s.root.querySelector('.eona-menu__lang button');
  sw.click();
  assert.equal(b.getAttribute('aria-expanded'), 'false');
  assert.equal(b.closest('li').classList.contains('is-open'), false);
  assert.equal(sw.getAttribute('aria-expanded'), 'true');
  sw.click();
  assert.equal(sw.getAttribute('aria-expanded'), 'false');
});

test('hover open, delayed close, re-enter cancels', () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  try {
    const s = setup();
    s.enh({ hoverDelay: 150 });
    const li = disc(s.root).closest('li');
    ev(s.win, li, 'mouseenter', { bubbles: false });
    assert.equal(disc(s.root).getAttribute('aria-expanded'), 'true');
    ev(s.win, li, 'mouseleave', { bubbles: false });
    mock.timers.tick(100);
    assert.equal(disc(s.root).getAttribute('aria-expanded'), 'true');
    ev(s.win, li, 'mouseenter', { bubbles: false });
    mock.timers.tick(500);
    assert.equal(disc(s.root).getAttribute('aria-expanded'), 'true');
    ev(s.win, li, 'mouseleave', { bubbles: false });
    mock.timers.tick(150);
    assert.equal(disc(s.root).getAttribute('aria-expanded'), 'false');
  } finally { mock.timers.reset(); }
});

test('hover ignored on mobile and without hover', () => {
  for (const o of [{ desktop: false }, { hover: false }]) {
    const s = setup(o);
    s.enh();
    ev(s.win, disc(s.root).closest('li'), 'mouseenter', { bubbles: false });
    assert.equal(disc(s.root).getAttribute('aria-expanded'), 'false');
  }
});

test('escape closes and focuses button', () => {
  const s = setup();
  s.enh();
  const b = disc(s.root);
  b.click();
  const link = b.closest('li').querySelector('.eona-menu__sublink');
  link.focus();
  key(s.win, link, 'Escape');
  assert.equal(b.getAttribute('aria-expanded'), 'false');
  assert.equal(s.doc.activeElement, b);
});

test('arrow navigation', () => {
  const s = setup();
  s.enh();
  const b = disc(s.root);
  const links = [...b.closest('li').querySelectorAll('.eona-menu__sublink')];
  key(s.win, b, 'ArrowDown');
  assert.equal(b.getAttribute('aria-expanded'), 'true');
  assert.equal(s.doc.activeElement, links[0]);
  key(s.win, links[0], 'ArrowDown');
  assert.equal(s.doc.activeElement, links[1]);
  key(s.win, links[1], 'ArrowDown');
  assert.equal(s.doc.activeElement, links[0]);
  key(s.win, links[0], 'ArrowUp');
  assert.equal(s.doc.activeElement, links[1]);
  key(s.win, links[1], 'Home');
  assert.equal(s.doc.activeElement, links[0]);
  key(s.win, links[0], 'End');
  assert.equal(s.doc.activeElement, links[1]);
});

test('focusout and outside pointerdown', () => {
  const s = setup();
  s.enh();
  const b = disc(s.root);
  b.click();
  const out = s.doc.getElementById('out');
  b.dispatchEvent(new s.win.FocusEvent('focusout', { bubbles: true, relatedTarget: out }));
  assert.equal(b.getAttribute('aria-expanded'), 'false');
  b.click();
  b.dispatchEvent(new s.win.FocusEvent('focusout', { bubbles: true, relatedTarget: b.closest('li').querySelector('a') }));
  assert.equal(b.getAttribute('aria-expanded'), 'true');
  ev(s.win, s.root, 'pointerdown');
  assert.equal(b.getAttribute('aria-expanded'), 'true');
  ev(s.win, out, 'pointerdown');
  assert.equal(b.getAttribute('aria-expanded'), 'false');
});

test('mobile toggle, focus, lock', () => {
  const s = setup({ desktop: false });
  const c = s.enh();
  const t = s.root.querySelector('.eona-menu__toggle');
  t.click();
  assert.equal(t.getAttribute('aria-expanded'), 'true');
  assert.ok(s.root.classList.contains('is-open'));
  assert.ok(s.doc.documentElement.classList.contains('eona-menu-lock'));
  const panel = s.root.querySelector('.eona-menu__panel');
  assert.equal(s.doc.activeElement, panel.querySelector('a, button'));
  t.click();
  assert.equal(s.root.classList.contains('is-open'), false);
  assert.equal(s.doc.documentElement.classList.contains('eona-menu-lock'), false);
  c.setOpen(true);
  assert.ok(s.root.classList.contains('is-open'));
  c.destroy();
  assert.equal(s.root.classList.contains('is-open'), false);
  assert.equal(s.doc.documentElement.classList.contains('eona-menu-lock'), false);
});

test('mobile escape and tab trap', () => {
  const s = setup({ desktop: false });
  s.enh();
  const t = s.root.querySelector('.eona-menu__toggle');
  t.click();
  const panel = s.root.querySelector('.eona-menu__panel');
  const f = [...panel.querySelectorAll('a[href], button')];
  const last = f[f.length - 1];
  last.focus();
  const e = key(s.win, last, 'Tab');
  assert.equal(e.defaultPrevented, true);
  assert.equal(s.doc.activeElement, t);
  const e2 = key(s.win, t, 'Tab', { shiftKey: true });
  assert.equal(e2.defaultPrevented, true);
  assert.equal(s.doc.activeElement, last);
  key(s.win, last, 'Escape');
  assert.equal(s.root.classList.contains('is-open'), false);
  assert.equal(s.doc.activeElement, t);
});

test('link click closes drawer; desktop switch closes', () => {
  const s = setup({ desktop: false });
  s.enh();
  const t = s.root.querySelector('.eona-menu__toggle');
  t.click();
  s.root.querySelector('.eona-menu__panel a[href="/fr/association/"]').addEventListener('click', (e) => e.preventDefault());
  s.root.querySelector('.eona-menu__panel a[href="/fr/association/"]').click();
  assert.equal(s.root.classList.contains('is-open'), false);
  t.click();
  assert.ok(s.root.classList.contains('is-open'));
  s.setDesktop(true);
  assert.equal(s.root.classList.contains('is-open'), false);
  assert.equal(s.doc.documentElement.classList.contains('eona-menu-lock'), false);
  assert.equal(t.getAttribute('aria-expanded'), 'false');
});
