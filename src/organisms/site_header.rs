use yew::prelude::*;

use crate::atoms::{PillButton, PillButtonKind};

#[derive(Clone, PartialEq)]
pub struct SiteNavLink {
    pub href: AttrValue,
    pub label: AttrValue,
    pub current: bool,
}

impl SiteNavLink {
    pub fn new(href: &'static str, label: &'static str, current: bool) -> Self {
        Self { href: href.into(), label: label.into(), current }
    }
}

/// The developer portal's own unified top bar — shared by every page
/// (`index.html`, `icons.html`, `components.html`, ...), matching eona-x.eu's
/// real `#en-tete` structure (confirmed identical across the homepage and
/// both `/fr/actualites/`/`/fr/cas-etude/` listing pages): three separate
/// pieces — a standalone logo, a centered rounded "glass" pill holding the
/// nav links, and a standalone right-side pill for secondary actions
/// (mirroring where the real site puts its language switcher). The logo
/// itself is the real mark (`logo/logo-transparent.svg`, same asset as the
/// Logo section's downloads — see `LogoDownloadCard`), not a placeholder,
/// matching eona-x.eu's own `.header-logo-link`.
///
/// **Fixed position + scroll collapse, reverse-engineered from the real
/// site's own JS** (`assets/frontend/js/components/header.js`, a `Header`
/// class using GSAP/ScrollTrigger — fetched and read directly, not guessed):
/// at rest, THREE pieces are visible (standalone logo, pill nav, standalone
/// actions) while a second, hidden copy of the logo and actions already sits
/// *inside* the pill (`.site-header__logo--pill` /
/// `.site-header__actions--pill` below, mirroring the real site's
/// `.js-logo-menu`/`.js-lang-menu`). Past 200px of scroll (desktop only,
/// ≥1024px — the real site's own thresholds), the standalone pieces fade out
/// and the pill's internal copies fade in, "absorbing" logo and actions into
/// one consolidated bar; scrolling back up reverses it. The header itself is
/// `position: fixed`, resting `top: 2rem` on desktop and snapping to `top: 0`
/// once scrolled (`.is-scrolled`, the real site's own class name) — `body`
/// gets compensating `padding-top` in components.css so fixed positioning
/// doesn't hide page content under it.
///
/// Two honest simplifications, not full parity:
/// - The real background is a *transparent* header with each piece's own
///   frosted-glass chip floating over a photo/hero backdrop; ours is a solid
///   navy strip (no photo backdrop on every page for glass-over-nothing to
///   make sense), so the fixed/scroll *mechanics* are faithful but the visual
///   material differs.
/// - The real transition is a GSAP timeline staggering the two fades by
///   0.1s; ours is plain CSS `transition` on `opacity`/`transform`/`width`
///   toggled by one class (see `crates/site/assets/site-header.js`) — same
///   practical effect, both fades run concurrently rather than staggered,
///   no extra runtime dependency (this project's consistent pattern: small
///   vanilla-JS glue, no framework, `jszip` being the one prior exception
///   because there's no vanilla substitute for zip generation — GSAP has no
///   such excuse here).
///
/// The login `PillButton` is real estate reserved for a future login/profile
/// control, not wired to anything yet — hidden by default (`show_login`
/// defaults to `false`) since a permanently-disabled button is confusing UX
/// with nothing behind it yet. Flip `show_login` to `true` once there's an
/// actual login flow to wire it to; the markup/CSS stay ready either way.
/// The `.site-header__actions*` wrappers always render so the scroll-collapse
/// JS has a stable, always-present pair of elements to animate regardless of
/// `show_login` — and, now, so there's always somewhere for the "EN" mark
/// below to live.
///
/// **"EN"**, in that same slot, is the real site's `.header-lang` language
/// switcher (a flag icon + "FR"/"EN" + a dropdown chevron, inspected
/// directly on eona-x.eu) reduced to only what's still true here: this
/// portal has exactly one language. It's a plain `<span>`, not a `<button>`
/// — deliberately not the interactive `NavDropdown` pattern used for the
/// real site's *own* language menu (see `components.html`'s "Nav dropdowns"
/// section for that pattern, ported faithfully elsewhere for when it's
/// actually needed) — with no click handler, no hover state, no chevron
/// (a chevron specifically signals "opens a menu," which would be false
/// here), and no flag (a flag disambiguates a *choice*; with nothing to
/// choose between it would just be decoration claiming an interactivity
/// that isn't there). `.site-header__lang`'s own chip styling (background,
/// blur, radius) is what `.site-header__actions--top` used to carry as a
/// workaround for staying visible while genuinely empty — moved onto the
/// mark itself so it keeps looking right once it sits next to a future,
/// independently-chipped `show_login` button rather than nesting one chip
/// inside another. Confirmed rendering identically with `show_login` both
/// `true` and `false` — it doesn't read that prop at all.
#[derive(Properties, PartialEq)]
pub struct SiteHeaderProps {
    pub links: Vec<SiteNavLink>,
    #[prop_or_default]
    pub show_login: bool,
}

#[function_component(SiteHeader)]
pub fn site_header(props: &SiteHeaderProps) -> Html {
    html! {
        <header class="site-header">
            <div class="wrap site-header__wrap">
                <a class="site-header__logo site-header__logo--top" href="index.html" aria-label="EONA-X">
                    <img src="logo/logo-transparent.svg" alt="" width="38" height="36" />
                </a>
                <nav class="site-header__pill" aria-label="Developer portal">
                    <a class="site-header__logo site-header__logo--pill" href="index.html" aria-label="EONA-X">
                        <img src="logo/logo-transparent.svg" alt="" width="28" height="27" />
                    </a>
                    { for props.links.iter().map(|link| {
                        let current: Option<AttrValue> = link.current.then(|| AttrValue::from("page"));
                        html! {
                            <a class="site-header__link" href={link.href.clone()} aria-current={current}>
                                { &link.label }
                            </a>
                        }
                    }) }
                    <div class="site-header__actions site-header__actions--pill">
                        <span class="site-header__lang">{ "EN" }</span>
                        if props.show_login {
                            <PillButton label="Log in" kind={PillButtonKind::GlassLight} disabled=true />
                        }
                    </div>
                </nav>
                <div class="site-header__actions site-header__actions--top">
                    <span class="site-header__lang">{ "EN" }</span>
                    if props.show_login {
                        <PillButton label="Log in" kind={PillButtonKind::GlassLight} disabled=true />
                    }
                </div>
            </div>
        </header>
    }
}
