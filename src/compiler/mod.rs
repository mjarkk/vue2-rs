pub mod error;
pub mod js;
pub mod template;
pub mod tests;
pub mod utils;

use error::ParserError;
use utils::is_space;

#[derive(Debug)]
pub struct Parser {
    pub source_chars: Vec<char>,
    pub source_chars_len: usize,
    pub current_char: usize,
    pub template: Option<Template>,
    pub script: Option<Script>,
    pub styles: Vec<Style>,
}

#[derive(Debug, Clone)]
pub struct Script {
    pub lang: Option<SourceLocation>,
    pub default_export_location: Option<SourceLocation>,
    pub content: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct Template {
    pub lang: Option<SourceLocation>,
    pub content: Vec<template::Child>,
}

#[derive(Debug, Clone)]
pub struct Style {
    pub lang: Option<SourceLocation>,
    pub scoped: bool,
    pub content: SourceLocation,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        let source_chars: Vec<char> = source.chars().collect();
        let source_chars_len = source_chars.len();
        return Self {
            source_chars,
            source_chars_len,
            current_char: 0,
            template: None,
            script: None,
            styles: Vec::new(),
        };
    }

    pub fn new_and_parse(source: &str) -> Result<Self, ParserError> {
        let mut p = Self::new(source);
        p.parse()?;
        Ok(p)
    }

    fn seek_one(&mut self) -> Option<char> {
        if self.source_chars_len == self.current_char {
            None
        } else {
            Some(self.source_chars[self.current_char])
        }
    }

    fn must_seek_one(&mut self) -> Result<char, ParserError> {
        self.seek_one().ok_or(ParserError::eof("must_seek_one"))
    }

    fn read_one(&mut self) -> Option<char> {
        let resp = self.seek_one()?;
        self.current_char += 1;
        return Some(resp);
    }

    fn must_read_one(&mut self) -> Result<char, ParserError> {
        self.read_one().ok_or(ParserError::eof("must_read_one"))
    }

    fn read_one_skip_spacing(&mut self) -> Option<char> {
        loop {
            let c = self.read_one()?;
            if !is_space(c) {
                return Some(c);
            }
        }
    }

    fn must_read_one_skip_spacing(&mut self) -> Result<char, ParserError> {
        loop {
            let c = self.must_read_one()?;
            if !is_space(c) {
                return Ok(c);
            }
        }
    }

    pub fn parse(&mut self) -> Result<(), ParserError> {
        while let Some(b) = self.read_one_skip_spacing() {
            match b {
                '<' => {
                    let top_level_tag = self.parse_top_level_tag()?;
                    match top_level_tag.1.type_ {
                        TagType::DocType => {},
                        TagType::Close => return Err(ParserError::new("execute", "found tag closure without open")),
                        TagType::OpenAndClose => return Err(ParserError::new("execute", "tag type not allowed on top level")),
                        TagType::Open => {},
                    };

                    let lang: Option<SourceLocation> = if let Some(lang_arg) = top_level_tag.1.arg(self, "lang") {
                        let arg_value = lang_arg.value();
                        if arg_value.is_empty() {
                            None
                        } else {
                            Some(arg_value)
                        }
                    } else {
                        None
                    };

                    match top_level_tag.0 {
                        TopLevelTag::DocType => continue,
                        TopLevelTag::Template => {
                            if self.template.is_some() {
                                return Err(ParserError::new("execute", "can't have multiple templates in your code"));
                            }
                            let children = template::compile(self)?;

                            self.template = Some(Template{
                                lang,
                                content: children,
                            });
                        },
                        TopLevelTag::Script => {
                            if self.script.is_some() {
                                return Err(ParserError::new("execute", "can't have multiple scripts in your code"));
                            }
                            let script_start = self.current_char;

                            let default_export_location = js::compile_script_content(self)?;
                            let content = SourceLocation(script_start, self.current_char - "</script>".len());

                            self.script = Some(Script{
                                lang,
                                default_export_location,
                                content,
                            });
                        },
                        TopLevelTag::Style => {
                            let style_start = self.current_char;
                            let SourceLocation(style_end, _) = self.look_for("</style>".chars().collect())?;

                            let scoped =  top_level_tag.1.arg(self, "scoped").is_some();

                            self.styles.push(Style{
                                lang,
                                scoped,
                                content: SourceLocation(style_start, style_end),
                            });
                        },
                    }
                },
                c => return Err(ParserError::new("execute", format!("found invalid character in source: '{}', expected <template ..> <script ..> or <style ..>", c))),
            };
        }
        Ok(())
    }

