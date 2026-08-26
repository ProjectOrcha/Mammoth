<!-- File browser — the root of the namespace. -->
<script lang="ts">
  import Browse from '$lib/components/Browse.svelte';
</script>

<header class="page">
  <h1>Files</h1>
  <p class="eyebrow">browse · block placement · read plan</p>
</header>

<Browse path="/" />

<style>
  .page {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 1.1rem;
  }
</style>
