<!-- The frame around one generated call in Gallery.svelte: its code, and it live. -->
<script lang="ts">
  import type { Snippet } from 'svelte';

  let { name, source, code, children }: { name: string; source: string; code: string; children: Snippet } =
    $props();
  let dark = $state(false);
</script>

<section class="entry" id={name}>
  <h2>{name}</h2>
  <p class="src">{source}</p>
  <div class="pair">
    <pre><code>{code}</code></pre>
    <div class="live">
      <button type="button" class="bg-toggle" onclick={() => (dark = !dark)}>
        {dark ? 'Light background' : 'Dark background'}
      </button>
      <div class="stage eona-ui" class:dark>{@render children()}</div>
    </div>
  </div>
</section>
