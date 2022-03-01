<template>
  <div class="test-component">
    <div>
      <slot>
        <div>Default content</div>
      </slot>

      <slot name="test" />
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
