pub mod error;
pub mod js;
pub mod style;
pub mod template;
pub mod tests;
pub mod utils;

use error::ParserError;
use std::collections::HashMap;
use utils::is_space;

#[derive(Debug)]
pub struct Parser {
    pub source_chars: Vec<char>,
    pub source_chars_len: usize,
    pub current_char: usize,
    pub template: Option<Template>,
    pub script: Option<Script>,
    pub styles: Vec<Style>,

    pub local_variables: HashMap<String, u16>,
}

#[derive(Debug, Clone)]
pub struct Script {
    pub lang: Option<String>,
    pub default_export_location: Option<SourceLocation>,
    pub content: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct Template {
    pub lang: Option<String>,
    pub content: Vec<template::Child>,
}

#[derive(Debug, Clone)]
pub struct Style {
    pub lang: Option<String>,
    pub scoped: bool,
    pub scoped_selector_injection_points: Option<Vec<usize>>,
    pub content: SourceLocation,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        let source_chars: Vec<char> = source.chars().collect();
        let source_chars_len = source_chars.len();
        return Self {
            local_variables: HashMap::new(),
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

    fn seek_one_or_null(&mut self) -> char {
        self.seek_one().unwrap_or(0 as char)
    }

    fn seek_one(&mut self) -> Option<char> {
        if self.source_chars_len == self.current_char {
            None
        } else {
            Some(self.source_chars[self.current_char])
        }
    }

    fn must_seek_one(&mut self) -> Result<char, ParserError> {
        match self.seek_one() {
            Some(v) => Ok(v),
            None => Err(ParserError::eof(self)),
        }
    }

    fn read_one(&mut self) -> Option<char> {
        let resp = self.seek_one()?;
        self.current_char += 1;
        return Some(resp);
    }

    fn must_read_one(&mut self) -> Result<char, ParserError> {
        match self.read_one() {
            Some(v) => Ok(v),
            None => Err(ParserError::eof(self)),
        }
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
                        template::TagType::Comment | template::TagType::DocType | template::TagType::Open => {},
                        template::TagType::Close => return Err(ParserError::new(self, "found tag closure without open")),
                        template::TagType::OpenAndClose => return Err(ParserError::new(self, "tag type not allowed on top level")),
                    };

                    let lang: Option<String> = top_level_tag.1.args.has_attr_or_prop_with_string("lang");

                    match top_level_tag.0 {
                        TopLevelTag::DocType | TopLevelTag::Comment => continue,
                        TopLevelTag::Template => {
                            if self.template.is_some() {
                                return Err(ParserError::new(self, "can't have multiple templates in your code"));
                            }
                            let children = template::compile(self)?;

                            self.template = Some(Template{
                                lang,
                                content: children,
                            });
                        },
                        TopLevelTag::Script => {
                            if self.script.is_some() {
                                return Err(ParserError::new(self, "can't have multiple scripts in your code"));
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
                            let scoped = match top_level_tag.1.args.has_attr_or_prop("scoped") {
                                Some("true") => true,
                                _ => false,
                            };

                            let (content_location, scoped_selector_injection_points) = match (scoped, lang.as_ref().map(|v| v.as_str())) {
                                (true, None) | (true, Some("css")) => {
                                    let start = self.current_char;
                                    let injection_points = style::parse_scoped_css(self, style::SelectorsEnd::StyleClosure)?;
                                    (SourceLocation(start, self.current_char-8), Some(injection_points))
                                }
                                _ => (SourceLocation(self.current_char, self.look_for("</style>".chars().collect())?.0), None),
                            };

                            self.styles.push(Style{
                                lang,
                                scoped,
                                scoped_selector_injection_points,
                                content: content_location,
                            });
                        },
                    }
                },
                c => return Err(ParserError::new(self, format!("found invalid character in source: '{}', expected <template ..> <script ..> or <style ..>", c))),
            };
        }
        Ok(())
    }

    fn look_for(&mut self, data: Vec<char>) -> Result<SourceLocation, ParserError> {
        let data_len = data.len();
        if data_len == 0 {
            return Err(ParserError::new(self, "cannot look for zero length data"));
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

    fn parse_top_level_tag(&mut self) -> Result<(TopLevelTag, template::Tag), ParserError> {
        let parsed_tag = template::parse_tag(self, false)?;

        match parsed_tag.type_ {
            template::TagType::DocType => return Ok((TopLevelTag::DocType, parsed_tag)),
            template::TagType::Comment => return Ok((TopLevelTag::Comment, parsed_tag)),
            _ => {}
        }

        let top_level_tag = if parsed_tag.name.eq(self, &mut "template".chars()) {
            TopLevelTag::Template
        } else if parsed_tag.name.eq(self, &mut "script".chars()) {
            TopLevelTag::Script
        } else if parsed_tag.name.eq(self, &mut "style".chars()) {
            TopLevelTag::Style
        } else {
            return Err(ParserError::new(
                self,
                format!(
                    "tag <{}> is not allowed on the top level ",
                    parsed_tag.name.string(self)
                ),
            ));
        };

        Ok((top_level_tag, parsed_tag))
    }

    fn parse_quotes(
        &mut self,
        kind: QuoteKind,
        global_references: &mut Option<Vec<SourceLocation>>,
    ) -> Result<(), ParserError> {
        let quote_char = match kind {
            QuoteKind::JSDouble => '"',
            QuoteKind::JSSingle => '\'',
            QuoteKind::JSBacktick => '`',
        };

        let is_js_backtick = if let QuoteKind::JSBacktick = kind {
            true
        } else {
            false
        };

        loop {
            match self.must_read_one()? {
                '\\' => {
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
    JSDouble,   // "
    JSSingle,   // '
    JSBacktick, // `
}

#[derive(Debug, Clone)]
pub struct SourceLocation(pub usize, pub usize);

impl SourceLocation {
    fn empty() -> Self {
        SourceLocation(0, 0)
    }
    pub fn chars<'a>(&self, parser: &'a Parser) -> &'a [char] {
        if self.is_empty() {
            &[]
        } else {
            &parser.source_chars[self.0..self.1]
        }
    }
    // pub fn chars_vec(&self, parser: &Parser) -> Vec<char> {
    //     if self.is_empty() {
    //         parser.source_chars[self.0..self.1].into()
    //     } else {
    //         Vec::new()
    //     }
    // }
    pub fn write_to_vec(&self, parser: &Parser, dest: &mut Vec<char>) {
        for c in self.chars(parser) {
            dest.push(*c);
        }
    }
    pub fn write_to_vec_escape(
        &self,
        parser: &Parser,
        dest: &mut Vec<char>,
        quote: char,
        escape_char: char,
    ) {
        for c in self.chars(parser) {
            let cc = *c;
            if cc == quote || cc == escape_char {
                dest.push(escape_char);
            }
            dest.push(*c);
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
    // pub fn starts_with(&self, parser: &Parser, mut other: impl Iterator<Item = char>) -> bool {
    //     let mut self_iter = self.chars(parser).iter();
    //     loop {
    //         match (self_iter.next(), other.next()) {
    //             (Some(a), Some(b)) if *a == b => continue,
    //             (_, None) => return true,
    //             _ => return false,
    //         }
    //     }
    // }
}

#[derive(Debug)]
enum TopLevelTag {
    DocType,
    Comment,
    Template,
    Script,
    Style,
}