    fn look_for(&mut self, data: Vec<char>) -> Result<SourceLocation, ParserError> {
        let data_len = data.len();
        if data_len == 0 {
            return Err(ParserError::new(
                "look_for",
                "cannot look for zero length data",
            ));
        }
        'outerLoop: loop {
            if self.must_read_one()? != data[0] {
                continue;
            }

            let start_index = self.current_char - 1;
            for idx in 1..data_len {
                if self.must_read_one()? != data[idx] {
                    continue 'outerLoop;
                }
            }
            return Ok(SourceLocation(start_index, self.current_char));
        }
    }

    fn parse_top_level_tag(&mut self) -> Result<(TopLevelTag, Tag), ParserError> {
        let parsed_tag = template::parse_tag(self)?;
        if let TagType::DocType = parsed_tag.type_ {
            return Ok((TopLevelTag::DocType, parsed_tag));
        }

        let top_level_tag = if parsed_tag.name.eq(self, &mut "template".chars()) {
            TopLevelTag::Template
        } else if parsed_tag.name.eq(self, &mut "script".chars()) {
            TopLevelTag::Script
        } else if parsed_tag.name.eq(self, &mut "style".chars()) {
            TopLevelTag::Style
        } else {
            return Err(ParserError::new(
                "parse_top_level_tag",
                format!(
                    "tag <{}> is not allowed on the top level ",
                    parsed_tag.name.string(self)
                ),
            ));
        };

        Ok((top_level_tag, parsed_tag))
    }

    fn parse_name(
        &mut self,
        first_char: Option<char>,
        no_name_err: String,
    ) -> Result<(SourceLocation, char), ParserError> {
        let mut start = self.current_char;

        let c = match first_char {
            Some(c) => {
                start -= 1;
                c
            }
            None => self.must_read_one()?,
        };
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {
                // do nothing
            }
            _ => return Err(ParserError::new("parse_name", no_name_err)),
        }

        loop {
            match self.must_read_one()? {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {}
                c => return Ok((SourceLocation(start, self.current_char - 1), c)),
            }
        }
    }

    // Try_parse_arg parses a key="value" , :key="value" , v-bind:key="value" , v-on:key="value" and @key="value"
    // It returns Ok(None) if first_char is not a char expected as first character of a argument
    fn try_parse_arg(&mut self, mut c: char) -> Result<Option<(TagArg, char)>, ParserError> {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '@' | ':' | '_' => {}
            _ => return Ok(None),
        };

        let mut key_location = SourceLocation(self.current_char - 1, 0);

        let mut has_value = false;
        loop {
            c = self.must_read_one()?;
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | ':' => {}
                '=' => {
                    let c = self.must_seek_one()?;
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
        key_location.1 = self.current_char - 1;

        let value_location = if has_value {
            // Parse the argument
            Some(match self.must_read_one()? {
                '"' => {
                    let start = self.current_char;
                    self.parse_quotes(QuoteKind::HTMLDouble, &mut None)?;
                    let sl = SourceLocation(start, self.current_char - 1);
                    c = self.must_read_one()?;
                    sl
                }
                '\'' => {
                    let start = self.current_char;
                    self.parse_quotes(QuoteKind::HTMLSingle, &mut None)?;
                    let sl = SourceLocation(start, self.current_char - 1);
                    c = self.must_read_one()?;
                    sl
                }
                _ => {
                    let start = self.current_char - 1;
                    loop {
                        c = self.must_read_one()?;
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
                    SourceLocation(start, self.current_char - 1)
                }
            })
        } else {
            None
        };

        if key_location.starts_with(self, "v-".chars()) {
            // parse vue spesific tag
            key_location.0 += 2;

            let vue_directives: &[(
                &'static str,
                bool,
                fn(k: SourceLocation, v: SourceLocation) -> TagArg,
            )] = &[
                ("if", true, |_, v| TagArg::If(v)),
                ("for", true, |_, v| TagArg::For(v)),
                ("pre", true, |_, v| TagArg::Pre(v)),
                ("else", false, |_, _| TagArg::Else),
                ("slot", true, |_, v| TagArg::Slot(v)),
                ("text", true, |_, v| TagArg::Text(v)),
                ("html", true, |_, v| TagArg::Html(v)),
                ("show", true, |_, v| TagArg::Show(v)),
                ("once", false, |_, _| TagArg::Once),
                ("model", true, |_, v| TagArg::Model(v)),
                ("cloak", true, |_, v| TagArg::Cloak(v)),
                ("else-if", true, |_, v| TagArg::ElseIf(v)),
                ("bind", true, |k, v| TagArg::Bind(k, v)),
                ("on", true, |k, v| TagArg::On(k, v)),
            ];

            let mut vue_directives_match_input = Vec::with_capacity(vue_directives.len());
            for e in vue_directives.iter() {
                vue_directives_match_input.push(e.0.chars());
            }

            if let Some(idx) = key_location.eq_some(self, true, vue_directives_match_input) {
                let (key, expects_argument, make_result_tag) = vue_directives[idx];

                if has_value != expects_argument {
                    Err(ParserError::new(
                        "try_parse_arg",
                        if expects_argument {
                            format!("expected an argument value for \"v-{}\"", key)
                        } else {
                            format!("expected no argument value for \"v-{}\"", key)
                        },
                    ))
                } else {
                    key_location.0 += key.len();
                    if self.source_chars[key_location.0] == ':' {
                        key_location.0 += 1;
                    }

                    let tag = make_result_tag(
                        key_location,
                        value_location.unwrap_or(SourceLocation::empty()),
                    );
                    Ok(Some((tag, c)))
                }
            } else {
                key_location.0 -= 2;
                Err(ParserError::new(
                    "try_parse_arg",
                    format!("unknown vue argument \"{}\"", key_location.string(self)),
                ))
            }
        } else {
            Ok(Some((TagArg::Default(key_location, value_location), c)))
        }
    }

    fn parse_quotes(
        &mut self,
        kind: QuoteKind,
        global_references: &mut Option<Vec<SourceLocation>>,
    ) -> Result<(), ParserError> {
        let (quote_char, escape): (char, bool) = match kind {
            QuoteKind::HTMLDouble => ('"', false),
            QuoteKind::HTMLSingle => ('\'', false),
            QuoteKind::JSDouble => ('"', true),
            QuoteKind::JSSingle => ('\'', true),
            QuoteKind::JSBacktick => ('`', true),
        };

        let is_js_backtick = if let QuoteKind::JSBacktick = kind {
            true
        } else {
            false
        };

        loop {
            match self.must_read_one()? {
                '\\' if escape => {
                    // Skip one char
                    self.must_read_one()?;
                }
                '$' if is_js_backtick && self.must_seek_one()? == '{' => {
                    self.current_char += 1;
                    js::parse_block_like(self, '}', global_references)?;
                }
                c if c == quote_char => return Ok(()),
                _ => {}
            }
        }
    }
}

#[derive(Debug)]
enum QuoteKind {
    HTMLDouble, // "
    HTMLSingle, // '
    JSDouble,   // "
    JSSingle,   // '
    JSBacktick, // `
}

#[derive(Debug, Clone)]
pub struct Tag {
    type_: TagType,
    name: SourceLocation,
    args: Vec<TagArg>,
}

impl Tag {
    fn arg(&self, parser: &Parser, key: &str) -> Option<&TagArg> {
        for arg in self.args.iter() {
            if arg.key_eq(parser, key) {
                return Some(arg);
            }
        }
        None
    }
    fn is_custom_component(&self, parser: &Parser) -> bool {
        let html_elements = vec![
            "a".chars(),
            "abbr".chars(),
            "acronym".chars(),
            "address".chars(),
            "applet".chars(),
            "area".chars(),
            "article".chars(),
            "aside".chars(),
            "audio".chars(),
            "b".chars(),
            "base".chars(),
            "basefont".chars(),
            "bdi".chars(),
            "bdo".chars(),
            "big".chars(),
            "blockquote".chars(),
            "body".chars(),
            "br".chars(),
            "button".chars(),
            "canvas".chars(),
            "caption".chars(),
            "center".chars(),
            "cite".chars(),
            "code".chars(),
            "col".chars(),
            "colgroup".chars(),
            "data".chars(),
            "datalist".chars(),
            "dd".chars(),
            "del".chars(),
            "details".chars(),
            "dfn".chars(),
            "dialog".chars(),
            "dir".chars(),
            "div".chars(),
            "dl".chars(),
            "dt".chars(),
            "em".chars(),
            "embed".chars(),
            "fieldset".chars(),
            "figcaption".chars(),
            "figure".chars(),
            "font".chars(),
            "footer".chars(),
            "form".chars(),
            "frame".chars(),
            "frameset".chars(),
            "head".chars(),
            "header".chars(),
            "hgroup".chars(),
            "h1".chars(),
            "h2".chars(),
            "h3".chars(),
            "h4".chars(),
            "h5".chars(),
            "h6".chars(),
            "hr".chars(),
            "html".chars(),
            "i".chars(),
            "iframe".chars(),
            "img".chars(),
            "input".chars(),
            "ins".chars(),
            "kbd".chars(),
            "keygen".chars(),
            "label".chars(),
            "legend".chars(),
            "li".chars(),
            "link".chars(),
            "main".chars(),
            "map".chars(),
            "mark".chars(),
            "menu".chars(),
            "menuitem".chars(),
            "meta".chars(),
            "meter".chars(),
            "nav".chars(),
            "noframes".chars(),
            "noscript".chars(),
            "object".chars(),
            "ol".chars(),
            "optgroup".chars(),
            "option".chars(),
            "output".chars(),
            "p".chars(),
            "param".chars(),
            "picture".chars(),
            "pre".chars(),
            "progress".chars(),
            "q".chars(),
            "rp".chars(),
            "rt".chars(),
            "ruby".chars(),
            "s".chars(),
            "samp".chars(),
            "script".chars(),
            "section".chars(),
            "select".chars(),
            "small".chars(),
            "source".chars(),
            "span".chars(),
            "strike".chars(),
            "strong".chars(),
            "style".chars(),
            "sub".chars(),
            "summary".chars(),
            "sup".chars(),
            "svg".chars(),
            "table".chars(),
            "tbody".chars(),
            "td".chars(),
            "template".chars(),
            "textarea".chars(),
            "tfoot".chars(),
            "th".chars(),
            "thead".chars(),
            "time".chars(),
            "title".chars(),
            "tr".chars(),
            "track".chars(),
            "tt".chars(),
            "u".chars(),
            "ul".chars(),
            "var".chars(),
            "video".chars(),
            "wbr".chars(),
        ];

        self.name.eq_some(parser, false, html_elements).is_none()
    }
}

#[derive(Debug, Clone)]
pub enum TagArg {
    Default(SourceLocation, Option<SourceLocation>), // value="val"
    Bind(SourceLocation, SourceLocation),            // :value="val" and v-bind:value="val"
    On(SourceLocation, SourceLocation),              // @click and v-on:click="val"
    Text(SourceLocation),                            // v-text=""
    Html(SourceLocation),                            // v-html=""
    Show(SourceLocation),                            // v-show=""
    If(SourceLocation),                              // v-if=""
    Else,                                            // v-else
    ElseIf(SourceLocation),                          // v-else-if
    For(SourceLocation),                             // v-for=""
    Model(SourceLocation),                           // v-model=""
    Slot(SourceLocation),                            // v-slot=""
    Pre(SourceLocation),                             // v-pre=""
    Cloak(SourceLocation),                           // v-cloak=""
    Once,                                            // v-once
}

impl TagArg {
    pub fn insert_into_js_tag_args(
        &self,
        add_to: &mut template::JsTagArgs,
        is_custom_component: bool,
    ) {
        let todo = |v| todo!("support {}", v);

        match self {
            Self::Default(key, value) => {
                let kv = (key.clone(), value.clone());

                let add_to_list = if is_custom_component {
                    &mut add_to.static_props
                } else {
                    &mut add_to.static_attrs
                };

                if let Some(list) = add_to_list.as_mut() {
                    list.push(kv);
                } else {
                    *add_to_list = Some(vec![kv])
                }
            }
            Self::Bind(key, value) => {
                let kv = (key.clone(), value.clone());

                let add_to_list = if is_custom_component {
                    &mut add_to.js_props
                } else {
                    &mut add_to.js_attrs
                };

                if let Some(list) = add_to_list.as_mut() {
                    list.push(kv);
                } else {
                    *add_to_list = Some(vec![kv])
                }
            }
            Self::On(key, value) => {
                let kv = (key.clone(), value.clone());

                if let Some(on) = add_to.on.as_mut() {
                    on.push(kv);
                } else {
                    add_to.on = Some(vec![kv]);
                }
            }
            Self::Text(_) => todo("v-text"),
            Self::Html(_) => todo("v-html"),
            Self::Show(_) => todo("v-show"),
            Self::If(_) => todo("v-if"),
            Self::Else => todo("v-else"),
            Self::ElseIf(_) => todo("v-else-if"),
            Self::For(_) => todo("v-for"),
            Self::Model(_) => todo("v-model"),
            Self::Slot(_) => todo("v-slot"),
            Self::Pre(_) => todo("v-pre"),
            Self::Cloak(_) => todo("v-cloak"),
            Self::Once => todo("v-once"),
        }
    }
    fn key_eq(&self, parser: &Parser, key: &str) -> bool {
        match self {
            Self::Default(key_location, _) => key_location.eq(parser, key.chars()),
            Self::Bind(key_location, _) => {
                if key.starts_with(':') {
                    key_location.eq(parser, key.chars().skip(1))
                } else if key.starts_with("v-bind:") {
                    key_location.eq(parser, key.chars().skip(7))
                } else {
                    key_location.eq(parser, key.chars())
                }
            }
            Self::On(key_location, _) => {
                if key.starts_with('@') {
                    key_location.eq(parser, key.chars().skip(1))
                } else if key.starts_with("v-on:") {
                    key_location.eq(parser, key.chars().skip(5))
                } else {
                    key_location.eq(parser, key.chars())
                }
            }
            Self::Text(_) => key == "v-text",
            Self::Html(_) => key == "v-html",
            Self::Show(_) => key == "v-show",
            Self::If(_) => key == "v-if",
            Self::Else => key == "v-else",
            Self::ElseIf(_) => key == "v-else-if",
            Self::For(_) => key == "v-for",
            Self::Model(_) => key == "v-model",
            Self::Slot(_) => key == "v-slot",
            Self::Pre(_) => key == "v-pre",
            Self::Cloak(_) => key == "v-cloak",
            Self::Once => key == "v-once",
        }
    }
    fn value(&self) -> SourceLocation {
        match self {
            Self::Default(_, v) => v.clone().unwrap_or(SourceLocation::empty()),
            Self::On(_, v)
            | Self::Bind(_, v)
            | Self::Text(v)
            | Self::Html(v)
            | Self::Show(v)
            | Self::If(v)
            | Self::ElseIf(v)
            | Self::For(v)
            | Self::Model(v)
            | Self::Slot(v)
            | Self::Pre(v)
            | Self::Cloak(v) => v.clone(),
            Self::Once => SourceLocation::empty(),
            Self::Else => SourceLocation::empty(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceLocation(pub usize, pub usize);

impl SourceLocation {
    fn empty() -> Self {
        SourceLocation(0, 0)
    }
    pub fn offset_start(mut self, offset: isize) -> Self {
        self.0 = ((self.0 as isize) + offset) as usize;
        self
    }
    pub fn chars<'a>(&self, parser: &'a Parser) -> &'a [char] {
        if self.is_empty() {
            &[]
        } else {
            &parser.source_chars[self.0..self.1]
        }
    }
    pub fn chars_vec<'a>(&self, parser: &'a Parser) -> Vec<char> {
        if self.is_empty() {
            parser.source_chars[self.0..self.1].into()
        } else {
            Vec::new()
        }
    }
    pub fn string(&self, parser: &Parser) -> String {
        self.chars(parser).iter().collect()
    }
    pub fn is_empty(&self) -> bool {
        self.0 == self.1
    }
    pub fn len(&self) -> usize {
        self.1 - self.0
    }
    pub fn eq_self(&self, parser: &Parser, other: &Self) -> bool {
        self.len() == other.len() && self.chars(parser) == other.chars(parser)
    }
    pub fn eq(&self, parser: &Parser, mut other: impl Iterator<Item = char>) -> bool {
        let mut self_iter = self.chars(parser).iter();
        loop {
            match (self_iter.next(), other.next()) {
                (Some(a), Some(b)) if *a == b => continue,
                (None, None) => return true,
                _ => return false,
            }
        }
    }
    pub fn eq_some<IteratorT: Iterator<Item = char>>(
        &self,
        parser: &Parser,
        can_start_with: bool,
        others: Vec<IteratorT>,
    ) -> Option<usize> {
        let mut results: Vec<Option<IteratorT>> = Vec::with_capacity(others.len());
        for other in others {
            results.push(Some(other));
        }
        let mut disabled_entries = 0;

        let mut self_iter = self.chars(parser).iter();
        let mut return_idx: Option<usize> = None;
        loop {
            if let Some(a) = self_iter.next() {
                for idx in 0..results.len() {
                    let result = results.get_mut(idx);
                    match result {
                        Some(Some(iter)) => {
                            if let Some(b) = iter.next() {
                                if *a == b {
                                    continue;
                                }
                            } else if can_start_with {
                                return_idx = Some(idx);
                                continue;
                            }
                            *results.get_mut(idx).unwrap() = None;
                            disabled_entries += 1;
                        }
                        _ => {}
                    }
                }
                if disabled_entries == results.len() {
                    return return_idx;
                }
            } else {
                for idx in 0..results.len() {
                    let result = results.get_mut(idx);
                    match result {
                        Some(Some(iter)) => {
                            if iter.next().is_none() {
                                return Some(idx);
                            }
                        }
                        _ => {}
                    }
                }
                return None;
            }
        }
    }
    pub fn starts_with(&self, parser: &Parser, mut other: impl Iterator<Item = char>) -> bool {
        let mut self_iter = self.chars(parser).iter();
        loop {
            match (self_iter.next(), other.next()) {
                (Some(a), Some(b)) if *a == b => continue,
                (_, None) => return true,
                _ => return false,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum TagType {
    DocType,
    Open,
    OpenAndClose,
    Close,
}

impl TagType {
    fn to_string(&self) -> &'static str {
        match self {
            Self::DocType => "DOCTYPE",
            Self::Open => "open",
            Self::OpenAndClose => "inline",
            Self::Close => "close",
        }
    }
}

#[derive(Debug)]
enum TopLevelTag {
    DocType,
    Template,
    Script,
    Style,
}
