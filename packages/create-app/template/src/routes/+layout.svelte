<script>
  // The design system: tokens + components, scoped under .eona-ui, and the
  // EONA-X page shell (fixed header offset, page typography). Drop page.css
  // if these pages are embedded in a layout of your own.
  import '@eona-x/ui-toolkit-css';
  import '@eona-x/ui-toolkit-css/page.css';

  import { SimpleFooter, SiteHeader } from '@eona-x/ui-toolkit-svelte';
  import { base } from '$app/paths';
  import { page } from '$app/state';

  let { children } = $props();

  const routes = [
    { path: '/', label: 'Home' },
    { path: '/about', label: 'About' },
  ];
  const links = $derived(
    routes.map((r) => ({ href: `${base}${r.path}`, label: r.label, current: page.url.pathname === `${base}${r.path}` })),
  );
</script>

<div class="eona-ui">
  <SiteHeader {links} />
  {@render children()}
  <SimpleFooter text="{{name}} — built with the EONA-X design system" link={{ href: 'https://eona-x.eu/', label: 'eona-x.eu' }} />
</div>
