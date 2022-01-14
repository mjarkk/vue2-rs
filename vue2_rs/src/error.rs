use std::error;
use std::fmt;

#[derive(Debug)]
pub struct ParserError {
    method: &'static str,
    message: String,
}

const ERR_EOF: &'static str = "Unexpected EOF";

impl ParserError {
    pub fn new(method: &'static str, message: impl Into<String>) -> Self {
        Self {
            method,
            message: message.into(),
        }
    }
    pub fn eof(method: &'static str) -> Self {
        Self::new(method, ERR_EOF)
    }
}

impl error::Error for ParserError {}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} (method {})", self.message, self.method);
        Ok(())
    }
}
