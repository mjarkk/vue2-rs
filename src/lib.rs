mod compiler;
mod utils;

use compiler::{error::ParserError, template::to_js::template_to_js, Parser, SourceLocation};
// use utils::set_panic_hook;

use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn resolve_id(_id: &str) {
    utils::set_panic_hook();
    // log!("resolve_id {}", id);
}

#[wasm_bindgen]
pub fn load(_id: &str) {
    utils::set_panic_hook();
    // log!("load {}", id);
}

#[wasm_bindgen]
pub fn transform(code: &str, id: &str) -> Option<String> {
    utils::set_panic_hook();

    let parsed_id = ParsedId::parse(id);
    if !parsed_id.is_vue {
        return None;
    }

    if parsed_id.is_main {
        let result = transform_main(code, id).unwrap();

        log!("result: {}", result);
        Some(result)
    } else {
        log!("TODO transform {}", id);
        None
    }
}

fn transform_main(code: &str, _id: &str) -> Result<String, ParserError> {
    let parsed_code = Parser::new_and_parse(code)?;

    let script = parsed_code.script.as_ref();
    let template = parsed_code.template.as_ref();
    if script.is_none() && template.is_none() {
        return Ok(String::from("export default undefined;"));
    }

    let mut resp: Vec<char> = if let Some(script) = script {
        if let Some(default_export_location) = script.default_export_location.as_ref() {
            let mut resp: Vec<char> = Vec::new();

            SourceLocation(script.content.0, default_export_location.0)
                .write_to_vec(&parsed_code, &mut resp);

            compiler::utils::write_str("\nconst __vue_2_file_default_export__ =", &mut resp);

            SourceLocation(default_export_location.1, script.content.1)
                .write_to_vec(&parsed_code, &mut resp);

            resp
        } else {
            let mut resp = script.content.chars_vec(&parsed_code);

            resp.append(
                &mut "\nconst __vue_2_file_default_export__ = {};"
                    .chars()
                    .collect::<Vec<char>>(),
            );

            resp
        }
    } else {
        "const __vue_2_file_default_export__ = {};"
            .chars()
            .collect()
    };

    template_to_js(&parsed_code, &mut resp);
    resp.append(
        &mut "\nexport default __vue_2_file_default_export__;"
            .chars()
            .collect::<Vec<char>>(),
    );

    Ok(resp.iter().collect())
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
