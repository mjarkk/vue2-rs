<template>
  <div>
    <h1>It wurks {{ count }}!</h1>
    <!-- <div v-if="count">If {{ count }}!</div>
    <div v-else>Else {{ count }}</div>
    <div v-if="!count">Another else</div>
    <div v-text="'abc'" />
    <div v-html="'abc'" />
    <div v-show="false"><div /></div>
    <div foo="bar">
      <span v-for="(key, index) in list" :key="key">
        {{ key }} #{{ index }}</span
      >
    </div>
    <input v-model="inputValue" /> -->
    <button v-on:click="count++">+</button>
    <button @click="count--">-</button>
  </div>
</template>

<script>
import { defineComponent } from "@vue/composition-api";

export default defineComponent({
  data: () => ({
    list: ["a", "b"],
    count: 1,
    inputValue: "",
  }),
});
</script>
