use super::Parser;
use miette::{Diagnostic, NamedSource, SourceSpan};
use std::error;
use std::fmt;

#[derive(Debug, Diagnostic)]
#[diagnostic(
    code(oops::my::bad),
    url(docsrs),
    help("try doing it better next time?")
)]
pub struct ParserError {
    pub message: String,

    #[source_code]
    pub src: NamedSource,

    #[label("This bit here")]
    pub location: SourceSpan,
}

const ERR_EOF: &'static str = "Unexpected EOF";

impl ParserError {
    pub fn new(p: &Parser, message: impl Into<String>) -> Self {
        let location = if p.current_char > 1 {
            (p.current_char - 2, p.current_char - 1)
        } else {
            (0, 1)
        };

        Self {
            message: message.into(),
            src: NamedSource::new("file.vue", p.source_chars.iter().collect::<String>()),
            location: location.into(),
        }
    }

    pub fn eof(p: &Parser) -> Self {
        Self::new(p, ERR_EOF)
    }

    pub fn is_eof(&self) -> bool {
        self.message == ERR_EOF
    }
}

impl error::Error for ParserError {}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "vue file parsing error: {}", self.message)
    }
}
