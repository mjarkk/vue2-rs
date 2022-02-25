use super::{utils, Parser, ParserError, QuoteKind, SourceLocation};

/*

TODO: Support :NOT(.foo,.bar)

TODO: Support vue's deep selectors
">>>", "::v-deep", "/deep/"
https://vue-loader.vuejs.org/guide/scoped-css.html#child-component-root-elements

*/

pub fn gen_scoped_css(
    p: &mut Parser,
    style_location: &SourceLocation,
    injection_points: Vec<usize>,
    id: &str,
) -> String {
    let mut resp = String::new();

    let mut last = style_location.0;
    for point in injection_points {
        SourceLocation(last, point).write_to_string(p, &mut resp);
        last = point;
        resp.push_str("[data-v-");
        resp.push_str(id);
        resp.push(']');
    }
    SourceLocation(last, style_location.1).write_to_string(p, &mut resp);

    resp
}

#[derive(PartialEq)]
pub enum SelectorsEnd {
    StyleClosure,
    EOF,
    ClosingBracket,
}

impl SelectorsEnd {
    fn matches(&self, p: &mut Parser, c: char) -> bool {
        match c {
            '<' => match self {
                Self::StyleClosure => is_style_close_tag(p),
                _ => false,
            },
            '}' => match self {
                Self::ClosingBracket => true,
                _ => false,
            },
            _ => false,
        }
    }
}

pub fn parse_scoped_css(p: &mut Parser, end: SelectorsEnd) -> Result<Vec<usize>, ParserError> {
    /*
    basic_selector_ends contains all the css selector location ends before any pseudo-classes
    This is for example:

    foo {}
       ^
    .foo  bar {}
        ^    ^
    .foo, .bar {}
        ^    ^
    .foo > .bar {}
        ^     ^
    foo:hover {}
       ^
    */

    let mut basic_selector_ends: Vec<usize> = Vec::new();
    let parsing_result = parse_selectors(p, &mut basic_selector_ends, &end);
    if let Err(e) = parsing_result {
        if SelectorsEnd::EOF != end || !e.is_eof() {
            return Err(e);
        }
    }
    Ok(basic_selector_ends)
}

pub fn parse_selectors(
    p: &mut Parser,
    basic_selector_ends: &mut Vec<usize>,
    end: &SelectorsEnd,
) -> Result<(), ParserError> {
    loop {
        let c = p.must_read_one_skip_spacing()?;
        match c {
            '@' => {
                match parse_at(p, basic_selector_ends, end)? {
                    ParseSelectorDoNext::Content => {}
                    ParseSelectorDoNext::Closure => break,
                };
            }
            c if end.matches(p, c) => break,
            _ => {
                p.current_char -= 1;

                match parse_selector(p, basic_selector_ends, end)? {
                    ParseSelectorDoNext::Content => parse_selector_content(p)?,
                    ParseSelectorDoNext::Closure => break,
                }
            }
        }
    }
    Ok(())
}

enum ParseSelectorDoNext {
    Content,
    Closure,
}

/* parse:
    @media {..}
    @namespace ..;
    @keyframes {..}
    @charset ..;
    @import ..;
    @supports {..}
    @layer {..}
*/
fn parse_at(
    p: &mut Parser,
    basic_selector_ends: &mut Vec<usize>,
    end: &SelectorsEnd,
) -> Result<ParseSelectorDoNext, ParserError> {
    let mut parse_args_next = false;
    let mut arg_open = false;
    let mut arg_string = false;

    let name_start = p.current_char;
    loop {
        match p.must_read_one()? {
            '{' => {
                // open args
                break;
            }
            '(' => {
                // parse args now
                parse_args_next = true;
                arg_open = true;
                break;
            }
            '\'' | '"' => {
                parse_args_next = true;
                arg_string = true;
                break;
            }
            ';' => {
                // end
                return Ok(ParseSelectorDoNext::Content);
            }
            c if utils::is_space(c) => {
                // parse args now
                parse_args_next = true;
                break;
            }
            '/' if p.seek_one_or_null() == '*' => {
                // This is the start of a comment
                parse_comment(p)?;
            }
            c if end.matches(p, c) => return Ok(ParseSelectorDoNext::Closure),
            _ => {}
        };
    }
    let name_location = SourceLocation(name_start, p.current_char - 1);

    if parse_args_next {
        if arg_open {
            parse_arg(p)?;
        } else if arg_string {
            p.current_char -= 1;
        }

        loop {
            match p.must_read_one()? {
                '\'' => p.parse_quotes(QuoteKind::JSSingle, &mut None)?,
                '"' => p.parse_quotes(QuoteKind::JSDouble, &mut None)?,
                '{' => break,
                '(' => parse_arg(p)?,
                ';' => {
                    // end
                    return Ok(ParseSelectorDoNext::Content);
                }
                '/' if p.seek_one_or_null() == '*' => {
                    // This is the start of a comment
                    parse_comment(p)?;
                }
                c if end.matches(p, c) => return Ok(ParseSelectorDoNext::Closure),
                _ => {}
            };
        }
    }

    let name_first_char = p.source_chars.get(name_location.0).unwrap();
    let is_keyframes = if *name_first_char == '-' {
        // Is vendor prefix
        name_location
            .eq_some(
                p,
                false,
                vec!["-webkit-keyframes".chars(), "-moz-keyframes".chars()],
            )
            .is_some()
    } else {
        name_location.eq(p, "keyframes".chars())
    };

    if is_keyframes {
        parse_selector_content(p)?;
    } else {
        parse_selectors(p, basic_selector_ends, &SelectorsEnd::ClosingBracket)?;
    }

    Ok(ParseSelectorDoNext::Content)
}

