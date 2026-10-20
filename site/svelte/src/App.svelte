<script lang="ts">
  import Gallery from './Gallery.svelte';
</script>

<header class="top">
  <h1>eona-ui-toolkit</h1>
  <nav>
    <a href="../">Yew gallery</a>
    <a href="../react/">React</a>
    <a href="./" aria-current="page">Svelte</a>
  </nav>
</header>
<p class="intro">
  Every component <code>@eona-x/ui-toolkit-svelte</code> exports, rendered by Svelte from the call
  the gallery prints, styled by <code>@eona-x/ui-toolkit-css</code>. Generated from the Yew source;
  see <a href="https://github.com/EONA-X/eona-ui-toolkit">the repository</a>.
</p>
<main class="entries">
  <Gallery />
</main>
