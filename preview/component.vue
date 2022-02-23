<template>
  <div>
    <!-- <template />
    <template><template /></template>
    <template>
      <h1>It wurks {{ count }}!</h1>
    </template>
    <template v-if="count" />
    <template v-for="(key, index) in list">{{ key }} #{{ index }}</template> -->

    <div v-if="count">
      If <span>{{ count }}</span> !
    </div>
    <div v-else>Else {{ count }}</div>
    <div v-if="!count">Another else</div>
    <div>
      <button v-on:click="count++">+</button>
      <button @click="count--">-</button>
    </div>
    <!-- <div v-text="'abc'" />
    <div v-html="'abc'" />
    <div v-show="false"><div /></div>
    <div foo="bar">
      <span v-for="(key, index) in list" :key="key">
        {{ key }} #{{ index }}
      </span>
    </div>
    <input v-model="inputValue" />
    {{ inputValue }} -->
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

<style>
.foo {
  color: red;
}
.bar {
  color: blue;
}
* {
  font-family: sans-serif;
  font-size: 22px;
}
</style>