fn parse_arg(p: &mut Parser) -> Result<(), ParserError> {
    loop {
        match p.must_read_one()? {
            '\'' => p.parse_quotes(QuoteKind::JSSingle, &mut None)?,
            '"' => p.parse_quotes(QuoteKind::JSDouble, &mut None)?,
            ')' => return Ok(()),
            '/' if p.seek_one_or_null() == '*' => {
                // This is the start of a comment
                parse_comment(p)?;
            }
            _ => {}
        }
    }
}

fn parse_selector_content(p: &mut Parser) -> Result<(), ParserError> {
    loop {
        match p.must_read_one()? {
            '\'' => p.parse_quotes(QuoteKind::JSSingle, &mut None)?,
            '"' => p.parse_quotes(QuoteKind::JSDouble, &mut None)?,
            '}' => return Ok(()),
            '{' => parse_selector_content(p)?,
            '/' if p.seek_one_or_null() == '*' => {
                // This is the start of a comment
                parse_comment(p)?;
            }
            _ => {}
        }
    }
}

fn parse_selector(
    p: &mut Parser,
    basic_selector_ends: &mut Vec<usize>,
    end: &SelectorsEnd,
) -> Result<ParseSelectorDoNext, ParserError> {
    // the top level loop loops over the selector components:
    // foo  bar
    // ^^^  ^^^ - these are 2 components
    loop {
        let mut handle_pseudo_classes_next = false;

        let mut has_any_chars = false;
        loop {
            match p.must_read_one()? {
                '/' if p.seek_one_or_null() == '*' => {
                    // This is the start of a comment
                    parse_comment(p)?;
                }
                '[' => {
                    // This is the start of a attribute selector
                    parse_attribute_selector(p)?;
                }
                ':' => {
                    // This is the start of a pseudo-classes selector
                    if has_any_chars {
                        basic_selector_ends.push(p.current_char - 1);
                    }
                    handle_pseudo_classes_next = true;
                    break;
                }
                '{' => {
                    // This is a tag opener
                    if has_any_chars {
                        basic_selector_ends.push(p.current_char - 1);
                    }
                    return Ok(ParseSelectorDoNext::Content);
                }
                c if is_combinator(c) => {
                    if has_any_chars {
                        basic_selector_ends.push(p.current_char - 1);
                    }
                    parse_combinator(p)?;
                    break;
                }
                c if end.matches(p, c) => return Ok(ParseSelectorDoNext::Closure),
                _ => {
                    has_any_chars = true;
                }
            };
        }

        if handle_pseudo_classes_next {
            // Handles the :hover, :focus, etc..
            loop {
                match p.must_read_one()? {
                    '/' if p.seek_one_or_null() == '*' => {
                        // This is the start of a comment
                        parse_comment(p)?;
                    }
                    '{' => {
                        // This is a tag opener
                        return Ok(ParseSelectorDoNext::Content);
                    }
                    c if is_combinator(c) => {
                        parse_combinator(p)?;
                        break;
                    }
                    c if end.matches(p, c) => return Ok(ParseSelectorDoNext::Closure),
                    _ => {}
                }
            }
        }
    }
}

fn is_style_close_tag(p: &mut Parser) -> bool {
    let start = p.current_char;
    for closure_char in "/style>".chars() {
        match p.read_one() {
            Some(c) if c == closure_char => {}
            _ => {
                p.current_char = start;
                return false;
            }
        }
    }
    true
}

fn is_combinator(c: char) -> bool {
    match c {
        c if utils::is_space(c) => true,
        ',' | '*' | '>' | '+' | '~' => true,
        _ => false,
    }
}

// https://www.w3.org/TR/selectors-3/#combinators
fn parse_combinator(p: &mut Parser) -> Result<(), ParserError> {
    loop {
        if !is_combinator(p.must_seek_one()?) {
            return Ok(());
        }
        p.current_char += 1;
    }
}

// parses: [foo] of .foo.bar[foo]
// expects to the character number to be after the '['
// https://www.w3.org/TR/selectors-3/#attribute-selectors
fn parse_attribute_selector(p: &mut Parser) -> Result<(), ParserError> {
    loop {
        match p.must_read_one()? {
            '/' if p.must_seek_one().unwrap_or(0 as char) == '*' => {
                // This is the start of a comment
                parse_comment(p)?;
            }
            ']' => return Ok(()),
            _ => {}
        }
    }
}

// parses a css comment: /* this is the comment content */
fn parse_comment(p: &mut Parser) -> Result<(), ParserError> {
    loop {
        if p.must_read_one()? == '*' {
            if p.must_seek_one()? == '/' {
                p.current_char += 1;
                return Ok(());
            }
        }
    }
}
