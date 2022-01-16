const { parse } = require('@vue/component-compiler-utils')
const compiler = require('vue-template-compiler')

const source = `
<template>
    <h1>Hello world</h1>
</template>

<script>
export default {}
</script>

<style lang="stylus" scoped>
h1
    color red
</style>
`

const parsed = parse({
    source,
    filename: 'index.vue',
    compiler,
    sourceRoot: './',
    needMap: true,
})

console.log(JSON.stringify(parsed, null, 2))

out = {
    "template": {
        "type": "template",
        "content": "\n<h1>Hello world</h1>\n",
        "start": 11,
        "attrs": {},
        "end": 37
    },
    "script": {
        "type": "script",
        "content": "//\n//\n//\n//\n//\n\nmodule.exports = {}\n",
        "start": 58,
        "attrs": {},
        "end": 79,
        "map": {
            "version": 3,
            "sources": [
                "index.vue"
            ],
            "names": [],
            "mappings": ";;;;;;AAMA",
            "file": "index.vue",
            "sourceRoot": "./",
            "sourcesContent": [
                "\n<template>\n    <h1>Hello world</h1>\n</template>\n\n<script>\nmodule.exports = {}\n</script>\n\n<style lang=\"stylus\" scoped>\nh1\n    color red\n</style>\n"
            ]
        }
    },
    "styles": [
        {
            "type": "style",
            "content": "\n\n\n\n\n\n\n\n\n\nh1\n    color red\n",
            "start": 118,
            "attrs": {
                "lang": "stylus",
                "scoped": true
            },
            "lang": "stylus",
            "scoped": true,
            "end": 136,
            "map": {
                "version": 3,
                "sources": [
                    "index.vue"
                ],
                "names": [],
                "mappings": ";;;;;;;;;;AAUA;AACA",
                "file": "index.vue",
                "sourceRoot": "./",
                "sourcesContent": [
                    "\n<template>\n    <h1>Hello world</h1>\n</template>\n\n<script>\nmodule.exports = {}\n</script>\n\n<style lang=\"stylus\" scoped>\nh1\n    color red\n</style>\n"
                ]
            }
        }
    ],
    "customBlocks": [],
    "errors": []
}
