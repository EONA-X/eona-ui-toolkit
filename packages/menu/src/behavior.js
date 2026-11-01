// Progressive enhancement: scroll collapse, disclosures, mobile drawer.
const BEH_BOX = '.eona-menu__item--parent, .eona-menu__lang';
const BEH_DISC = '.eona-menu__disclosure';
const BEH_FOCUS = 'a[href], button:not([disabled])';

export function enhance(root, opts) {
  const o = opts || {};
  const win = o.window || window;
  const doc = root.ownerDocument;
  const html = doc.documentElement;
  const bp = Number(o.breakpoint ?? root.getAttribute('data-breakpoint')) || 1024;
  const offset = Number(o.collapseOffset ?? root.getAttribute('data-collapse-offset')) || 200;
  const delay = o.hoverDelay ?? 150;
  const media = (q) => (win.matchMedia ? win.matchMedia(q) : { matches: true });
  const mq = media('(min-width: ' + bp + 'px)');
  const hoverMq = media('(hover: hover)');
  const toggle = root.querySelector('.eona-menu__toggle');
  const panel = root.querySelector('.eona-menu__panel');
  const undo = [];
  const timers = new Map();
  const on = (t, type, fn, opt) => {
    t.addEventListener(type, fn, opt);
    undo.push(() => t.removeEventListener(type, fn, opt));
  };
  const discs = () => [...root.querySelectorAll(BEH_DISC)];
  const isOpen = (b) => b.getAttribute('aria-expanded') === 'true';
  const setExp = (b, v) => {
    b.setAttribute('aria-expanded', v ? 'true' : 'false');
    b.closest(BEH_BOX).classList.toggle('is-open', v);
  };
  const closeAll = (except) => discs().forEach((b) => b !== except && isOpen(b) && setExp(b, false));
  const drawerOpen = () => root.classList.contains('is-open');

  let frame = false;
  const update = () => {
    root.classList.toggle('is-scrolled', Boolean(mq.matches) && (win.scrollY || 0) > offset);
  };
  const onScroll = () => {
    if (!win.requestAnimationFrame) return update();
    if (frame) return;
    frame = true;
    win.requestAnimationFrame(() => {
      frame = false;
      update();
    });
  };

  const focusables = () =>
    panel
      ? [...panel.querySelectorAll(BEH_FOCUS)].filter((el) => {
          const sub = el.closest('.eona-menu__sub');
          return !sub || isOpen(sub.previousElementSibling);
        })
      : [];
  const setOpen = (v) => {
    root.classList.toggle('is-open', v);
    html.classList.toggle('eona-menu-lock', v);
    toggle?.setAttribute('aria-expanded', v ? 'true' : 'false');
    if (v) focusables()[0]?.focus();
    else closeAll();
  };

  const onMq = () => {
    update();
    if (mq.matches) setOpen(false);
  };
  if (mq.addEventListener) on(mq, 'change', onMq);
  on(win, 'scroll', onScroll, { passive: true });

  on(root, 'click', (e) => {
    const t = e.target;
    const b = t.closest(BEH_DISC);
    if (b) {
      const v = !isOpen(b);
      closeAll(b);
      setExp(b, v);
      return;
    }
    if (toggle && toggle.contains(t)) setOpen(!drawerOpen());
    else if (drawerOpen() && panel && t.closest('a[href]') && panel.contains(t)) setOpen(false);
  });

  const hoverOk = () => mq.matches && hoverMq.matches;
  root.querySelectorAll(BEH_BOX).forEach((box) => {
    const b = box.querySelector(BEH_DISC);
    const hover = (type, fn) =>
      on(box, type, () => {
        if (!hoverOk()) return;
        clearTimeout(timers.get(box));
        fn();
      });
    hover('mouseenter', () => {
      closeAll(b);
      setExp(b, true);
    });
    hover('mouseleave', () => timers.set(box, setTimeout(() => setExp(b, false), delay)));
  });

  const subLinks = (ul) => [...ul.querySelectorAll('.eona-menu__sublink')];
  on(root, 'keydown', (e) => {
    const t = e.target;
    const k = e.key;
    const box = t.closest(BEH_BOX);
    const b = box && box.querySelector(BEH_DISC);
    if (k === 'Escape') {
      if (b && isOpen(b)) {
        setExp(b, false);
        b.focus();
        return;
      }
      if (drawerOpen()) {
        setOpen(false);
        toggle?.focus();
      }
      return;
    }
    if (k === 'ArrowDown' && t.matches(BEH_DISC)) {
      e.preventDefault();
      closeAll(t);
      setExp(t, true);
      subLinks(box)[0]?.focus();
      return;
    }
    if (t.matches('.eona-menu__sublink')) {
      const list = subLinks(t.closest('.eona-menu__sub'));
      const i = list.indexOf(t);
      const to = { ArrowDown: list[(i + 1) % list.length], ArrowUp: list[(i - 1 + list.length) % list.length], Home: list[0], End: list[list.length - 1] }[k];
      if (to) {
        e.preventDefault();
        to.focus();
      }
      return;
    }
    if (k === 'Tab' && drawerOpen() && !mq.matches && toggle) {
      const f = focusables();
      if (!e.shiftKey && f.length && t === f[f.length - 1]) {
        e.preventDefault();
        toggle.focus();
      } else if (e.shiftKey && t === toggle && f.length) {
        e.preventDefault();
        f[f.length - 1].focus();
      }
    }
  });

  on(root, 'focusout', (e) => {
    const box = e.target.closest(BEH_BOX);
    const b = box && box.querySelector(BEH_DISC);
    if (b && isOpen(b) && e.relatedTarget && !box.contains(e.relatedTarget)) setExp(b, false);
  });
  on(doc, 'pointerdown', (e) => {
    if (!root.contains(e.target)) closeAll();
  });

  root.classList.add('is-enhanced');
  update();

  return {
    destroy() {
      undo.splice(0).forEach((f) => f());
      timers.forEach((id) => clearTimeout(id));
      timers.clear();
      discs().forEach((b) => setExp(b, false));
      toggle?.setAttribute('aria-expanded', 'false');
      html.classList.remove('eona-menu-lock');
      root.classList.remove('is-enhanced', 'is-open', 'is-scrolled');
    },
    closeAll: () => closeAll(),
    setOpen,
    update,
  };
}
