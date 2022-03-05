<template>
  <div class="test-component">
    <div>
      <slot>
        <div>Default content</div>
      </slot>

      <slot
        name="test"
        v-bind="{ test: 'ok' }"
        v-bind:bar="'bar'"
        v-bind:foo="'foo'"
      />
    </div>
  </div>
</template>

<script>
export default {};
</script>

<style scoped>
* {
  color: darkorchid;
}
</style>
