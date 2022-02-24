use super::{utils, Parser, ParserError};

/*

TODO: Support :NOT(.foo,.bar)

TODO: Support vue's deep selectors
">>>", "::v-deep", "/deep/"
https://vue-loader.vuejs.org/guide/scoped-css.html#child-component-root-elements

*/

pub fn parse_scoped_css(p: &mut Parser) -> Result<Vec<usize>, ParserError> {
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

    loop {
        p.must_read_one_skip_spacing()?;
        p.current_char -= 1;

        match parse_selector(p, &mut basic_selector_ends)? {
            ParseSelectorDoNext::Content => parse_selector_content(p)?,
            ParseSelectorDoNext::StyleClose => return Ok(basic_selector_ends),
        }
    }
}

fn parse_selector_content(p: &mut Parser) -> Result<(), ParserError> {
    loop {
        match p.must_read_one()? {
            '}' => return Ok(()),
            '/' if p.seek_one_or_null() == '*' => {
                // This is the start of a comment
                parse_comment(p)?;
            }
            _ => {}
        }
    }
}

enum ParseSelectorDoNext {
    Content,
    StyleClose,
}

fn parse_selector(
    p: &mut Parser,
    basic_selector_ends: &mut Vec<usize>,
) -> Result<ParseSelectorDoNext, ParserError> {
    // the top level loop loops over the selector components:
    // foo  bar
    // ^^^  ^^^ - these are 2 components
    loop {
        let mut handle_pseudo_classes_next = false;

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
                    basic_selector_ends.push(p.current_char - 1);
                    handle_pseudo_classes_next = true;
                    break;
                }
                '<' if is_style_close_tag(p) => {
                    // this is the style closing tag
                    return Ok(ParseSelectorDoNext::StyleClose);
                }
                '{' => {
                    // This is a tag opener
                    basic_selector_ends.push(p.current_char - 1);
                    return Ok(ParseSelectorDoNext::Content);
                }
                c if is_combinator(c) => {
                    parse_combinator(p)?;
                    break;
                }
                _ => {}
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
                    '<' if is_style_close_tag(p) => {
                        // this is the style closing tag
                        return Ok(ParseSelectorDoNext::StyleClose);
                    }
                    '{' => {
                        // This is a tag opener
                        return Ok(ParseSelectorDoNext::Content);
                    }
                    c if is_combinator(c) => {
                        parse_combinator(p)?;
                        break;
                    }
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
        '*' | '>' | '+' | '~' => true,
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
