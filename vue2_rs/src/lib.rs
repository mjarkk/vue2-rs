mod compiler;
mod utils;

use utils::set_panic_hook;

use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn resolve_id(id: &str) {
    set_panic_hook();
    log!("resolve_id {}", id);
}

#[wasm_bindgen]
pub fn load(id: &str) {
    set_panic_hook();
    log!("load {}", id);
}

#[wasm_bindgen]
pub fn transform(id: &str) {
    set_panic_hook();
    log!("transform {}", id);
}
