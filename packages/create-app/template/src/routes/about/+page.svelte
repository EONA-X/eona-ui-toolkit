<script>
  import { Callout, HeroFilter, PillButton } from '@eona-x/ui-toolkit-svelte';
  import { base } from '$app/paths';
</script>

<svelte:head>
  <title>About — {{name}}</title>
</svelte:head>

<HeroFilter title="About" description="How this site is put together." withBackdrop>
  <PillButton label="Back home" kind="glass-light" href="{base}/" />
</HeroFilter>

<section class="block">
  <div class="wrap">
    <p>
      Pages live in <code>src/routes</code>; each folder is a URL. The components come from
      <code>@eona-x/ui-toolkit-svelte</code> and the styles from <code>@eona-x/ui-toolkit-css</code> —
      both generated from the EONA-X toolkit's Yew source, so they match the other stacks exactly.
    </p>
    <p>
      Browse every component: <a href="https://eona-x.github.io/eona-ui-toolkit/svelte/">the Svelte components page</a>.
    </p>
  </div>
</section>
