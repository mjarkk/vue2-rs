use super::{Parser, ParserError};

pub fn compile_tempalte(p: &mut Parser) -> Result<(), ParserError> {
    let c = p.must_read_one_skip_spacing()?;
    if c == '<' {
        p.parse_tag()?;
    } else {
        // name like charcters
    }

    Ok(())
}
