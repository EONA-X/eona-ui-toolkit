// Load a model and mount it into the page.
import { MenuError } from './errors.js';
import { normalize } from './model.js';
import { fromJsonLd } from './jsonld.js';
import { renderMenuHtml } from './render.js';
import { enhance } from './behavior.js';

const MOUNT_SERVER = '[data-eona-menu]:not(script)';

function mountToModel(body) {
  return Array.isArray(body) || '@context' in Object(body) || '@graph' in Object(body) ? fromJsonLd(body) : normalize(body);
}

export const loadMenu = async (url, opts) => {
  const o = opts || {};
  const f = o.fetch || fetch;
  const res = await f(url, { headers: { Accept: 'application/ld+json, application/json;q=0.9' }, credentials: 'same-origin', signal: o.signal });
  if (!res.ok) throw new MenuError('HTTP ' + res.status + ' loading ' + url);
  return mountToModel(await res.json());
};

export const mountMenu = async (target, source, opts) => {
  const o = opts || {};
  try {
    const doc = o.window ? o.window.document : document;
    const el = typeof target === 'string' ? doc.querySelector(target) : target;
    if (!el) throw new MenuError('Menu target not found: ' + target);
    const win = o.window || el.ownerDocument.defaultView;
    if (source === undefined) {
      const existing = el.matches(MOUNT_SERVER) ? el : el.querySelector(MOUNT_SERVER);
      if (!existing) throw new MenuError('No menu source and no server-rendered menu in target');
      return enhance(existing, { ...o, window: win });
    }
    const model = typeof source === 'string' ? await loadMenu(source, o) : mountToModel(source);
    el.innerHTML = renderMenuHtml(model, o);
    return enhance(el.querySelector(MOUNT_SERVER), { ...o, window: win });
  } catch (e) {
    console.error(e);
    throw e instanceof MenuError ? e : new MenuError(String(e?.message ?? e));
  }
};
