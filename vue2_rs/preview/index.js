import Vue from 'vue'
import App1 from './component.vue'

const App2 = Vue.component('test', { render: c => c('span', 'test') })
const App3 = Vue.component('test2', {
    data: () => ({
        count: 1,
    }),
    render: function () {
        var _vm = this;
        var _h = _vm.$createElement;
        var _c = _vm._self._c || _h;
        return _c('div', [
            _c('h1', [
                _vm._v("It wurks " + _vm._s(_vm.count) + " !")
            ]),
            _c('button', { on: { "click": $event => { _vm.count++ } } }, [_vm._v("+")]),
            _c('button', { on: { "click": $event => { _vm.count-- } } }, [_vm._v("-")]),
        ])
    }
})

console.log(App2)
console.log(App1)

new Vue({
    render: h => h(App1),
}).$mount('#app')
