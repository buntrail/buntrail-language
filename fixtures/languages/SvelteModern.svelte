<script module>export const name = 'hello';</script>
<script lang="ts">
let count = $state(0); let doubled = $derived(count * 2); let { users } = $props();
function bump() { const literal = `brace } ${'{'}`; count++; }
</script>
{#snippet row(user)}
 <b>{user.name}</b>
{/snippet}
{#each users as user (user.id)}
 {@render row(user)}
{/each}
{#if count}
 <p>{doubled}</p>
{/if}
{#await Promise.resolve('hello')}
 pending
{:then value}
 {value}
{:catch error}
 {error}
{/await}
<style>b { color: red; }</style>
