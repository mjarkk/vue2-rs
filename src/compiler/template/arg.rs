use super::super::utils::is_space;
use super::super::{js, Parser, ParserError, SourceLocation};
use super::{add_or_set, StaticOrJS, VueTagArgs};

pub fn new_try_parse(
    p: &mut Parser,
    mut c: char,
    result: &mut VueTagArgs,
    v_else_allowed: bool,
    is_custom_component: bool,
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
        "v-for" => (ExpectValue::Yes, false, false, VueArgKind::For),
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
            p,
            format!(
                "target set on argument {} but is not allowed",
                name_result.name
            ),
        ));
    }

    if !modifier_allowed && name_result.modifiers.is_some() {
        return Err(ParserError::new(
            p,
            format!(
                "modifier set on argument {} but is not allowed",
                name_result.name
            ),
        ));
    }

    match expect_value {
        ExpectValue::Yes if !name_result.parse_value_next => {
            return Err(ParserError::new(
                p,
                format!(
                    "expected an argument value for {} but got none",
                    name_result.name
                ),
            ))
        }
        ExpectValue::No if name_result.parse_value_next => {
            return Err(ParserError::new(
                p,
                format!(
                    "expected NO argument value for {} but got one",
                    name_result.name
                ),
            ))
        }
        _ => {}
    };

    match arg_kind {
        VueArgKind::Default => {
            let (contents, next_c) = might_get_arg_value(p, &name_result, c)?;
            c = next_c;
            result.set_default_or_bind(
                p,
                name_result,
                if let Some(value) = contents {
                    StaticOrJS::Static(value)
                } else {
                    StaticOrJS::Non
                },
                false,
            )?;
            result.has_js_component_args = true;
        }
        VueArgKind::Bind => {
            let (content, next_c) = get_arg_js_value(p)?;
            c = next_c;
            result.set_default_or_bind(p, name_result, StaticOrJS::Bind(content), true)?;
            result.has_js_component_args = true;
        }
        VueArgKind::On => {
            let (content, next_c) = get_arg_js_value(p)?;
            c = next_c;
            let target = if let Some(target) = name_result.target {
                target
            } else {
                return Err(ParserError::new(p, "expected a v-on target"));
            };
            add_or_set(&mut result.on, (target, content));
            result.has_js_component_args = true;
        }
        VueArgKind::Text => {
            let (content, next_c) = get_arg_js_value(p)?;
            c = next_c;
            add_or_set(
                &mut result.dom_props,
                (String::from("textContent"), content),
            );
            result.has_js_component_args = true;
        }
        VueArgKind::Html => {
            let (content, next_c) = get_arg_js_value(p)?;
            c = next_c;
            add_or_set(&mut result.dom_props, (String::from("innerHTML"), content));
            result.has_js_component_args = true;
        }
        VueArgKind::If => {
            let (content, next_c) = get_arg_js_value(p)?;
            c = next_c;
            result.set_modifier(p, VueTagModifier::If(content))?;
        }
        VueArgKind::Else => {
            if !v_else_allowed {
                return Err(ParserError::new(
                    p,
                    "v-else can only be used after en v-if(-else) element",
                ));
            }
            result.set_modifier(p, VueTagModifier::Else)?;
        }
        VueArgKind::ElseIf => {
            if !v_else_allowed {
                return Err(ParserError::new(
                    p,
                    "v-else-if can only be used after en v-if element",
                ));
            }
            let (content, next_c) = get_arg_js_value(p)?;
            c = next_c;
            result.set_modifier(p, VueTagModifier::ElseIf(content))?;
        }
        VueArgKind::For => {
            let content = parse_v_for_value(p)?;

            // Remember the local variables set by the v-for
            let mut local_variables_list: Vec<String> = vec![content.value.clone()];
            if let Some(key) = content.key.as_ref() {
                local_variables_list.push(key.clone());
                if let Some(index) = content.index.as_ref() {
                    local_variables_list.push(index.clone());
                }
            }
            result.new_local_variables = Some(local_variables_list);

            c = p.must_read_one()?;
            result.set_modifier(p, VueTagModifier::For(content))?;
        }
        VueArgKind::Model => {
            let (content, next_c) = get_arg_js_value(p)?;
            c = next_c;

            add_or_set(
                &mut result.on,
                (
                    String::from("input"),
                    format!(
                        "$event.target.composing?undefined:{}=$event.target.value",
                        &content
                    ),
                ),
            );

            if is_custom_component {
                if let Some(target) = name_result.target.as_ref() {
                    add_or_set(
                        &mut result.attrs_or_props,
                        (target.clone(), StaticOrJS::Bind(content.to_string())),
                    );
                } else {
                    add_or_set(
                        &mut result.attrs_or_props,
                        (String::from("value"), StaticOrJS::Bind(content.to_string())),
                    );
                }
            } else {
                add_or_set(
                    &mut result.dom_props,
                    (String::from("value"), content.to_string()),
                );
            }

            add_or_set(&mut result.directives, (name_result, content));
            result.has_js_component_args = true;
        }
        VueArgKind::Slot => {
            todo!("support slot");
        }
        VueArgKind::Pre => {
            todo!("support pre");
        }
        VueArgKind::Cloak => {
            todo!("support cloak");
        }
        VueArgKind::Once => {
            todo!("support once");
        }
        VueArgKind::CustomDirective(_) => {
            let (content, next_c) = get_arg_js_value(p)?;
            c = next_c;

            add_or_set(&mut result.directives, (name_result, content));
            result.has_js_component_args = true;
        }
    }

    Ok(Some(c))
}

