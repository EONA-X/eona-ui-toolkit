<script lang="ts">
  import { HeroFilter, PillButton, SimpleFooter, SiteHeader } from '@eona-x/ui-toolkit-svelte';

  import nav from '../../nav.json';
  import Gallery from './Gallery.svelte';

  // This page is /svelte/: one level below the site root.
  const links = nav.links.map((l) => ({ href: `../${l.path}`, label: l.label, current: l.stack === 'svelte' }));
</script>

<div class="eona-ui">
  <SiteHeader {links} />
  <HeroFilter
    title="Components"
    description="Every component @eona-x/ui-toolkit-svelte exports, rendered live by Svelte from the call the gallery prints, styled by @eona-x/ui-toolkit-css."
    withBackdrop
  >
    <PillButton label="Getting started" kind="glass-light" href="getting-started/" />
    <PillButton label="Components" href="./" />
  </HeroFilter>
  <main class="entries">
    <Gallery />
  </main>
  <SimpleFooter
    text="eona-ui-toolkit — one design system, generated for Yew, React and Svelte"
    link={{ href: nav.repo, label: 'Source on GitHub' }}
  />
</div>
