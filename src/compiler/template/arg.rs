use super::super::utils::is_space;
use super::super::{js, Parser, ParserError, QuoteKind, SourceLocation};
use super::VueTagArgs;

fn new_try_parse(
    p: &mut Parser,
    mut c: char,
    result: &mut VueTagArgs,
) -> Result<Option<char>, ParserError> {
    if !is_start_of_arg(c) {
        return Ok(None);
    }

    let (name_result, next_c) = parse_arg_name(p, c)?;
    c = next_c;

    let (expect_value, target_allowed, modifier_allowed, arg_kind) = match name_result.name.as_str()
    {
        "v-if" => (ExpectValue::Yes, false, false, VueArgKind::If),
        "v-pre" => (ExpectValue::Yes, false, false, VueArgKind::Pre),
        "v-else" => (ExpectValue::No, false, false, VueArgKind::Else),
        "v-slot" => (ExpectValue::Yes, true, false, VueArgKind::Slot),
        "v-text" => (ExpectValue::Yes, false, false, VueArgKind::Text),
        "v-html" => (ExpectValue::Yes, false, false, VueArgKind::Html),
        "v-once" => (ExpectValue::No, false, false, VueArgKind::Once),
        "v-model" => (ExpectValue::Yes, true, true, VueArgKind::Model),
        "v-cloak" => (ExpectValue::Yes, false, false, VueArgKind::Cloak),
        "v-else-if" => (ExpectValue::Yes, false, false, VueArgKind::ElseIf),
        "v-for" => (ExpectValue::Yes, false, false, VueArgKind::ElseIf),
        "v-bind" => (ExpectValue::Yes, true, true, VueArgKind::Bind),
        "v-on" => (ExpectValue::Yes, true, true, VueArgKind::On),
        name if name.starts_with("v-") => (
            ExpectValue::Yes,
            true,
            true,
            VueArgKind::CustomDirective(name.to_string()),
        ),
        _ => (ExpectValue::Both, false, false, VueArgKind::Default),
    };

    if !target_allowed && name_result.target.is_some() {
        return Err(ParserError::new(
            "try_parse_arg",
            format!(
                "target set on argument {} but is not allowed",
                name_result.name
            ),
        ));
    }

    if !modifier_allowed && name_result.modifiers.is_some() {
        return Err(ParserError::new(
            "try_parse_arg",
            format!(
                "modifier set on argument {} but is not allowed",
                name_result.name
            ),
        ));
    }

    match expect_value {
        ExpectValue::Yes if !name_result.parse_value_next => {
            return Err(ParserError::new(
                "try_parse_arg",
                format!(
                    "expected an argument value for {} but got none",
                    name_result.name
                ),
            ))
        }
        ExpectValue::No if name_result.parse_value_next => {
            return Err(ParserError::new(
                "try_parse_arg",
                format!(
                    "expected NO argument value for {} but got one",
                    name_result.name
                ),
            ))
        }
        _ => {}
    };

    match arg_kind {
        VueArgKind::Default => {}
        VueArgKind::Bind => {}
        VueArgKind::On => {}
        VueArgKind::Text => {}
        VueArgKind::Html => {}
        VueArgKind::If => {}
        VueArgKind::Else => {}
        VueArgKind::ElseIf => {}
        VueArgKind::For => {}
        VueArgKind::Model => {}
        VueArgKind::Slot => {}
        VueArgKind::Pre => {}
        VueArgKind::Cloak => {}
        VueArgKind::Once => {}
        VueArgKind::CustomDirective(_) => {}
    }

    Ok(Some(c))
}

fn is_start_of_arg(c: char) -> bool {
    match c {
        '@' | ':' | 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => true,
        _ => false,
    }
}

enum ExpectValue {
    Yes,
    No,
    Both,
}

struct ParseArgNameResult {
    parse_value_next: bool,
    // name details
    name: String,                   // `v-bind` of `v-bind:some_value.trim`
    target: Option<String>,         // `some_value` of `v-bind:some_value.trim`
    modifiers: Option<Vec<String>>, // `trim` of `v-bind:some_value.trim`
}

