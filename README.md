# vue2-rs

A vue 2 template compiler written in rust and exposed as a plugin for [Vite](https://vitejs.dev) and [Rollup.js](https://rollupjs.org/guide/en/)

# WORK IN PROGRESS

This is a project that is in progress

## Development

### Links

- [html spec](https://html.spec.whatwg.org/multipage/syntax.html)
- [vue file spec](https://vue-loader.vuejs.org/spec.html#intro)
- [vite writing a plugin](https://vitejs.dev/guide/api-plugin.html)

### Build WASM file and run vite to test
```
wasm-pack build --target nodejs --dev && npm run dev
```

You can inspect what the vite is doing on: [localhost:3000/__inspect](http://localhost:3000/__inspect)
