const input: &'static str = "
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
    if let Err(e) = Parser::parse(input) {
        panic!("{}", e.to_string());
    }
}

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
    fn read_byte(&mut self) -> Option<char> {
        if self.source_chars_len == self.current_char {
            return None;
        }
        let resp = self.source_chars[self.current_char];
        self.current_char += 1;
        return Some(resp);
    }
    fn execute(&mut self) -> Result<(), String> {
        while let Some(b) = self.read_byte() {
            match b {
                ' ' | '\t' | '\n' | '\r' => {},
                '<' => {self.parseTopLevelTag()?;},
                c => return Err(format!("found invalid character in source: '{}', expected <template ..> <script ..> or <style ..>", c.to_string() )),
            };
        }
        Ok(())
    }
    fn parseTopLevelTag(&mut self) -> Result<TopLevelTag, String> {
        // TODO
        Ok(TopLevelTag::Script)
    }
}

enum TopLevelTag {
    Template,
    Script,
    Style,
}
