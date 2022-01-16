mod compiler;
mod utils;

use compiler::{error::ParserError, Parser};
// use utils::set_panic_hook;

use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn resolve_id(id: &str) {
    // utils::set_panic_hook();
    // log!("resolve_id {}", id);
}

#[wasm_bindgen]
pub fn load(id: &str) {
    // utils::set_panic_hook();
    // log!("load {}", id);
}

#[wasm_bindgen]
pub fn transform(code: &str, id: &str) -> Option<String> {
    // utils::set_panic_hook();

    let parsed_id = ParsedId::parse(id);
    if !parsed_id.is_vue {
        return None;
    }

    if parsed_id.is_main {
        log!("transforming {}", id);
        log!("code: {}", code);
        transform_main(code, id);
    } else {
        log!("TODO transform {}", id);
    }

    None
}

fn transform_main(code: &str, id: &str) -> Result<String, ParserError> {
    let compiled_source = compiler::Parser::parse(code);

    log!("{:#?}", compiled_source);

    Ok(String::new())
}

struct ParsedId {
    is_vue: bool,
    is_main: bool,
}

impl ParsedId {
    fn parse(id: &str) -> Self {
        if let Some((fist, _)) = id.split_once('?') {
            ParsedId {
                is_vue: fist.ends_with(".vue"),
                is_main: false,
            }
        } else {
            ParsedId {
                is_vue: id.ends_with(".vue"),
                is_main: true,
            }
        }
    }
}