fn parse_arg_name(p: &mut Parser, mut c: char) -> Result<(ParseArgNameResult, char), ParserError> {
    let invalid_character_err = |c: char| {
        Err(ParserError::new(
            "try_parse_arg",
            format!("invalid argument character '{}'", c.to_string()),
        ))
    };

    let mut name = String::new();

    let mut parse_target_next = false;
    let mut parse_modifier_next = false;
    let mut parse_value_next = false;

    match c {
        '@' => {
            name = String::from("v-on");
            parse_target_next = true;
        }
        ':' => {
            name = String::from("v-bind");
            parse_target_next = true;
        }
        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {
            name.push(c);
            loop {
                c = p.must_read_one()?;
                match c {
                    'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => name.push(c),
                    ':' => {
                        parse_target_next = true;
                        break;
                    }
                    '.' => {
                        parse_modifier_next = true;
                        break;
                    }
                    '=' => {
                        parse_value_next = true;
                        break;
                    }
                    c => return invalid_character_err(c),
                }
            }
        }
        c => return invalid_character_err(c),
    };

    let target: Option<String> = if parse_target_next {
        let mut target = String::new();
        loop {
            c = p.must_read_one()?;
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => target.push(c),
                '.' => {
                    parse_modifier_next = true;
                    break;
                }
                '=' => {
                    parse_value_next = true;
                    break;
                }
                c => return invalid_character_err(c),
            }
        }
        Some(target)
    } else {
        None
    };

    let modifiers: Option<Vec<String>> = if parse_modifier_next {
        let mut modifiers: Vec<String> = Vec::new();
        'outer: loop {
            let mut modifier = String::new();
            loop {
                c = p.must_read_one()?;
                match c {
                    'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => modifier.push(c),
                    '.' => {
                        break;
                    }
                    '=' => {
                        parse_value_next = true;
                        break 'outer;
                    }
                    c => return invalid_character_err(c),
                }
            }
            modifiers.push(modifier);
        }
        Some(modifiers)
    } else {
        None
    };

    Ok((
        ParseArgNameResult {
            parse_value_next: parse_value_next,
            name,
            target,
            modifiers,
        },
        c,
    ))
}

