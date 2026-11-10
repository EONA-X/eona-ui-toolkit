<script lang="ts">
  import { HeroFilter, PillButton, SimpleFooter } from '@eona-x/ui-toolkit-svelte';

  import Gallery from './Gallery.svelte';

  // The menu is not part of this app: pages.yml renders site/menu.ttl into index.html.
  const repo = 'https://github.com/EONA-X/eona-ui-toolkit';
</script>

<div class="eona-ui">
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
    link={{ href: repo, label: 'Source on GitHub' }}
  />
</div>
