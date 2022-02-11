use super::{template, utils::is_space, Parser, ParserError, QuoteKind, SourceLocation, TagType};

pub fn add_vm_references(
    p: &Parser,
    js: &SourceLocation,
    js_global_refs: &Vec<SourceLocation>,
) -> String {
    let mut resp = String::new();
    let mut js_global_refs_iter = js_global_refs.iter();

    let mut last = SourceLocation(js.0, js.0);
    let mut current = if let Some(location) = js_global_refs_iter.next() {
        location
    } else {
        return js.string(p);
    };

    loop {
        resp.push_str(&SourceLocation(last.1, current.0).string(p));

        let current_str = current.string(p);
        if current_str == "this" {
            resp.push_str("_vm");
        } else if p.local_variables.get(&current_str).is_some() {
            // is local variable, do not make modifications
            resp.push_str(&current_str);
        } else {
            resp.push_str("_vm.");
            resp.push_str(&current_str);
        }

        last = current.clone();
        if let Some(location) = js_global_refs_iter.next() {
            current = location;
        } else {
            break;
        }
    }

    resp.push_str(&SourceLocation(current.1, js.1).string(p));

    resp
}

// parses {{ foo + ' ' + bar }}
pub fn parse_template_var(p: &mut Parser) -> Result<Vec<SourceLocation>, ParserError> {
    let mut global_references: Option<Vec<SourceLocation>> = Some(Vec::with_capacity(1));

    parse_block_like(p, '}', &mut global_references)?;

    let c = p.must_read_one()?;
    if c != '}' {
        Err(ParserError::new(
            p,
            format!("expected '{}' but got '{}'", "}", c.to_string()),
        ))
    } else {
        Ok(global_references.unwrap())
    }
}

// parses v-bind:value="some_value"
pub fn parse_template_arg(
    p: &mut Parser,
    closure: char,
) -> Result<Vec<SourceLocation>, ParserError> {
    let mut global_references: Option<Vec<SourceLocation>> = Some(Vec::with_capacity(1));
    parse_block_like(p, closure, &mut global_references)?;
    Ok(global_references.unwrap())
}

