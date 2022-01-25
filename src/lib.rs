mod compiler;
mod utils;

use compiler::{
    error::ParserError, template::convert_template_to_js_render_fn, Parser, SourceLocation,
};
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
        let result = transform_main(code, id).unwrap();

        log!("result: {}", result);
    } else {
        log!("TODO transform {}", id);
    }

    None
}

fn transform_main(code: &str, id: &str) -> Result<String, ParserError> {
    let parsed_code = Parser::new_and_parse(code)?;

    let resp: Vec<char> = match (parsed_code.template.as_ref(), parsed_code.script.as_ref()) {
        (_, Some(script)) => {
            if let Some(default_export_location) = &script.default_export_location {
                let mut resp = SourceLocation(script.content.0, default_export_location.0)
                    .chars_vec(&parsed_code);
                resp.append(
                    &mut "const __vue_2_file_default_export__ ="
                        .chars()
                        .collect::<Vec<char>>(),
                );
                resp.append(
                    &mut SourceLocation(default_export_location.1, script.content.1)
                        .chars_vec(&parsed_code),
                );
                if let Some(template) = parsed_code.template {
                    convert_template_to_js_render_fn(template, &mut resp);
                }
                resp.append(
                    &mut "\nexport default __vue_2_file_default_export__"
                        .chars()
                        .collect::<Vec<char>>(),
                );
                resp
            } else {
                // This vue file doesn't seem to have a deafult export, lets add it
                let mut resp = script.content.chars_vec(&parsed_code);
                resp.append(&mut "\nexport default undefined;".chars().collect::<Vec<char>>());
                resp
            }
        }
        _ => "export default undefined".chars().collect(),
    };

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
