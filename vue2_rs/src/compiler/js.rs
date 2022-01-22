use super::{utils::is_space, Parser, ParserError, QuoteKind, SourceLocation, TagType};

pub fn compile_template_var(p: &mut Parser) -> Result<(), ParserError> {
    loop {
        match p.must_read_one()? {
            c if handle_common(p, c)? => {}

            '}' if p.must_seek_one()? == '}' => {
                p.current_char += 1;
                return Ok(());
            }
            _ => {}
        }
    }
}

pub fn compile_script_content(p: &mut Parser) -> Result<Option<SourceLocation>, ParserError> {
    let mut default_export_location: Option<SourceLocation> = None;
    'outer_loop: loop {
        match p.must_read_one()? {
            c if handle_common(p, c)? => {}

            // check if this is the location of the "export default"
            'e' => {
                let default_export_start = p.current_char - 1;
                let mut export_remaining_chars = "xport".chars();
                while let Some(c) = export_remaining_chars.next() {
                    if p.must_read_one()? != c {
                        p.current_char -= 1;
                        continue 'outer_loop;
                    }
                }

                // There must be at least one space between "export" and "default"
                if !is_space(p.must_seek_one()?) {
                    continue;
                }

                // Read first character ('d') of "default"
                if p.must_read_one_skip_spacing()? != 'd' {
                    p.current_char -= 1;
                    continue;
                };

                let mut default_remaining_chars = "efault".chars();
                while let Some(c) = default_remaining_chars.next() {
                    if p.must_read_one()? != c {
                        p.current_char -= 1;
                        continue 'outer_loop;
                    }
                }

                if !is_space(p.must_seek_one()?) {
                    continue;
                }

                default_export_location =
                    Some(SourceLocation(default_export_start, p.current_char));
            }

            // Check if this is the script tag end </script>
            '<' => {
                match p.must_seek_one()? {
                    '/' | 'a'..='z' | 'A'..='Z' | '0'..='9' => {
                        match p.parse_tag() {
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
                                if !tag.name.eq(p, &mut "script".chars()) {
                                    return Err(ParserError::new(
                                        "parse_script_content",
                                        format!(
                                            "expected script closure but got {}",
                                            tag.name.string(p)
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

fn parse_comment(p: &mut Parser) -> Result<bool, ParserError> {
    match p.must_seek_one()? {
        '/' => {
            // this line is a comment
            p.current_char += 1;
            p.look_for(vec!['\n'])?;
            p.current_char -= 1;
            Ok(true)
        }
        '*' => {
            // look for end of comment
            p.current_char += 1;
            p.look_for(vec!['*', '/'])?;
            p.current_char -= 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub fn parse_block_like(p: &mut Parser, closure: char) -> Result<(), ParserError> {
    loop {
        match p.must_read_one()? {
            c if handle_common(p, c)? => {}
            // Is closing character
            c if c == closure => return Ok(()),
            _ => {}
        }
    }
}

pub fn handle_common(p: &mut Parser, c: char) -> Result<bool, ParserError> {
    match c {
        // Parse string
        '\'' => {
            p.parse_quotes(QuoteKind::JSSingle)?;
            Ok(true)
        }
        '"' => {
            p.parse_quotes(QuoteKind::JSDouble)?;
            Ok(true)
        }
        '`' => {
            p.parse_quotes(QuoteKind::JSBacktick)?;
            Ok(true)
        }
        // Parse comment
        '/' if parse_comment(p)? => Ok(true),
        // Parse block like
        '{' => {
            parse_block_like(p, '}')?;
            Ok(true)
        }
        '(' => {
            parse_block_like(p, ')')?;
            Ok(true)
        }
        '[' => {
            parse_block_like(p, ']')?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
