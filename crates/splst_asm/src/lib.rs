mod lex;
mod parse;
mod gen;
mod ir;

use std::fmt;

#[derive(thiserror::Error, Debug)]
pub struct Error {
    line: usize,
    message: String,
}

impl Error {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self { line, message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[error]:{}: {}", self.line, self.message)
    }
}

pub fn assemble<'a>(input: &'a str, base: u32) -> Result<Vec<u8>, Error> {
    let (text, data) = parse::parse(input)?;
    gen::gen_machine_code(text, data, base)
}
