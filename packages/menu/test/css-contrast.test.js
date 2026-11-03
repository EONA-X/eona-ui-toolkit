// The chips must stay legible over any page background: they cannot be white-on-white.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const css = readFileSync(new URL('../menu.css', import.meta.url), 'utf8');
const rule = (sel) => css.split('}').find((r) => r.includes(sel + '{') || r.includes(sel + ',')) || '';
const mobileToggle = css.split('}').find((r) => r.includes('.eona-menu__toggle{display:inline-flex')) || '';

test('chip background is a themable navy glass, not translucent white', () => {
  assert.match(css, /--eona-menu-chip/);
  assert.match(css, /rgba\(9,\s*27,\s*59/);
});

test('scrolled pill and language chip use the chip background', () => {
  const r = rule('.eona-menu.is-scrolled .site-header__pill');
  assert.match(r, /var\(--eona-menu-chip/);
  assert.match(rule('.eona-menu.is-scrolled .site-header__lang'), /var\(--eona-menu-chip/);
});

test('mobile toggle uses the chip background', () => {
  assert.match(mobileToggle, /var\(--eona-menu-chip/);
});
