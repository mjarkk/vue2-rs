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
  - [ ] Source maps
  - [ ] Pug support?
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
        - [x] `v-text`
        - [x] `v-html`
        - [ ] `v-once`
        - [x] `v-model`
        - [ ] `v-cloak`
        - [x] Custom (`v-custom-directive`)
    - [ ] `<template>` element
      - [ ] Args
        - [x] `v-if`
        - [x] `v-for`
        - [ ] `v-pre`
        - [ ] `v-slot`
          - [x] with no data arg
          - [ ] data arg
          - [ ] working in combination with v-(if, if-else, else)
        - [ ] `v-text`
        - [ ] `v-html`
    - [ ] [`<slot>` element](https://v2.vuejs.org/v2/guide/components-slots.html)
      - [x] Default slot
      - [x] Named slots
      - [x] Default slot content
      - [ ] Slot args using v-bind
        - [ ] `v-bind=".."`
        - [ ] `v-bind:value=".."`
        - [ ] `v-bind:foo=".." v-bind:bar=".." v-bind=".."`

- Script
  - [x] Basic start and end detection (`<script>..</script>`)
  - [x] Inject JS render function from template
  - [x] Support other script languages (typescript)
  - [ ] Source maps
  - [x] Inject styles
    - [x] Global
    - [x] Scoped
- Style
  - [x] Basic start and end detection (`<style>..</style>`)
  - [x] Style parsing
  - [ ] Source maps
  - [x] Global
  - [x] [Scoped](https://vue-loader.vuejs.org/guide/scoped-css.html#scoped-css)
    - [x] [Deep selectors](https://vue-loader.vuejs.org/guide/scoped-css.html#mixing-local-and-global-styles)
- Vite stuff
  - [x] Compiling of Vue components
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

### Setup

```sh
cargo install wasm-pack
```

### Build WASM file and run vite to test

```sh
wasm-pack build --target nodejs --dev && npm run dev
```

You can inspect what the vite is doing on:
[localhost:3000/\_\_inspect](http://localhost:3000/__inspect)
