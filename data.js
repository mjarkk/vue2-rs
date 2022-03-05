// v-bind:value="'data'"
var render = function () {
    var _vm = this;
    var _h = _vm.$createElement;
    var _c = _vm._self._c || _h;
    return _c("div", { staticClass: "test-component" }, [
        _c(
            "div",
            [
                _vm._t("default", function () {
                    return [_c("div", [_vm._v("Default content")])];
                }),
                _vm._t("test", null, null, { value: "data" }),
            ],
            2
        ),
    ]);
};

// v-bind="{value:'data'}"
var render = function () {
    var _vm = this;
    var _h = _vm.$createElement;
    var _c = _vm._self._c || _h;
    return _c("div", { staticClass: "test-component" }, [
        _c(
            "div",
            [
                _vm._t("default", function () {
                    return [_c("div", [_vm._v("Default content")])];
                }),
                _vm._t("test", null, null, { value: "data" }),
            ],
            2
        ),
    ]);
};

// v-bind="{test:'ok'}" v-bind:foo="'foo'" v-bind:bar="'bar'"
var render = function () {
    var _vm = this;
    var _h = _vm.$createElement;
    var _c = _vm._self._c || _h;
    return _c("div", { staticClass: "test-component" }, [
        _c(
            "div",
            [
                _vm._t("default", function () {
                    return [_c("div", [_vm._v("Default content")])];
                }),
                _vm._t("test", null, { bar: "bar", foo: "foo" }, { test: "ok" }),
            ],
            2
        ),
    ]);
};

