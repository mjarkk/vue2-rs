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
    if let Err(e) = Parser::parse(INPUT) {
        panic!("{}", e.to_string());
    }
}

const ERR_EOF: &'static str = "Unexpected EOF";

struct Parser {
    source_chars: Vec<char>,
    source_chars_len: usize,
    current_char: usize,
    template: usize,
    script: usize,
    styles: Vec<usize>,
}

impl Parser {
    fn parse(source: &str) -> Result<Self, String> {
        let source_chars: Vec<char> = source.chars().collect();
        let source_chars_len = source_chars.len();
        let mut p = Self {
            source_chars,
            source_chars_len,
            current_char: 0,
            template: 0,
            script: 0,
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
            self.read_one()?;

            match self.read_one()? {
                ' ' | '\t' | '\n' | '\r' => {}
                c => return Some(c),
            };
        }
    }
    fn execute(&mut self) -> Result<(), String> {
        while let Some(b) = self.read_one_skip_spacing() {
            match b {
                '<' => {
                    println!("{:?}", self.parse_top_level_tag()?);
                },
                c => return Err(format!("found invalid character in source: '{}', expected <template ..> <script ..> or <style ..>", c.to_string() )),
            };
        }
        Ok(())
    }
    fn parse_top_level_tag(&mut self) -> Result<(TopLevelTag, Tag), String> {
        let parsed_tag = self.parse_tag()?;

        let top_level_tag = match parsed_tag.name(self) {
            ['t', 'e', 'm', 'p', 'l', 'a', 't', 'e'] => TopLevelTag::Template,
            ['s', 'c', 'r', 'i', 'p', 't'] => TopLevelTag::Script,
            ['s', 't', 'y', 'l', 'e'] => TopLevelTag::Style,
            _ => {
                return Err(format!(
                    "unknown top level tag <{}>",
                    parsed_tag.name_string(self)
                ))
            }
        };

        Ok((top_level_tag, parsed_tag))
    }
    // parse_tag is expected to be next to the open indicator (<) at the first character of the tag name
    // TODO support upper case tag names
    fn parse_tag(&mut self) -> Result<Tag, String> {
        let mut tag = Tag {
            type_: TagType::Open,
            name_start: self.current_char,
            args: Vec::new(),
            name_end: 0,
        };

        let mut is_close_tag = false;
        let mut c = self.seek_one().ok_or(ERR_EOF)?;
        if c == '/' {
            tag.type_ = TagType::Close;
            self.current_char += 1;
            is_close_tag = true;
        }

        // Parse names
        loop {
            c = self.read_one().ok_or(ERR_EOF)?;
            if (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') {
                continue;
            }
            self.current_char -= 1;
            tag.name_end = self.current_char;
            break;
        }

        c = self.read_one_skip_spacing().ok_or(ERR_EOF)?;
        match c {
            '>' => return Ok(tag),
            '/' => {
                return if is_close_tag {
                    Err(String::from("Invalid html tag"))
                } else {
                    c = self.read_one_skip_spacing().ok_or(ERR_EOF)?;
                    if c == '>' {
                        Ok(tag)
                    } else {
                        Err(format!("Expected element closure '>' but got '{}'", c))
                    }
                }
            }
            _ => {}
        }

        // Parse args
        // loop {}

        Ok(tag)
    }
}

#[derive(Debug)]
struct Tag {
    type_: TagType,

    // name_start indicates the tag name start character index in the source
    name_start: usize,
    // name_end indicates the tag name end character index in the source
    name_end: usize,

    args: Vec<(String, String)>,
}

impl Tag {
    pub fn name<'a>(&self, parser: &'a Parser) -> &'a [char] {
        &parser.source_chars[self.name_start..self.name_end]
    }
    pub fn name_string(&self, parser: &Parser) -> String {
        self.name(parser).iter().collect()
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
