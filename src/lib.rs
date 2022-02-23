#[macro_use]
extern crate lazy_static;

mod compiler;
mod utils;

use compiler::template::to_js::template_to_js;
use compiler::utils::{write_str, write_str_escaped};
use compiler::{error::ParserError, Parser, SourceLocation};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

lazy_static! {
    static ref STYLES_CACHE: Rc<RefCell<HashMap<String, Vec<String>>>> =
        Rc::new(RefCell::new(HashMap::with_capacity(16)));
}

#[wasm_bindgen]
pub fn resolve_id(_id: &str) {
    utils::set_panic_hook();
    // log!("resolve_id {}", id);
}

#[wasm_bindgen]
pub fn load(id: &str) -> Option<String> {
    utils::set_panic_hook();

    match ParsedId::parse(id) {
        ParsedId::Other => None,
        ParsedId::Main => None,
        ParsedId::Style(index) => {
            log!("LOAD: IDX: {}, ID: {}", index, id);
            None
        }
    }
}

#[wasm_bindgen]
pub fn transform(code: &str, id: &str) -> Option<String> {
    utils::set_panic_hook();

    match ParsedId::parse(id) {
        ParsedId::Other => None,
        ParsedId::Main => {
            // TODO remove unwrap
            Some(transform_main(code, id).unwrap())
        }
        ParsedId::Style(_) => {
            log!("TODO transform style {}", id);
            None
        }
    }
}

fn transform_main(code: &str, id: &str) -> Result<String, ParserError> {
    let parsed_code = Parser::new_and_parse(code)?;

    let script = parsed_code.script.as_ref();
    let template = parsed_code.template.as_ref();
    let styles = &parsed_code.styles;

    let mut resp: Vec<char> = Vec::new();
    if styles.len() != 0 {
        let styles_cache = STYLES_CACHE.borrow_mut();
        let (cache, is_new_entry) = if let Some(arr) = styles_cache.get_mut(id) {
            (arr, true)
        } else {
            (&mut Vec::with_capacity(styles.len()), false)
        };

        for (index, style) in styles.iter().enumerate() {
            if style.scoped {
                todo!("scoped style");
            } else {
                style.content.string(&parsed_code);
                let lang_extension = style.lang.as_ref().map(|v| v.as_str()).unwrap_or("css");

                // Writes:
                // id.vue?vue&type=style&index=0&lang.css
                write_str("import '", &mut resp);
                write_str_escaped(id, '\'', '\\', &mut resp);
                write_str("?vue&type=style&index=", &mut resp);
                write_str(&index.to_string(), &mut resp);
                write_str("&lang.", &mut resp);
                write_str(&lang_extension, &mut resp);
                write_str("';\n", &mut resp);
            }
        }

        if is_new_entry {
            styles_cache.insert(id.to_string(), *cache);
        }
    }

    if script.is_none() && template.is_none() {
        write_str("export default undefined;", &mut resp);
        return Ok(resp.iter().collect());
    }

    if let Some(script) = script {
        if let Some(default_export_location) = script.default_export_location.as_ref() {
            SourceLocation(script.content.0, default_export_location.0)
                .write_to_vec(&parsed_code, &mut resp);

            write_str("\nconst __vue_2_file_default_export__ =", &mut resp);

            SourceLocation(default_export_location.1, script.content.1)
                .write_to_vec(&parsed_code, &mut resp);
        } else {
            script.content.write_to_vec(&parsed_code, &mut resp);
            write_str("\nconst __vue_2_file_default_export__ = {}", &mut resp);
        }
    } else {
        write_str("\nconst __vue_2_file_default_export__ = {};", &mut resp);
    }

    template_to_js(&parsed_code, &mut resp);
    resp.append(
        &mut "\nexport default __vue_2_file_default_export__;"
            .chars()
            .collect::<Vec<char>>(),
    );

    Ok(resp.iter().collect())
}

enum ParsedId {
    Other,      // Not a vue file
    Main,       // The global vue file
    Style(u16), // A style from the vue file
}

impl ParsedId {
    fn parse(id: &str) -> Self {
        if let Some((fist, args)) = id.split_once('?') {
            if !fist.ends_with(".vue") {
                return Self::Other;
            }

            // parse arg, Example:
            // vue&type=style&index=0&lang.css
            let mut import_type = ImportType::Main;
            let mut index = 0u16;

            for elem in args.split('&') {
                if let Some((key, value)) = elem.split_once('=') {
                    match key {
                        "type" => {
                            import_type = match value {
                                "style" => ImportType::Style,
                                _ => ImportType::Main,
                            };
                        }
                        "index" => {
                            if let Ok(new_index) = value.parse::<u16>() {
                                index = new_index;
                            }
                        }
                        _ => {}
                    }
                };
            }

            match import_type {
                ImportType::Main => ParsedId::Main,
                ImportType::Style => ParsedId::Style(index),
            }
        } else if id.ends_with(".vue") {
            Self::Main
        } else {
            Self::Other
        }
    }
}

enum ImportType {
    Main,
    Style,
}
