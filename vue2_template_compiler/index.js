const { parse } = require('@vue/component-compiler-utils')
const compiler = require('vue-template-compiler')

const source = `
<template>
    <h1>Hello world</h1>
</template>

<script>
module.exports = {}
</script>

<style lang="stylus" scoped>
h1
    color red
</style>
`

console.log(parse({
    source,
    filename: 'index.vue',
    compiler,
    sourceRoot: './',
    needMap: true,
}))