pub fn compile_script_content(p: &mut Parser) -> Result<Option<SourceLocation>, ParserError> {
    let mut default_export_location: Option<SourceLocation> = None;
    'outer_loop: loop {
        match p.must_read_one()? {
            c if handle_common(p, c, &mut None)? => {}

            // check if this is the location of the "export default"
            'e' => {
                let default_export_start = p.current_char - 1;
                let mut export_remaining_chars = "xport".chars();
                while let Some(c) = export_remaining_chars.next() {
                    if p.must_read_one()? != c {
                        p.current_char -= 1;
                        continue 'outer_loop;
                    }
                }

                // There must be at least one space between "export" and "default"
                if !is_space(p.must_seek_one()?) {
                    continue;
                }

                // Read first character ('d') of "default"
                if p.must_read_one_skip_spacing()? != 'd' {
                    p.current_char -= 1;
                    continue;
                };

                let mut default_remaining_chars = "efault".chars();
                while let Some(c) = default_remaining_chars.next() {
                    if p.must_read_one()? != c {
                        p.current_char -= 1;
                        continue 'outer_loop;
                    }
                }

                if !is_space(p.must_seek_one()?) {
                    continue;
                }

                default_export_location =
                    Some(SourceLocation(default_export_start, p.current_char));
            }

            // Check if this is the script tag end </script>
            '<' => {
                match p.must_seek_one()? {
                    '/' | 'a'..='z' | 'A'..='Z' | '0'..='9' => {
                        match template::parse_tag(p, false) {
                            Err(e) => {
                                if e.is_eof() {
                                    return Err(e);
                                }
                                // Ignore if error is something else
                            }
                            Ok(tag) => {
                                // Check tag type, it needs to be </script>, not <script> nor <script />
                                if let TagType::Close = tag.type_ {
                                    // We expect this type
                                } else {
                                    return Err(ParserError::new(
                                        p,
                                        format!(
                                            "expected script closure but got {}",
                                            tag.type_.to_string()
                                        ),
                                    ));
                                }

                                // Tag needs to be a script tag
                                if !tag.name.eq(p, &mut "script".chars()) {
                                    return Err(ParserError::new(
                                        p,
                                        format!(
                                            "expected script closure but got {}",
                                            tag.name.string(p)
                                        ),
                                    ));
                                }

                                return Ok(default_export_location);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

pub fn parse_block_like(
    p: &mut Parser,
    closure: char,
    global_references: &mut Option<Vec<SourceLocation>>,
) -> Result<(), ParserError> {
    loop {
        match p.must_read_one()? {
            c if c == closure => return Ok(()),
            c if handle_common(p, c, global_references)? => {}
            c if c.is_ascii_lowercase() || c.is_ascii_uppercase() || c > '}' => {
                // Start of word, this might be a var or a static method
                parse_potential_var(p, global_references)?;
            }
            _ => {}
        }
    }
}

fn parse_potential_var(
    p: &mut Parser,
    global_references: &mut Option<Vec<SourceLocation>>,
) -> Result<(), ParserError> {
    let (mut c, name) = parse_name(p)?;

    // Note that "this" and "super" are removed from this list
    let name_matches_js_keyword = name.eq_some(
        p,
        false,
        vec![
            "abstract".chars(),
            "abstract".chars(),
            "arguments".chars(),
            "boolean".chars(),
            "break".chars(),
            "byte".chars(),
            "case".chars(),
            "catch".chars(),
            "char".chars(),
            "const".chars(),
            "continue".chars(),
            "debugger".chars(),
            "default".chars(),
            "delete".chars(),
            "do".chars(),
            "double".chars(),
            "else".chars(),
            "eval".chars(),
            "false".chars(),
            "final".chars(),
            "finally".chars(),
            "float".chars(),
            "for".chars(),
            "function".chars(),
            "goto".chars(),
            "if".chars(),
            "implements".chars(),
            "in".chars(),
            "instanceof".chars(),
            "int".chars(),
            "interface".chars(),
            "let".chars(),
            "long".chars(),
            "native".chars(),
            "new".chars(),
            "null".chars(),
            "package".chars(),
            "private".chars(),
            "protected".chars(),
            "public".chars(),
            "return".chars(),
            "short".chars(),
            "static".chars(),
            "switch".chars(),
            "synchronized".chars(),
            "throw".chars(),
            "throws".chars(),
            "transient".chars(),
            "true".chars(),
            "try".chars(),
            "typeof".chars(),
            "var".chars(),
            "void".chars(),
            "volatile".chars(),
            "while".chars(),
            "with".chars(),
            "yield".chars(),
            // ES5 keywords
            "class".chars(),
            "enum".chars(),
            "export".chars(),
            "extends".chars(),
            "import".chars(),
            // Extra
            "undefined".chars(),
        ],
    );

    if name_matches_js_keyword.is_some() {
        p.current_char -= 1;
        return Ok(());
    }

    if let Some(refs) = global_references {
        refs.push(name);
    }

    loop {
        match c {
            c if is_space(c) => {}
            '.' => {
                break;
            }
            '?' if p.must_seek_one()? == '.' => {
                p.current_char += 1;
                break;
            }
            '[' => {
                parse_block_like(p, ']', global_references)?;
            }
            ';' => {
                return Ok(());
            }
            _ => {
                p.current_char -= 1;
                return Ok(());
            }
        }
        c = p.must_read_one()?;
    }

    // This is a chain (a.b.c) or (a['b']['c']) or (a?.b?.c) or (a?.['b']?.['c'])
    loop {
        if c == '[' {
            // is a['b']['c'] or a?.['b']?.['c']
            parse_block_like(p, ']', global_references)?;
            c = p.must_read_one()?;
        } else {
            // is a.b.c or a?.b?.c
            let (next_c, _) = parse_name(p)?;
            c = next_c;
        }
        loop {
            match c {
                c if is_space(c) => {}
                '.' => {
                    break;
                }
                '?' if p.must_seek_one()? == '.' => {
                    p.current_char += 1;
                    break;
                }
                '[' => {
                    parse_block_like(p, ']', global_references)?;
                }
                ';' => {
                    return Ok(());
                }
                _ => {
                    p.current_char -= 1;
                    return Ok(());
                }
            }
            c = p.must_read_one()?;
        }
    }
}

pub fn parse_name(p: &mut Parser) -> Result<(char, SourceLocation), ParserError> {
    let start = p.current_char - 1;

    loop {
        match p.must_read_one()? {
            '_' => {}
            c if c.is_numeric() || c.is_ascii_lowercase() || c.is_ascii_uppercase() || c > '}' => {}
            c => {
                return Ok((c, SourceLocation(start, p.current_char - 1)));
            }
        }
    }
}

fn handle_common(
    p: &mut Parser,
    c: char,
    global_references: &mut Option<Vec<SourceLocation>>,
) -> Result<bool, ParserError> {
    match c {
        // Parse string
        '\'' => {
            p.parse_quotes(QuoteKind::JSSingle, global_references)?;
            Ok(true)
        }
        '"' => {
            p.parse_quotes(QuoteKind::JSDouble, global_references)?;
            Ok(true)
        }
        '`' => {
            p.parse_quotes(QuoteKind::JSBacktick, global_references)?;
            Ok(true)
        }
        // Parse comment
        '/' if parse_comment(p)? => Ok(true),
        // Parse block like
        '{' => {
            parse_block_like(p, '}', global_references)?;
            Ok(true)
        }
        '(' => {
            parse_block_like(p, ')', global_references)?;
            Ok(true)
        }
        '[' => {
            parse_block_like(p, ']', global_references)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn parse_comment(p: &mut Parser) -> Result<bool, ParserError> {
    match p.must_seek_one()? {
        '/' => {
            // this line is a comment
            p.current_char += 1;
            p.look_for(vec!['\n'])?;
            p.current_char -= 1;
            Ok(true)
        }
        '*' => {
            // look for end of comment
            p.current_char += 1;
            p.look_for(vec!['*', '/'])?;
            p.current_char -= 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub fn escape_quotes(input: &str, quote: char) -> String {
    let mut resp = String::new();
    for c in input.chars() {
        match c {
            c if c == quote => {
                resp.push('\\');
                resp.push(quote);
            }
            '\\' => {
                resp.push('\\');
                resp.push('\\');
            }
            c => resp.push(c),
        }
    }
    resp
}
