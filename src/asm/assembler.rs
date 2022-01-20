use std::collections::HashMap;
use std::fmt;

#[derive(thiserror::Error, Debug)]
struct Error {
    line: usize,
    message: String,
}

impl Error {
    fn new<T: Into<String>>(line: usize, message: T) -> Self {
        Self {
            line,
            message: message.into()
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line: {} - {}", self.line, self.message)
    }
}

enum Section {
    Data,
    Text,
}

enum ParseState<'a> {
    Blank,
    Label(&'a str),
    Instruction(u8),
}

pub struct Assembler {
    section: Option<Section>,
    labels: HashMap<String, u32>,
}

impl Assembler {
    pub fn new() -> Self {
        Self {
            section: None,
            labels: HashMap::with_capacity(16),
        }
    }

    pub fn assemble(&mut self, source: &str) -> Result<Vec<u8>, Error> {
        let mut state = ParseState::Blank;
        for (linenr, line) in source.lines().enumerate() {
            for word in line.split_whitespace() {
                match state {
                    ParseState::Blank if word.starts_with('#') => break,
                    ParseState::Blank if word.starts_with('.') => match word {
                        ".text" => self.section = Some(Section::Text),
                        ".data" => self.section = Some(Section::Data),
                        _ => return Err(Error::new(linenr, format!("Invalid section: {}", word))),
                    }
                    ParseState::Blank if word.ends_with(':') => {
                        state = ParseState::Label(word.strip_suffix(':').unwrap());
                    }
                    ParseState::Blank => state = ParseState::Instruction(match word {
                        "j" => 0x2,
                        "jal" => 0x3,
                        "beq" => 0x4,
                        "bne" => 0x5,
                        "blez" => 0x6,
                        "bgtz" => 0x7,
                        _ => 0x8,
                    }),
                    ParseState::Instruction(..) => {
                    }
                    ParseState::Label(..) => {
                    }
                }
            }
        }
        Ok(vec![])
    }
}
