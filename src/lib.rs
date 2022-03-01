mod compiler;
mod utils;

use compiler::template::to_js::template_to_js;
use compiler::utils::{write_str, write_str_escaped};
use compiler::{error::ParserError, style, Parser, SourceLocation, Style};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

struct ComponentCache {
    logic: Option<String>,
    styles: Vec<String>,
}

#[wasm_bindgen]
pub struct Plugin {
    components_cache: HashMap<String, ComponentCache>,
}

#[wasm_bindgen]
impl Plugin {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        utils::set_panic_hook();
        Self {
            components_cache: HashMap::new(),
        }
    }

    #[wasm_bindgen]
    pub fn resolve_id(&mut self, _id: &str) {
        // log!("resolve_id {}", id);
    }

    #[wasm_bindgen]
    pub fn load(&mut self, id: &str) -> Option<String> {
        match ParsedId::parse(id) {
            ParsedId::Other => None,
            ParsedId::Main => None,
            ParsedId::Style(style) => {
                if let Some(component) = self.components_cache.get(style.id) {
                    if let Some(style) = component.styles.get(style.index as usize) {
                        return Some(style.clone());
                    }
                }
                Some(String::new())
            }
            ParsedId::Logic(id) => {
                if let Some(component) = self.components_cache.get(id) {
                    if let Some(logic) = &component.logic {
                        return Some(logic.clone());
                    }
                }
                Some(String::new())
            }
        }
    }

    #[wasm_bindgen]
    pub fn transform(&mut self, code: &str, id: &str) -> Option<String> {
        match ParsedId::parse(id) {
            ParsedId::Other => None,
            ParsedId::Main => {
                let mut resp: Vec<char> = Vec::new();
                // TODO remove unwrap
                self.transform_main(code, id, &mut resp).unwrap();
                Some(resp.iter().collect())
            }
            ParsedId::Style(style_data) => {
                if style_data.scoped {
                    let mut parser = Parser::new(code);
                    // TODO remove unwrap
                    let injection_points =
                        style::parse_scoped_css(&mut parser, style::SelectorsEnd::EOF).unwrap();

                    Some(style::gen_scoped_css(
                        &mut parser,
                        SourceLocation(0, code.len()),
                        injection_points,
                        &simple_hash_crypto_unsafe(id),
                    ))
                } else {
                    None
                }
            }
            ParsedId::Logic(_) => None,
        }
    }

    fn transform_main(
        &mut self,
        code: &str,
        id: &str,
        resp: &mut Vec<char>,
    ) -> Result<(), ParserError> {
        let id_hash = &simple_hash_crypto_unsafe(id);
        let parsed_code = Parser::new_and_parse(code, id_hash)?;

        let script = parsed_code.script.as_ref();
        let template = parsed_code.template.as_ref();
        let styles = &parsed_code.styles;

        let mut cache_entry = ComponentCache {
            logic: None,
            styles: Vec::new(),
        };

        if styles.len() != 0 {
            for (index, style_kind) in styles.iter().enumerate() {
                // Writes:
                // id.vue?vue&type=style&index=0&lang.css
                // Or
                // id.vue?vue&type=style&index=0&scoped=true&lang.css
                write_str("import '", resp);
                write_str_escaped(id, '\'', '\\', resp);
                write_str("?vue&type=style&index=", resp);
                write_str(&index.to_string(), resp);

                match style_kind {
                    Style::Normal(style) => {
                        cache_entry.styles.push(style.content.string(&parsed_code));

                        if style.scoped {
                            write_str("&scoped=true", resp);
                        }

                        let lang_extension =
                            style.lang.as_ref().map(|v| v.as_str()).unwrap_or("css");
                        write_str("&lang.", resp);
                        write_str(&lang_extension, resp);
                    }
                    Style::DirectScopedCSS(style) => {
                        cache_entry.styles.push(style.clone());

                        write_str("&pre-scoped=true", resp);
                        write_str("&lang.css", resp);
                    }
                };

                write_str("';\n", resp);
            }
        }

        if script.is_none() && template.is_none() {
            write_str("export default undefined;", resp);
            return Ok(());
        }

        if let Some(script) = script {
            cache_entry.logic = Some(script.content.string(&parsed_code));

            // Writes:
            // id.vue?vue&type=logic&lang.js
            write_str("\nimport * as logic from '", resp);
            write_str_escaped(id, '\'', '\\', resp);
            let lang_extension = script.lang.as_ref().map(|v| v.as_str()).unwrap_or("js");
            write_str("?vue&type=logic&lang.", resp);
            write_str(lang_extension, resp);
            write_str("';\nconst c = logic.default || {};", resp);
        } else {
            write_str("\nconst c = {};", resp);
        }

        // Write the renderer to the result
        template_to_js(&parsed_code, resp);

        // Write the _scopeId to the result
        write_str("\nc._scopeId = 'data-v-", resp);
        write_str(id_hash, resp);
        resp.push('\'');

        // Write the filename to the component
        write_str("\nc.__file = '", resp);
        write_str_escaped(id, '\'', '\\', resp);
        resp.push('\'');

        write_str("\nexport default c;", resp);

        self.components_cache.insert(id.to_string(), cache_entry);

        Ok(())
    }
}

pub fn simple_hash_crypto_unsafe(input: &str) -> String {
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
    Other,                  // Not a vue file
    Main,                   // The global vue file
    Style(TargetStyle<'a>), // Targets a style within a vue file
    Logic(&'a str),         // Targets a script tag within a vue file
}

struct TargetStyle<'a> {
    id: &'a str,
    index: u16,
    scoped: bool,
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
            let mut scoped = false;

            for elem in args.split('&') {
                if let Some((key, value)) = elem.split_once('=') {
                    match key {
                        "type" => {
                            import_type = match value {
                                "style" => ImportType::Style,
                                "logic" => ImportType::Logic,
                                _ => ImportType::Main,
                            };
                        }
                        "index" => {
                            if let Ok(new_index) = value.parse::<u16>() {
                                index = new_index;
                            }
                        }
                        "scoped" => {
                            scoped = true;
                        }
                        _ => {}
                    }
                };
            }

            match import_type {
                ImportType::Main => ParsedId::Main,
                ImportType::Style => ParsedId::Style(TargetStyle {
                    id: first,
                    index,
                    scoped,
                }),
                ImportType::Logic => ParsedId::Logic(first),
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
    Logic,
}
