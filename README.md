# vue2-rs

A vue 2 template compiler written in rust and exposed as a plugin for
[Vite](https://vitejs.dev) and [Rollup.js](https://rollupjs.org/guide/en/)

# WORK IN PROGRESS

This is a project that is in progress

## TODO List

- Template
  - [x] Basic start and end detection (`<template>..</template>`)
  - [x] Parsing
    - [x] Tags
    - [x] Text
    - [x] Vars
  - [ ] Convert to JS render method
    - [x] Static text
    - [x] Var (`{{ foo+bar }}`)
    - [ ] Support HTML escape characters
    - [ ] Tag
      - [x] Name
      - [x] Children
      - [ ] Args
        - [x] normal (`<tag foo="bar" />`)
        - [x] `v-bind`
        - [x] `v-on`
        - [x] `v-if`, `v-else-if`, `v-else`
        - [x] `v-for`
        - [ ] `v-pre`
        - [ ] `v-slot`
        - [x] `v-text`
        - [x] `v-html`
        - [ ] `v-once`
        - [x] `v-model`
        - [ ] `v-cloak`
        - [x] Custom (`v-custom-directive`)
    - [ ] Support for `<template>` element
      - [ ] Args
        - [x] `v-if`
        - [x] `v-for`
        - [ ] `v-pre`
        - [ ] `v-slot`
        - [ ] `v-text`
        - [ ] `v-html`
    - [ ] Pug support?
- Script
  - [x] Basic start and end detection (`<script>..</script>`)
  - [x] Inject JS render function from template
  - [ ] Inject styles
- Style
  - [x] Basic start and end detection (`<style>..</style>`)
  - [ ] Style parsing
  - [x] Global
  - [ ] Scoped
- Vite stuff
  - [x] Transpiling of Vue components
  - [ ] Component error handling
  - [ ] Hot Module Reloading
- Other
  - [x] Html comments

## Development

### Links

- [html spec](https://html.spec.whatwg.org/multipage/syntax.html)
- [vue file spec](https://vue-loader.vuejs.org/spec.html#intro)
- [vite writing a plugin](https://vitejs.dev/guide/api-plugin.html)
- [wasm-bindgen docs](https://rustwasm.github.io/docs/wasm-bindgen/examples/web-audio.html)

### Build WASM file and run vite to test

```
wasm-pack build --target nodejs --dev && npm run dev
```

You can inspect what the vite is doing on:
[localhost:3000/\_\_inspect](http://localhost:3000/__inspect)
