import { defineConfig } from 'vite'
import { createVuePlugin } from 'vite-plugin-vue2'
import { resolve_id, load, transform } from './pkg/vue2_rs.js'
import Inspect from 'vite-plugin-inspect'

const vuePlugin = createVuePlugin()

const vuePluginProxy = {
    name: 'vite-plugin-vue2',

    // config(config) {
    //     return vuePlugin.config(config)
    // },

    // handleHotUpdate(ctx) {
    //     return vuePlugin.handleHotUpdate(ctx)
    // },

    // configResolved(config) {
    //     return vuePlugin.configResolved(config)
    // },

    // configureServer(server) {
    //     return vuePlugin.configureServer(server)
    // },

    // // Returns should return virtual ids
    // async resolveId(id) {
    //     // resolve_id(id)

    //     const resp = await vuePlugin.resolveId(id)
    //     // if (resp) console.log(id, '>', resp)
    //     return resp
    // },

    // // Returns the file contents for a virtual ID
    // load(id) {
    //     // load(id)
    //     const resp = vuePlugin.load(id)
    //     if (resp) console.log(id)
    //     return resp
    // },

    // transforms the code into the module
    async transform(code, id) {
        if (/\.vue$/.test(id)) {
            const code = `
            import {defineComponent} from "@vue/composition-api";

            const __vue_2_file_default_export__ = defineComponent({
                data: ()=>({
                    list: ["a", "b"],
                    count: 1,
                    inputValue: "",
                }),
            });

            __vue_2_file_default_export__.render = function(c) {
                const _vm = this;
                const _h = _vm.$createElement;
                const _c = _vm._self._c || _h;
                return _c('div', [
                    _c('h1', [
                        _vm._v("It wurks " + _vm._s(_vm.count) + "!")
                    ]),
                    _c('button', {on: {"click": $event=>{_vm.count++}}}, [
                        _vm._v("+")
                    ]),
                    _c('button', {on: {"click": $event=>{_vm.count--}}}, [
                        _vm._v("-")
                    ])
                ])
            }
            ;
            export default __vue_2_file_default_export__;
            `
            return { code, map: null }
        }

        // const t1 = performance.now()
        const transformedCode = transform(code, id)
        if (transformedCode) {
            return { code: transformedCode, map: null }
        }
        // const t2 = performance.now()
        // const resp = await vuePlugin.transform(code, id)
        // const t3 = performance.now()
        // // console.log(`${t2 - t1} - ${t3 - t2}`)
        // // if (resp) console.log(resp)
        // return resp
    },
}

export default defineConfig({
    root: process.cwd() + '/preview',
    clearScreen: false,
    plugins: [
        Inspect(),
        // vuePlugin,
        vuePluginProxy,
    ]
})

