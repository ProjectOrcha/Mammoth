<!-- One path: a directory listing, or a file with its block matrix, its read
     plan and its erasure-coding layout. -->
<script lang="ts">
  import { page } from '$app/state';
  import Browse from '$lib/components/Browse.svelte';

  const path = $derived('/' + (page.params.path ?? ''));
</script>

<header class="page">
  <h1>Files</h1>
  <p class="eyebrow">browse · block placement · read plan</p>
</header>

<Browse {path} />

<style>
  .page {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 1.1rem;
  }
</style>
