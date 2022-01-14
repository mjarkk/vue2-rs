pub mod error;
pub mod tests;

use error::ParserError;

const INPUT: &'static str = "
<template>
    <h1>Hello world</h1>
</template>

<script>
module.exports = {}
</script>

<style lang=\"stylus\" scoped>
h1
    color red
</style>
";

fn main() {
    match Parser::parse(INPUT) {
        Err(e) => panic!("{}", e.to_string()),
        Ok(v) => println!("{:#?}", v),
    }
}

#[derive(Debug)]
pub struct Parser {
    pub source_chars: Vec<char>,
    pub source_chars_len: usize,
    pub current_char: usize,
    pub template: Option<SourceLocation>,
    pub script: Option<SourceLocation>,
    pub styles: Vec<SourceLocation>,
}

impl Parser {
    pub fn parse(source: &str) -> Result<Self, ParserError> {
        let source_chars: Vec<char> = source.chars().collect();
        let source_chars_len = source_chars.len();
        let mut p = Self {
            source_chars,
            source_chars_len,
            current_char: 0,
            template: None,
            script: None,
            styles: Vec::new(),
        };
        p.execute()?;
        Ok(p)
    }
    fn seek_one(&mut self) -> Option<char> {
        if self.source_chars_len == self.current_char {
            None
        } else {
            Some(self.source_chars[self.current_char])
        }
    }
    fn read_one(&mut self) -> Option<char> {
        let resp = self.seek_one()?;
        self.current_char += 1;
        return Some(resp);
    }
    fn read_one_skip_spacing(&mut self) -> Option<char> {
        loop {
            let c = self.read_one()?;
            if !is_space(c) {
                return Some(c);
            }
        }
    }
    fn execute(&mut self) -> Result<(), ParserError> {
        while let Some(b) = self.read_one_skip_spacing() {
            match b {
                '<' => {
                    let top_level_tag = self.parse_top_level_tag()?;
                    match top_level_tag.1.type_ {
                        TagType::Close => return Err(ParserError::new("execute", "found tag closure without open")),
                        TagType::OpenAndClose => return Err(ParserError::new("execute", "tag type not allowed on top level")),
                        TagType::Open => {},
                    };

                    match top_level_tag.0 {
                        TopLevelTag::Template => {
                            if self.template.is_some() {
                                return Err(ParserError::new("execute", "can't have multiple templates in your code"));
                            }
                            let template_start = self.current_char;
                            let SourceLocation(template_end, _) = self.look_for("</template>".chars().collect())?;
                            self.template = Some(SourceLocation(template_start, template_end));
                        },
                        TopLevelTag::Script => {
                            if self.script.is_some() {
                                return Err(ParserError::new("execute", "can't have multiple scripts in your code"));
                            }
                            let script_start = self.current_char;
                            let SourceLocation(script_end, _) = self.look_for("</script>".chars().collect())?;
                            self.script = Some(SourceLocation(script_start, script_end));
                        },
                        TopLevelTag::Style => {
                            let style_start = self.current_char;
                            let SourceLocation(style_end, _) = self.look_for("</style>".chars().collect())?;
                            self.styles.push(SourceLocation(style_start, style_end));
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
            let c = self.read_one().ok_or(ParserError::eof("look_for"))?;
            if c != data[0] {
                continue;
            }

            let start_index = self.current_char - 1;
            for idx in 1..data_len {
                let c = self.read_one().ok_or(ParserError::eof("look_for"))?;
                if c != data[idx] {
                    continue 'outerLoop;
                }
            }
            return Ok(SourceLocation(start_index, self.current_char));
        }
    }
    fn parse_top_level_tag(&mut self) -> Result<(TopLevelTag, Tag), ParserError> {
        let parsed_tag = self.parse_tag()?;

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

        let mut c = match first_char {
            Some(c) => {
                start -= 1;
                c
            }
            None => self.read_one().ok_or(ParserError::eof("parse_name"))?,
        };
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {
                // do nothing
            }
            _ => return Err(ParserError::new("parse_name", no_name_err)),
        }

        loop {
            c = self.read_one().ok_or(ParserError::eof("parse_name"))?;

            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => {}
                c => return Ok((SourceLocation(start, self.current_char - 1), c)),
            }
        }
    }
    // parse_tag is expected to be next to the open indicator (<) at the first character of the tag name
    // TODO support upper case tag names
    fn parse_tag(&mut self) -> Result<Tag, ParserError> {
        let mut tag = Tag {
            type_: TagType::Open,
            name: SourceLocation(self.current_char, 0),
            args: Vec::new(),
        };

        let mut is_close_tag = false;
        let mut c = self
            .seek_one()
            .ok_or(ParserError::eof("parse_tag check closure tag"))?;
        if c == '/' {
            tag.type_ = TagType::Close;
            self.current_char += 1;
            is_close_tag = true;
        }

        // Parse names
        loop {
            c = self.read_one().ok_or(ParserError::eof("parse_tag name"))?;
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' => {}
                _ => {
                    self.current_char -= 1;
                    tag.name.1 = self.current_char;
                    break;
                }
            };
        }

        // Parse args
        loop {
            c = self
                .read_one_skip_spacing()
                .ok_or(ParserError::eof("parse_tag args"))?;

            match c {
                '>' => return Ok(tag),
                '/' => {
                    return if is_close_tag {
                        Err(ParserError::new("parse_tag", "Invalid html tag"))
                    } else {
                        c = self
                            .read_one_skip_spacing()
                            .ok_or(ParserError::eof("parse_tag tag closure"))?;
                        if c == '>' {
                            tag.type_ = TagType::OpenAndClose;
                            Ok(tag)
                        } else {
                            Err(ParserError::new(
                                "parse_tag",
                                format!("Expected element closure '>' but got '{}'", c),
                            ))
                        }
                    }
                }
                _ => {}
            }

            let (key_location, mut c) =
                self.parse_name(Some(c), format!("unexpected character '{}'", c))?;

            let value_location = if c != '=' {
                self.current_char -= 1;
                None
            } else {
                // Parse arg value
                c = self
                    .read_one()
                    .ok_or(ParserError::eof("parse_tag arg value"))?;

                let value_location = match c {
                    '>' => return Ok(tag),
                    '/' => {
                        self.current_char -= 1;
                        continue;
                    }
                    '"' => {
                        let start = self.current_char;
                        self.parse_quotes(QuoteKind::HTMLDouble)?;
                        SourceLocation(start, self.current_char - 1)
                    }
                    '\'' => {
                        let start = self.current_char;
                        self.parse_quotes(QuoteKind::HTMLSingle)?;
                        SourceLocation(start, self.current_char - 1)
                    }
                    c if is_space(c) => continue,
                    _ => {
                        let start = self.current_char - 1;
                        loop {
                            c = self
                                .read_one()
                                .ok_or(ParserError::eof("parse_tag arg value"))?;

                            match c {
                                '>' | '/' => {
                                    break;
                                }
                                c if is_space(c) => {
                                    break;
                                }
                                _ => {}
                            }

                            break;
                        }
                        self.current_char -= 1;
                        SourceLocation(start, self.current_char)
                    }
                };

                Some(value_location)
            };

            tag.args.push(TagArg {
                key: key_location,
                value: value_location,
            });
        }
    }
    fn parse_quotes(&mut self, kind: QuoteKind) -> Result<(), ParserError> {
        let quote_char = match kind {
            QuoteKind::HTMLDouble => '"',
            QuoteKind::HTMLSingle => '\'',
        };

        loop {
            let c = self.read_one().ok_or(ParserError::eof("parse_quote"))?;
            if c == quote_char {
                return Ok(());
            }
        }
    }
}

#[derive(Debug)]
enum QuoteKind {
    HTMLDouble, // "
    HTMLSingle, // '
}

#[derive(Debug)]
pub struct Tag {
    type_: TagType,
    name: SourceLocation,
    args: Vec<TagArg>,
}

#[derive(Debug)]
pub struct TagArg {
    pub key: SourceLocation,
    pub value: Option<SourceLocation>,
}

#[derive(Debug)]
pub struct SourceLocation(usize, usize);

impl SourceLocation {
    pub fn chars<'a>(&self, parser: &'a Parser) -> &'a [char] {
        &parser.source_chars[self.0..self.1]
    }
    pub fn string(&self, parser: &Parser) -> String {
        self.chars(parser).iter().collect()
    }
    pub fn len(&self) -> usize {
        self.1 - self.0
    }
    pub fn eq(&self, parser: &Parser, other: &mut impl Iterator<Item = char>) -> bool {
        let mut self_iter = self.chars(parser).iter();
        loop {
            match (self_iter.next(), other.next()) {
                (Some(a), Some(b)) if *a == b => continue,
                (None, None) => return true,
                _ => return false,
            }
        }
    }
}

#[derive(Debug)]
enum TagType {
    Open,
    OpenAndClose,
    Close,
}

#[derive(Debug)]
enum TopLevelTag {
    Template,
    Script,
    Style,
}

fn is_space(c: char) -> bool {
    match c {
        ' ' | '\t' | '\n' | '\r' => true,
        _ => false,
    }
}
