mod compiler;
mod utils;

use compiler::template::to_js::template_to_js;
use compiler::utils::{write_str, write_str_escaped};
use compiler::{error::ParserError, Parser, SourceLocation};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct Plugin {
    styles_cache: HashMap<String, Vec<String>>,
}

#[wasm_bindgen]
impl Plugin {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            styles_cache: HashMap::new(),
        }
    }

    #[wasm_bindgen]
    pub fn resolve_id(&mut self, _id: &str) {
        utils::set_panic_hook();
        // log!("resolve_id {}", id);
    }

    #[wasm_bindgen]
    pub fn load(&mut self, id: &str) -> Option<String> {
        utils::set_panic_hook();

        match ParsedId::parse(id) {
            ParsedId::Other => None,
            ParsedId::Main => None,
            ParsedId::Style(component_id, index) => {
                if let Some(styles) = self.styles_cache.get(component_id) {
                    if let Some(style) = styles.get(index as usize) {
                        Some(style.clone())
                    } else {
                        Some(String::new())
                    }
                } else {
                    Some(String::new())
                }
            }
        }
    }

    #[wasm_bindgen]
    pub fn transform(&mut self, code: &str, id: &str) -> Option<String> {
        utils::set_panic_hook();

        match ParsedId::parse(id) {
            ParsedId::Other => None,
            ParsedId::Main => {
                // TODO remove unwrap
                let mut resp: Vec<char> = Vec::new();
                self.transform_main(code, id, &mut resp).unwrap();
                Some(resp.iter().collect())
            }
            ParsedId::Style(_, _) => {
                log!("TODO transform style {}", id);
                None
            }
        }
    }

    fn transform_main(
        &mut self,
        code: &str,
        id: &str,
        resp: &mut Vec<char>,
    ) -> Result<(), ParserError> {
        let parsed_code = Parser::new_and_parse(code)?;

        let script = parsed_code.script.as_ref();
        let template = parsed_code.template.as_ref();
        let styles = &parsed_code.styles;

        if styles.len() != 0 {
            let mut cache = Vec::with_capacity(styles.len());

            for (index, style) in styles.iter().enumerate() {
                if style.scoped {
                    cache.push(String::new());
                    todo!("scoped style");
                } else {
                    cache.push(style.content.string(&parsed_code));
                    let lang_extension = style.lang.as_ref().map(|v| v.as_str()).unwrap_or("css");

                    // Writes:
                    // id.vue?vue&type=style&index=0&lang.css
                    write_str("import '", resp);
                    write_str_escaped(id, '\'', '\\', resp);
                    write_str("?vue&type=style&index=", resp);
                    write_str(&index.to_string(), resp);
                    write_str("&lang.", resp);
                    write_str(&lang_extension, resp);
                    write_str("';\n", resp);
                }
            }

            self.styles_cache.insert(id.to_string(), cache);
        }

        if script.is_none() && template.is_none() {
            write_str("export default undefined;", resp);
            return Ok(());
        }

        if let Some(script) = script {
            if let Some(default_export_location) = script.default_export_location.as_ref() {
                SourceLocation(script.content.0, default_export_location.0)
                    .write_to_vec(&parsed_code, resp);

                write_str("\nconst __vue_2_file_default_export__ =", resp);

                SourceLocation(default_export_location.1, script.content.1)
                    .write_to_vec(&parsed_code, resp);
            } else {
                script.content.write_to_vec(&parsed_code, resp);
                write_str("\nconst __vue_2_file_default_export__ = {}", resp);
            }
        } else {
            write_str("\nconst __vue_2_file_default_export__ = {};", resp);
        }

        // Write the renderer to the result
        template_to_js(&parsed_code, resp);

        // Write the _scopeId to the result
        write_str("\n__vue_2_file_default_export__._scopeId = 'data-v-", resp);
        write_str(&simple_hash_crypto_unsafe(id), resp);
        resp.push('\'');

        write_str("\nexport default __vue_2_file_default_export__;", resp);

        Ok(())
    }
}

fn simple_hash_crypto_unsafe(input: &str) -> String {
    let mut hash: [u8; 8] = [0; 8];

    for (index, b) in input.as_bytes().iter().enumerate() {
        let index_in_hash = index % 8;
        hash[index_in_hash] ^= b;
    }

    let hex_chars = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];
    let mut resp = String::with_capacity(16);
    for b in hash {
        resp.push(hex_chars[(b >> 4) as usize]);
        resp.push(hex_chars[(b & 0x0F) as usize]);
    }
    resp
}

enum ParsedId<'a> {
    Other,               // Not a vue file
    Main,                // The global vue file
    Style(&'a str, u16), // A style from the vue file
}

impl<'a> ParsedId<'a> {
    fn parse(id: &'a str) -> Self {
        if let Some((first, args)) = id.split_once('?') {
            if !first.ends_with(".vue") {
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
                ImportType::Style => ParsedId::Style(first, index),
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
