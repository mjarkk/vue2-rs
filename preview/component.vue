<template>
  <div>
    <h1>It wurks {{ count }}!</h1>
    <div v-if="count">If {{ count }}!</div>
    <div v-else>Else {{ count }}</div>
    <div v-if="!count">Another else</div>
    <button v-on:click="count++">+</button>
    <button @click="count--">-</button>
  </div>
</template>

<script>
import { defineComponent } from "@vue/composition-api";

export default defineComponent({
  data: () => ({
    count: 1,
  }),
});
</script>
