pub mod error;
pub mod tests;

use error::ParserError;

// const INPUT: &'static str = "
// <template>
//     <h1>Hello world</h1>
// </template>

// <script>
// export default {}
// </script>

// <style lang=\"stylus\" scoped>
// h1
//     color red
// </style>
// ";

// fn main() {
//     match Parser::parse(INPUT) {
//         Err(e) => panic!("{}", e.to_string()),
//         Ok(v) => println!("{:#?}", v),
//     }
// }

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
    pub content: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct Style {
    pub lang: Option<SourceLocation>,
    pub scoped: bool,
    pub content: SourceLocation,
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
                    let lang = top_level_tag.1.arg(self, "lang");

                    match top_level_tag.0 {
                        TopLevelTag::Template => {
                            if self.template.is_some() {
                                return Err(ParserError::new("execute", "can't have multiple templates in your code"));
                            }
                            let template_start = self.current_char;
                            let SourceLocation(template_end, _) = self.look_for("</template>".chars().collect())?;


                            self.template = Some(Template{
                                lang,
                                content: SourceLocation(template_start, template_end),
                            });
                        },
                        TopLevelTag::Script => {
                            if self.script.is_some() {
                                return Err(ParserError::new("execute", "can't have multiple scripts in your code"));
                            }
                            let script_start = self.current_char;

                            let default_export_location = self.parse_script_content()?;
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

    fn parse_script_content(&mut self) -> Result<Option<SourceLocation>, ParserError> {
        let mut default_export_location: Option<SourceLocation> = None;
        'outer_loop: loop {
            match self.must_read_one()? {
                // Parse JS string
                '\'' => self.parse_quotes(QuoteKind::JSSingle)?,
                '"' => self.parse_quotes(QuoteKind::JSDouble)?,
                '`' => self.parse_quotes(QuoteKind::JSBacktick)?,
                // Parse JS comment
                '/' => {
                    match self.must_read_one()? {
                        '/' => {
                            // this line is a comment
                            self.look_for(vec!['\n'])?;
                        }
                        '*' => {
                            // look for end of comment
                            self.look_for(vec!['*', '/'])?;
                        }
                        _ => {}
                    };
                    self.current_char -= 1;
                }
                // check if this is the location of the "export default"
                'e' => {
                    let default_export_start = self.current_char - 1;
                    let mut export_remaining_chars = "xport".chars();
                    while let Some(c) = export_remaining_chars.next() {
                        if self.must_read_one()? != c {
                            self.current_char -= 1;
                            continue 'outer_loop;
                        }
                    }

                    // There must be at least one space between "export" and "default"
                    if !is_space(self.must_seek_one()?) {
                        continue;
                    }

                    // Read first character ('d') of "default"
                    if self.must_read_one_skip_spacing()? != 'd' {
                        self.current_char -= 1;
                        continue;
                    };

                    let mut default_remaining_chars = "efault".chars();
                    while let Some(c) = default_remaining_chars.next() {
                        if self.must_read_one()? != c {
                            self.current_char -= 1;
                            continue 'outer_loop;
                        }
                    }

                    if !is_space(self.must_seek_one()?) {
                        continue;
                    }

                    default_export_location =
                        Some(SourceLocation(default_export_start, self.current_char));
                }
                // Check if this is the script tag end </script>
                '<' => {
                    match self.must_seek_one()? {
                        '/' | 'a'..='z' | 'A'..='Z' | '0'..='9' => {
                            match self.parse_tag() {
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
                                            "parse_script_content",
                                            format!(
                                                "expected script closure but got {}",
                                                tag.type_.to_string()
                                            ),
                                        ));
                                    }

                                    // Tag needs to be a script tag
                                    if !tag.name.eq(self, &mut "script".chars()) {
                                        return Err(ParserError::new(
                                            "parse_script_content",
                                            format!(
                                                "expected script closure but got {}",
                                                tag.name.string(self)
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
    fn try_parse_arg(&mut self, first_char: char) -> Result<Option<TagArg>, ParserError> {
        match first_char {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '@' | ':' | '_' => {}
            _ => return Ok(None),
        };

        let mut key_location = SourceLocation(self.current_char - 1, 0);

        let mut has_arg = true;
        loop {
            match self.must_read_one()? {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | ':' => {}
                '=' => {
                    let c = self.must_seek_one()?;
                    has_arg = !is_space(c) && c != '/' && c != '>';
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

        let value_location = if has_arg {
            // Parse the argument
            Some(match self.must_read_one()? {
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
                _ => {
                    let start = self.current_char - 1;
                    loop {
                        match self.must_read_one()? {
                            '>' | '/' => {
                                break;
                            }
                            c if is_space(c) => {
                                break;
                            }
                            _ => {}
                        }
                    }
                    self.current_char -= 1;
                    SourceLocation(start, self.current_char)
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
                ("once", true, |_, v| TagArg::Once(v)),
                ("model", true, |_, v| TagArg::Model(v)),
                ("cloak", true, |_, v| TagArg::Cloak(v)),
                ("else-if", true, |_, v| TagArg::ElseIf(v)),
                ("bind", true, |k, v| TagArg::Bind(k, v)),
                ("on", true, |k, v| TagArg::On(k, v)),
            ];

            let vue_directives_match_input = Vec::with_capacity(vue_directives.len());
            for e in vue_directives.iter() {
                vue_directives_match_input.push(e.0.chars());
            }

            if let Some(idx) = key_location.eq_some(self, true, vue_directives_match_input) {
                let (key, expects_argument, make_result_tag) = vue_directives[idx];

                if has_arg != expects_argument {
                    Err(ParserError::new(
                        "try_parse_arg",
                        format!(
                            "expected {} argument value for \"{}\"",
                            if expects_argument { "an" } else { "no" },
                            key_location.string(self)
                        ),
                    ))
                } else {
                    key_location.0 += key.len();
                    if self.source_chars[key_location.0] == ':' {
                        key_location.0 += 1;
                    }

                    Ok(Some(make_result_tag(
                        key_location,
                        value_location.unwrap_or(SourceLocation(0, 0)),
                    )))
                }
            } else {
                key_location.0 -= 2;
                Err(ParserError::new(
                    "try_parse_arg",
                    format!("unknown vue argument \"{}\"", key_location.string(self)),
                ))
            }
        } else {
            Ok(Some(TagArg::Default(key_location, value_location)))
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
            tag.name.0 += 1;
            self.current_char += 1;
            is_close_tag = true;
        }

        // Parse names
        loop {
            match self.must_read_one()? {
                'a'..='z' | 'A'..='Z' | '0'..='9' => {}
                _ => {
                    self.current_char -= 1;
                    tag.name.1 = self.current_char;
                    break;
                }
            };
        }

        if tag.name.1 == 0 {
            return Err(ParserError::new("parse_tag", "expected tag name"));
        }

        // Parse args
        loop {
            c = self.must_read_one_skip_spacing()?;

            match c {
                '>' => return Ok(tag),
                '/' => {
                    return if is_close_tag {
                        Err(ParserError::new("parse_tag", "Invalid html tag"))
                    } else {
                        c = self.must_read_one_skip_spacing()?;
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
                Some(match self.must_read_one()? {
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
                        self.current_char -= 1;
                        SourceLocation(start, self.current_char)
                    }
                })
            };

            // tag.args.push(TagArg {
            //     key: key_location,
            //     value: value_location,
            // });
        }
    }

    fn parse_quotes(&mut self, kind: QuoteKind) -> Result<(), ParserError> {
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
                    todo!("JS backtick string inner code");
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

#[derive(Debug)]
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
}

#[derive(Debug)]
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
    Once(SourceLocation),                            // v-once=""
}

impl TagArg {
    fn key_eq(&self, parser: &Parser, key: &str) -> bool {
        match self {
            Self::Default(key_location, _) => key_location.eq(parser, key.chars()), // value="val"
            Self::Bind(key_location, _) => {
                if key.starts_with(':') {
                    key_location.eq(parser, key.chars().skip(1))
                } else if key.starts_with("v-bind:") {
                    key_location.eq(parser, key.chars().skip(7))
                } else {
                    key_location.eq(parser, key.chars())
                }
            } // :value="val" and v-bind:value="val"
            Self::On(key_location, _) => key == "", // @click and v-on:click="val"
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
            Self::Once(_) => key == "v-once",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceLocation(pub usize, pub usize);

impl SourceLocation {
    pub fn chars<'a>(&self, parser: &'a Parser) -> &'a [char] {
        &parser.source_chars[self.0..self.1]
    }
    pub fn chars_vec<'a>(&self, parser: &'a Parser) -> Vec<char> {
        parser.source_chars[self.0..self.1].into()
    }
    pub fn string(&self, parser: &Parser) -> String {
        self.chars(parser).iter().collect()
    }
    pub fn len(&self) -> usize {
        self.1 - self.0
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
        let disabled_entries = 0;

        let mut self_iter = self.chars(parser).iter();
        loop {
            if let Some(a) = self_iter.next() {
                for (idx, result) in results.iter().enumerate() {
                    if let Some(iter) = result {
                        if let Some(b) = iter.next() {
                            if *a == b {
                                continue;
                            }
                        } else if can_start_with {
                            return Some(idx);
                        }
                        *results.get_mut(idx).unwrap() = None;

                        disabled_entries += 1;
                        if disabled_entries == others.len() {
                            return None;
                        }
                    }
                }
            } else {
                for (idx, result) in results.iter().enumerate() {
                    if let Some(iter) = result {
                        if iter.next().is_none() {
                            return Some(idx);
                        }
                    }
                }
                return None;
            }
        }
    }
    pub fn starts_with(&self, parser: &Parser, other: impl Iterator<Item = char>) -> bool {
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

#[derive(Debug)]
enum TagType {
    Open,
    OpenAndClose,
    Close,
}

impl TagType {
    fn to_string(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::OpenAndClose => "inline",
            Self::Close => "close",
        }
    }
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
