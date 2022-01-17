import Vue from 'vue'
import App from './component.vue'

const App2 = Vue.component('test', { render: c => c('span', 'test') })

console.log(App2)
console.log(App)

new Vue({
    render: h => h(App),
}).$mount('#app')
