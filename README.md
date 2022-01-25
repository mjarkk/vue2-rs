# vue2-rs

A vue 2 template compiler written in rust

## Development

### Links

- [html spec](https://html.spec.whatwg.org/multipage/syntax.html)
- [vue file spec](https://vue-loader.vuejs.org/spec.html#intro)
- [vite writing a plugin](https://vitejs.dev/guide/api-plugin.html)

### Build WASM file and run the vite to test
```
wasm-pack build --target nodejs --dev && npm run dev
```

You can inspect what the vite is doing on: [localhost:3000/__inspect](http://localhost:3000/__inspect)
