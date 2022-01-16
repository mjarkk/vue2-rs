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

    async resolveId(id) {
        resolve_id(id)
        return await vuePlugin.resolveId(id)
    },

    load(id) {
        load(id)
        return vuePlugin.load(id)
    },

    async transform(code, id, transformOptions) {
        transform(id)
        return await vuePlugin.transform(code, id, transformOptions)
    },
}

export default defineConfig({
    root: process.cwd() + '/preview',
    clearScreen: false,
    plugins: [
        Inspect(),
        vuePluginProxy,
    ]
})

