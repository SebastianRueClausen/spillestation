mod lex;
mod parse;
mod gen;

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

#[derive(Debug, PartialEq, Eq)]
pub enum Section {
    Text,
    Data,
}

#[allow(dead_code)]
pub fn assemble<'a>(input: &'a str, base: u32) -> Result<Vec<u8>, Error> {
    gen::gen_machine_code(parse::parse(input)?, base)
}
