import { defineConfig } from 'vite'
import { createVuePlugin } from 'vite-plugin-vue2'
import { resolve_id, load, transform } from './pkg/vue2_rs.js'
import Inspect from 'vite-plugin-inspect'

const vuePlugin = createVuePlugin()

const vuePluginProxy = {
    name: 'vite-plugin-vue2',

    config(config) {
        return vuePlugin.config(config)
    },

    handleHotUpdate(ctx) {
        return vuePlugin.handleHotUpdate(ctx)
    },

    configResolved(config) {
        return vuePlugin.configResolved(config)
    },

    configureServer(server) {
        return vuePlugin.configureServer(server)
    },

    // Returns should return virtual ids
    async resolveId(id) {
        resolve_id(id)

        const resp = await vuePlugin.resolveId(id)
        // if (resp) {
        //     console.log(id, '>', resp)
        // }
        return resp
    },

    // Returns the file contents of id
    load(id) {
        load(id)
        return vuePlugin.load(id)
    },

    // transforms the code into the module
    async transform(code, id) {
        const t1 = performance.now()
        transform(code, id)
        const t2 = performance.now()
        const resp = await vuePlugin.transform(code, id)
        const t3 = performance.now()
        // console.log(`${t2 - t1} - ${t3 - t2}`)
        // if (resp) console.log(resp)
        return resp
    },
}

export default defineConfig({
    root: process.cwd() + '/preview',
    clearScreen: false,
    plugins: [
        Inspect(),
        vuePlugin,
        // vuePluginProxy,
    ]
})