// try_parse parses a key="value" , :key="value" , v-bind:key="value" , v-on:key="value" and @key="value"
// It returns Ok(None) if first_char is not a char expected as first character of a argument
pub fn try_parse(
    p: &mut Parser,
    mut c: char,
    result_args: &mut VueTagArgs,
    v_else_allowed: bool,
    is_custom_component: bool,
) -> Result<Option<char>, ParserError> {
    let mut is_v_on_shortcut = false;
    let mut is_v_bind_shortcut = false;

    let mut key_location = SourceLocation(p.current_char - 1, 0);

    match c {
        '@' => {
            is_v_on_shortcut = true;
            key_location.0 += 1;
        }
        ':' => {
            is_v_bind_shortcut = true;
            key_location.0 += 1;
        }
        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {}
        _ => return Ok(None),
    };

    let mut has_value = false;
    loop {
        c = p.must_read_one()?;
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | ':' => {}
            '=' => {
                let c = p.must_seek_one()?;
                has_value = !is_space(c) && c != '/' && c != '>';
                break;
            }
            c if is_space(c) || c == '/' || c == '>' => {
                break;
            }
            c => {
                return Err(ParserError::new(
                    "try_parse_arg",
                    format!("unexpected argument character '{}'", c.to_string()),
                ))
            }
        }
    }
    key_location.1 = p.current_char - 1;
    let is_vue_dash_arg = key_location.starts_with(p, "v-".chars());
    let is_vue_arg = is_vue_dash_arg || is_v_on_shortcut || is_v_bind_shortcut;
    let mut key = key_location.string(p);

    if is_vue_arg {
        // parse vue specific tag
        if key == "v-for" {
            // Parse the value of the v-for tag
            // V-for has a special value we cannot parse like the others
            if !has_value {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "expected an argument value for \"v-for\"",
                ));
            }

            let result = parse_v_for_value(p)?;

            let mut local_variables_list: Vec<String> = vec![result.value.clone()];
            if let Some(key) = result.key.as_ref() {
                local_variables_list.push(key.clone());
                if let Some(index) = result.index.as_ref() {
                    local_variables_list.push(index.clone());
                }
            }
            result_args.new_local_variables = Some(local_variables_list);
            result_args.set_modifier(VueTagModifier::For(result))?;

            c = p.must_read_one()?;
            return Ok(Some(c));
        }

        let value: String = if has_value {
            let closure = p.must_read_one()?;
            match closure {
                '"' | '\'' => {} // Ok
                c => {
                    return Err(ParserError::new(
                        "try_parse_arg",
                        format!(
                            "expected opening of argument value ('\"' or \"'\") but got '{}'",
                            c.to_string()
                        ),
                    ))
                }
            }
            let start = p.current_char;
            let replacements = js::parse_template_arg(p, closure)?;
            let sl = SourceLocation(start, p.current_char - 1);
            c = p.must_read_one()?;

            js::add_vm_references(p, &sl, &replacements)
        } else {
            String::from("undefined")
        };

        if is_v_on_shortcut {
            if is_vue_dash_arg {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "cannot use @v-.. as arg name",
                ));
            }
            if !has_value {
                return Err(ParserError::new(
                    "try_parse_arg",
                    format!("expected an argument value for \"@{}\"", key),
                ));
            }

            result_args.add(VueArgKind::On, key, value, is_custom_component)?;
            return Ok(Some(c));
        }

        if is_v_bind_shortcut {
            if is_vue_dash_arg {
                return Err(ParserError::new(
                    "try_parse_arg",
                    "cannot use :v-.. as arg name",
                ));
            }
            if !has_value {
                return Err(ParserError::new(
                    "try_parse_arg",
                    format!("expected an argument value for \":{}\"", key),
                ));
            }

            result_args.add(VueArgKind::Bind, key, value, is_custom_component)?;
            return Ok(Some(c));
        }

        // remove the v- from the argument
        key.replace_range(..2, "");

        let (expects_argument, arg_kind) = match key.as_str() {
            "if" => (true, VueArgKind::If),
            "pre" => (true, VueArgKind::Pre),
            "else" => (false, VueArgKind::Else),
            "slot" => (true, VueArgKind::Slot),
            key if key.starts_with("slot") => (true, VueArgKind::Slot),
            "text" => (true, VueArgKind::Text),
            "html" => (true, VueArgKind::Html),
            "once" => (false, VueArgKind::Once),
            "model" => (true, VueArgKind::Model),
            key if key.starts_with("model") => (true, VueArgKind::Model),
            "cloak" => (true, VueArgKind::Cloak),
            "else-if" => (true, VueArgKind::ElseIf),
            "bind" => (true, VueArgKind::Bind),
            key if key.starts_with("bind:") => (true, VueArgKind::Bind),
            "on" => (true, VueArgKind::On),
            key if key.starts_with("on") => (true, VueArgKind::On),
            _ => (true, VueArgKind::CustomDirective(key.clone())),
        };

        if has_value != expects_argument {
            return Err(ParserError::new(
                "try_parse_arg",
                if expects_argument {
                    format!("expected an argument value for \"v-{}\"", key)
                } else {
                    format!("expected no argument value for \"v-{}\"", key)
                },
            ));
        }

        key = match key.split_once(':') {
            Some((_, after)) => after.to_string(),
            None => String::new(),
        };

        if !v_else_allowed {
            match arg_kind {
                VueArgKind::ElseIf => {
                    return Err(ParserError::new(
                        "try_parse_arg",
                        "cannot use v-else-if here",
                    ));
                }
                VueArgKind::Else => {
                    return Err(ParserError::new("try_parse_arg", "cannot use v-else here"));
                }
                _ => {}
            }
        }

        result_args.add(arg_kind, key, value, is_custom_component)?;
        Ok(Some(c))
    } else {
        let value_as_js: String = if has_value {
            // Parse a static argument
            let value_location = match p.must_read_one()? {
                '"' => {
                    let start = p.current_char;
                    p.parse_quotes(QuoteKind::HTMLDouble, &mut None)?;
                    let sl = SourceLocation(start, p.current_char - 1);
                    c = p.must_read_one()?;
                    sl
                }
                '\'' => {
                    let start = p.current_char;
                    p.parse_quotes(QuoteKind::HTMLSingle, &mut None)?;
                    let sl = SourceLocation(start, p.current_char - 1);
                    c = p.must_read_one()?;
                    sl
                }
                _ => {
                    let start = p.current_char - 1;
                    loop {
                        c = p.must_read_one()?;
                        match c {
                            '>' | '/' => {
                                break;
                            }
                            c if is_space(c) => {
                                break;
                            }
                            _ => {}
                        }
                    }
                    SourceLocation(start, p.current_char - 1)
                }
            };

            let mut s = String::new();
            s.push('"');
            for c in value_location.chars(p) {
                match c {
                    '\\' => {
                        s.push('\\');
                        s.push('\\');
                    }
                    '"' => {
                        s.push('\\');
                        s.push('"');
                    }
                    c => s.push(*c),
                }
            }
            s.push('"');
            s
        } else {
            String::from("true")
        };

        result_args.add(
            VueArgKind::Default,
            key_location.string(p),
            value_as_js,
            is_custom_component,
        )?;
        Ok(Some(c))
    }
}

