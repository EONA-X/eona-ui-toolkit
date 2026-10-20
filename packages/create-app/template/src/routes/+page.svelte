<script>
  import { HeroFilter, MetricTile, Pill, PillButton, SectionHead } from '@eona-x/ui-toolkit-svelte';
  import { base } from '$app/paths';
</script>

<svelte:head>
  <title>{{name}}</title>
</svelte:head>

<HeroFilter
  title="{{name}}"
  description="A static site on the EONA-X design system: SvelteKit renders every page to HTML at build time."
  withBackdrop
>
  <PillButton label="About this site" href="{base}/about" />
</HeroFilter>

<section class="block">
  <div class="wrap">
    <SectionHead number="01" kicker="at a glance" title="Your first components" />
    <div class="tiles">
      <MetricTile label="Datasets" value="12" help="Unchanged since the last run" />
      <MetricTile label="Participants" value="38" />
      <MetricTile label="Records analysed" value="128 430" />
    </div>
    <p class="pills">
      <Pill tone="ok">Mobility</Pill>
      <Pill tone="ok">Tourism</Pill>
    </p>
  </div>
</section>

<style>
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 18px; }
  .pills { display: flex; gap: 8px; margin-top: 24px; }
</style>
