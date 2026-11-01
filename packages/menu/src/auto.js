// Bundle-only auto-mount. Reads script data-* attributes.
import { mountMenu, loadMenu } from './mount.js';
import { renderMenuHtml } from './render.js';
import { normalize } from './model.js';
import { fromJsonLd } from './jsonld.js';
import { enhance } from './behavior.js';
const AUTO_VERSION = '0.9.2';
const autoScript = typeof document !== 'undefined' ? document.currentScript : null;

const autoAttr = (name) => autoScript && autoScript.getAttribute(name);

function autoRun() {
  const inline = document.querySelector('script[type="application/ld+json"][data-eona-menu]');
  const config = typeof __EONA_MENU_CONFIG__ === 'undefined' ? null : __EONA_MENU_CONFIG__;
  const opts = {};
  const lang = autoAttr('data-lang') || document.documentElement.lang;
  if (lang) opts.lang = lang;
  if (autoAttr('data-current') === 'auto') opts.currentUrl = location.pathname;
  const source = autoAttr('data-src') || (inline && JSON.parse(inline.textContent)) || config;
  if (!source) {
    const existing = document.querySelector('[data-eona-menu]:not(script)');
    if (existing) mountMenu(existing, undefined, opts).catch(() => {});
    return;
  }
  const sel = autoAttr('data-target');
  let target = document.querySelector(sel || '[data-eona-menu-root]');
  if (!target && !sel) {
    target = document.createElement('div');
    target.setAttribute('data-eona-menu-root', '');
    document.body.insertBefore(target, document.body.firstChild);
  }
  mountMenu(target || sel, source, opts).catch(() => {});
}

if (typeof window !== 'undefined') {
  window.EonaMenu = { mountMenu, loadMenu, renderMenuHtml, normalize, fromJsonLd, enhance, version: AUTO_VERSION };
  if (autoScript === null || !autoScript.hasAttribute('data-manual')) {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', autoRun);
    else autoRun();
  }
}
