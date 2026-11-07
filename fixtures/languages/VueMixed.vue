<template>
 <ul>
  <li v-for="user in users" :key="user.name" v-if="user.active" @[event]="label(user)">{{ label(user) }}</li>
 </ul>
</template>
<script setup lang="ts">
interface User { name: string; active: boolean }
const users: User[] = []; const event = 'click';
function label(user: User) { return 'hello ' + user.name; }
</script>
<script>export default { name: 'Hello' }</script>
<style scoped lang="scss">ul { li { color: red; } }</style>
<style>.hello { color: blue; }</style>
