// Without JavaScript (no .is-enhanced) the menu must stay reachable below the breakpoint.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const css = readFileSync(new URL('../menu.css', import.meta.url), 'utf8');
const start = css.indexOf('@media (max-width:1023px){');
const end = css.indexOf('@media (prefers-reduced-motion');
const mobile = css.slice(start + '@media (max-width:1023px){'.length, end).replace(/\}\s*$/, '');
const rules = mobile.split('}').map((r) => r.trim()).filter(Boolean).map((r) => {
  const i = r.indexOf('{');
  return { selectors: r.slice(0, i).split(',').map((s) => s.trim()), body: r.slice(i + 1) };
});
const staticRules = rules.filter((r) => r.selectors.every((s) => s.includes(':not(.is-enhanced)')));
const gated = (s) => s.includes('.is-enhanced') && !s.includes(':not(.is-enhanced)');

test('the mobile block has a no-JS static layout', () => {
  assert.ok(staticRules.length > 0);
});

test('drawer, toggle and hiding rules below the breakpoint are scoped to .is-enhanced', () => {
  const risky = /(__toggle|__bars|__panel|__list|__item|__disclosure|__sub|__lang|site-header__actions|site-header__logo|site-header__wrap)/;
  for (const r of rules) {
    if (staticRules.includes(r)) continue;
    for (const s of r.selectors) {
      if (risky.test(s)) assert.ok(gated(s), 'not gated on .is-enhanced: ' + s);
    }
  }
});

test('no-JS: the panel stays in the flow and is visible', () => {
  const panel = staticRules.find((r) => r.selectors.some((s) => s.endsWith('.eona-menu__panel')));
  assert.ok(panel, 'missing :not(.is-enhanced) panel rule');
  assert.match(panel.body, /position:static/);
  assert.doesNotMatch(panel.body, /visibility:hidden|translateX/);
});

test('no-JS: sublinks are expanded inline', () => {
  const sub = staticRules.find((r) => r.selectors.some((s) => /__sub$/.test(s)));
  assert.ok(sub, 'missing :not(.is-enhanced) sub rule');
  assert.match(sub.body, /display:block/);
  assert.match(sub.body, /position:static/);
});

test('no-JS: the dead toggle and disclosure buttons are hidden, the top actions are not', () => {
  const hidden = staticRules.filter((r) => /display:none/.test(r.body)).flatMap((r) => r.selectors).join(' ');
  assert.match(hidden, /__toggle/);
  assert.match(hidden, /__disclosure--icon/);
  assert.match(hidden, /__lang ?> ?\.eona-menu__disclosure/);
  assert.doesNotMatch(hidden, /actions--top/);
});

test('the toggle stays hidden by default and shown only when enhanced', () => {
  assert.match(css, /\.eona-menu \.eona-menu__toggle\{display:none\}/);
  const shown = rules.find((r) => /display:inline-flex/.test(r.body) && r.selectors.some((s) => /__toggle/.test(s)));
  assert.ok(shown && shown.selectors.every(gated));
});

test('no-JS: the header takes its place in the flow, so the taller menu does not cover the page', () => {
  const root = staticRules.find((r) => r.selectors.some((s) => s === '.eona-menu:not(.is-enhanced)'));
  assert.ok(root, 'missing .eona-menu:not(.is-enhanced) rule');
  assert.match(root.body, /position:relative/);
});
