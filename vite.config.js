import { defineConfig } from 'vite'
import { createVuePlugin } from 'vite-plugin-vue2'
import { Plugin } from './pkg/vue2_rs.js'
import Inspect from 'vite-plugin-inspect'

const vuePlugin = createVuePlugin()

const newVue2Plugin = () => {
    const plugin = new Plugin()

    return {
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

        // Returns the file contents for a virtual ID
        load(id) {
            return plugin.load(id)
            // const resp = vuePlugin.load(id)
            // if (resp) console.log(id)
            // return resp
        },

        // transforms the code into the module
        async transform(code, id) {
            // const t1 = performance.now()
            const transformedCode = plugin.transform(code, id)
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
}

export default defineConfig({
    root: process.cwd() + '/preview',
    clearScreen: false,
    plugins: [
        Inspect(),
        vuePlugin,
        // newVue2Plugin(),
    ]
})

