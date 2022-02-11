//! A small Mips assmebler. Written mainly to be used for convenient testing and debugging.
//!
//! todo:
//! * Support for multiple files and global scoped labels.
//!
//! * More pseudo instructions / directives such as 'align'.
//!
//! * Macros.
//!
//! * Add an 'API' with functions for common operations on the system such as DMA transfers or GPU
//!   commands.
//!
//! * Mfc0 / Mtc0 should take register arguments instead of immediate values for the second
//!   argument. Problem is that it should only take numbered register arguments.
//!
//! * Check for overflow in immediate values.

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
        write!(f, "{}: {}", self.line, self.message)
    }
}

/// Assemble the input string. 'base' is the address of the first instruction in the text segment.
pub fn assemble<'a>(input: &'a str, base: u32) -> Result<Vec<u8>, Error> {
    let (text, data) = parse::parse(input)?;
    gen::gen_machine_code(text, data, base)
}