#[derive(Debug, Clone)]
pub struct ParsedVFor {
    pub value: String,
    pub key: Option<String>,
    pub index: Option<String>,
    pub list: String,
}

fn parse_v_for_value(p: &mut Parser) -> Result<ParsedVFor, ParserError> {
    let closure = p.must_read_one()?;
    match closure {
        '"' | '\'' => {} // Ok
        c => {
            return Err(ParserError::new(
                "parse_v_for_value",
                format!(
                    "expected opening of argument value ('\"' or \"'\") but got '{}'",
                    c.to_string()
                ),
            ))
        }
    }

    // if true `foo in bar`, if false: `(foo, idx) in bar` or `(foo) in bar`
    let mut is_single = false;

    // Look for the start of the name
    // `(foo) in bar`
    //   ^- Find this
    match p.must_read_one_skip_spacing()? {
        '(' => {
            let c = p.must_read_one_skip_spacing()?;
            if !c.is_ascii_lowercase() && !c.is_ascii_uppercase() && c <= '}' {
                return Err(ParserError::new(
                    "parse_v_for_value",
                    format!("unexpected character '{}'", c.to_string()),
                ));
            }
        }
        c if c.is_ascii_lowercase() || c.is_ascii_uppercase() || c > '}' => {
            is_single = true;
        }
        c => {
            return Err(ParserError::new(
                "parse_v_for_value",
                format!("unexpected character '{}'", c.to_string()),
            ))
        }
    }

    let (mut c, value_location) = js::parse_name(p)?;
    if is_space(c) {
        c = p.must_read_one_skip_spacing()?;
    }

    let mut result = ParsedVFor {
        value: value_location.string(p),
        key: None,
        index: None,
        list: String::new(),
    };

    if !is_single {
        if c == ',' {
            // Read the key
            // `v-for"(value, key) in list"`
            //                ^- That one
            p.must_read_one_skip_spacing()?;

            let (next_c, key_location) = js::parse_name(p)?;
            c = next_c;
            result.key = Some(key_location.string(p));

            if is_space(c) {
                c = p.must_read_one_skip_spacing()?;
            }

            if c == ',' {
                // Read the index
                // `v-for"(value, key, index) in object"`
                //                     ^- That one
                p.must_read_one_skip_spacing()?;

                let (next_c, index_location) = js::parse_name(p)?;
                c = next_c;
                result.index = Some(index_location.string(p));

                if is_space(c) {
                    c = p.must_read_one_skip_spacing()?;
                }
            }
        }

        if c != ')' {
            return Err(ParserError::new(
                "parse_v_for_value",
                format!("expected ')' but got '{}'", c.to_string()),
            ));
        }
        c = p.must_read_one_skip_spacing()?;
    }

    if c != 'i' {
        return Err(ParserError::new(
            "parse_v_for_value",
            format!(
                "expected v-for value to be \".. in ..\" but got '{}'",
                c.to_string()
            ),
        ));
    }
    c = p.must_read_one()?;
    if c != 'n' {
        return Err(ParserError::new(
            "parse_v_for_value",
            format!(
                "expected v-for value to be \".. in ..\" but got '{}'",
                c.to_string()
            ),
        ));
    }
    c = p.must_read_one()?;
    if !is_space(c) {
        return Err(ParserError::new(
            "parse_v_for_value",
            format!(
                "expected v-for value to be \".. in ..\" but got '{}'",
                c.to_string()
            ),
        ));
    }

    let start = p.current_char;
    let replacements = js::parse_template_arg(p, closure)?;
    let list_location = SourceLocation(start, p.current_char - 1);
    result.list = js::add_vm_references(p, &list_location, &replacements);

    Ok(result)
}

pub enum VueArgKind {
    Default,
    Bind,
    On,
    Text,
    Html,
    If,
    Else,
    ElseIf,
    For,
    Model,
    Slot,
    Pre,
    Cloak,
    Once,
    CustomDirective(String),
}

#[derive(Debug, Clone)]
pub enum VueTagModifier {
    For(ParsedVFor),
    If(String),
    ElseIf(String),
    Else,
}

impl VueTagModifier {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::For(_) => "v-for",
            Self::If(_) => "v-if",
            Self::ElseIf(_) => "v-else-if",
            Self::Else => "v-else",
        }
    }
}
