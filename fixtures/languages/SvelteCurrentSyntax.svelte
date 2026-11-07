<script lang="ts" generics="T extends { name: string }">
  let { items }: { items: T[] } = $props();
  function tooltip(node: HTMLElement) { node.title = 'hello'; }
</script>
<svelte:boundary>
  <div {@attach tooltip}>hello</div>
  {#snippet failed(error, reset)}<button onclick={reset}>Try again</button>{/snippet}
</svelte:boundary>