fn get_arg_js_value(p: &mut Parser) -> Result<(String, char), ParserError> {
    let closure = p.must_read_one()?;
    match closure {
        '"' | '\'' => {} // Ok
        c => {
            return Err(ParserError::new(
                p,
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
    let c = p.must_read_one()?;

    let value = js::add_vm_references(p, &sl, &replacements);
    Ok((value, c))
}

fn escape_string_to_js_string_or(input: Option<String>, or: String) -> String {
    if let Some(mut input) = input {
        escape_string_to_js_string(&mut input);
        input
    } else {
        or
    }
}

fn escape_string_to_js_string(input: &mut String) {
    input.push('"');
    let input_len = input.len();

    for idx in (0..input_len).rev().skip(1) {
        match input.get(idx..idx + 1) {
            Some("\"") | Some("\\") => {
                input.insert(idx, '\\');
            }
            _ => {}
        }
    }
    input.insert(0, '"');
}

fn might_get_arg_value(
    p: &mut Parser,
    name: &ParseArgNameResult,
    c: char,
) -> Result<(Option<String>, char), ParserError> {
    Ok(if name.parse_value_next {
        let (contents, c) = get_arg_value(p)?;
        (Some(contents), c)
    } else {
        (None, c)
    })
}

fn get_arg_value(p: &mut Parser) -> Result<(String, char), ParserError> {
    let mut c = p.must_read_one_skip_spacing()?;
    let quote = match c {
        '\'' => '\'',
        '"' => '"',
        _ => {
            let mut resp = c.to_string();
            loop {
                c = p.must_read_one()?;
                if is_space(c) || c == '/' || c == '>' {
                    return Ok((resp, c));
                }
                resp.push(c);
            }
        }
    };

    let mut resp = String::new();
    loop {
        c = p.must_read_one()?;
        if c == quote {
            break;
        }
        resp.push(c);
    }

    Ok((resp, p.must_read_one()?))
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

#[derive(Debug, Clone)]
pub struct ParseArgNameResult {
    parse_value_next: bool,
    // name details
    pub name: String,                   // `v-bind` of `v-bind:some_value.trim`
    pub target: Option<String>,         // `some_value` of `v-bind:some_value.trim`
    pub modifiers: Option<Vec<String>>, // `trim` of `v-bind:some_value.trim`
}

fn parse_arg_name(p: &mut Parser, mut c: char) -> Result<(ParseArgNameResult, char), ParserError> {
    let invalid_character_err = |p: &mut Parser, c: char| {
        Err(ParserError::new(
            p,
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
                    '/' | '>' => break,
                    c => return invalid_character_err(p, c),
                }
            }
        }
        c => return invalid_character_err(p, c),
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
                '/' | '>' => break,
                c => return invalid_character_err(p, c),
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
                        modifiers.push(modifier);
                        break 'outer;
                    }
                    '/' | '>' => break,
                    c => return invalid_character_err(p, c),
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
                p,
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
                    p,
                    format!("unexpected character '{}'", c.to_string()),
                ));
            }
        }
        c if c.is_ascii_lowercase() || c.is_ascii_uppercase() || c > '}' => {
            is_single = true;
        }
        c => {
            return Err(ParserError::new(
                p,
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
                p,
                format!("expected ')' but got '{}'", c.to_string()),
            ));
        }
        c = p.must_read_one_skip_spacing()?;
    }

    if c != 'i' {
        return Err(ParserError::new(
            p,
            format!(
                "expected v-for value to be \".. in ..\" but got '{}'",
                c.to_string()
            ),
        ));
    }
    c = p.must_read_one()?;
    if c != 'n' {
        return Err(ParserError::new(
            p,
            format!(
                "expected v-for value to be \".. in ..\" but got '{}'",
                c.to_string()
            ),
        ));
    }
    c = p.must_read_one()?;
    if !is_space(c) {
        return Err(ParserError::new(
            p,
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
