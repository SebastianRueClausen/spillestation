use super::{Section, Error};
use crate::cpu::{REGISTER_NAMES, RegIdx};

use std::str::Chars;

#[derive(PartialEq, Eq, Debug)]
pub enum TokTy<'a> {
    Section(Section),
    Label(&'a str),
    Id(&'a str),
    Num(u32),
    Reg(RegIdx), 
    Comma,
    Eof,
}

pub struct Tok<'a> {
    pub ty: TokTy<'a>,
    pub line: usize,
}

impl<'a> Tok<'a> {
    fn new(line: usize, ty: TokTy<'a>) -> Self {
        Self { line, ty }
    }
}

struct Lexer<'a> {
   chars: Chars<'a>,
   line: usize,
}

fn is_id_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_id_con(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t')
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Lexer<'a> {
        Self {
            chars: input.chars(),
            line: 1,
        }
    }

    fn err(&self, msg: impl Into<String>) -> Error {
        Error::new(self.line, msg)
    }

    fn first(&self) -> char {
        self.chars.clone().next().unwrap_or('\0')
    }

    fn second(&self) -> char {
        let mut clone = self.chars.clone();
        clone.next();
        clone.next().unwrap_or('\0')
    }

    fn is_done(&mut self) -> bool {
        self.chars.as_str().is_empty()
    }

    fn eat(&mut self) -> Option<char> {
        self.chars.next()
    }

    fn eat_char(&mut self, c: char) -> bool {
        if self.first() == c {
            self.eat();
            true
        } else {
            false
        }
    }
    
    fn eat_while(&mut self, mut pred: impl FnMut(char) -> bool) -> usize {
        let mut eaten = 0;
        while pred(self.first()) && !self.is_done() {
            self.eat();
            eaten += 1;
        }
        eaten
    }

    fn eat_whitespace(&mut self) {
        loop {
            self.eat_while(is_whitespace);
            match self.first() {
                '#' => {
                    self.eat_while(|c| c != '\n');
                    self.line += 1;
                    self.eat();
                }
                '\n' => {
                    self.line += 1;
                    self.eat();
                }
                _ => break,
            }
        }
    }

    fn eat_id(&mut self) -> &'a str {
        let as_str = self.chars.as_str();
        let eaten = if is_id_start(self.first()) {
            self.eat();
            1
        } else {
            0
        };
        let eaten = eaten + self.eat_while(is_id_con);
        &as_str[..eaten]
    }

    fn eat_num(&mut self) -> Result<u32, Error> {
        debug_assert!(self.first().is_ascii_digit());
        let base = if self.first() == '0' {
            if self.second() == 'x' {
                self.eat();
                self.eat();
                16
            } else if self.second() == 'b' {
                self.eat();
                self.eat();
                2
            } else {
                10
            }
        } else {
            10
        };
        let as_str = self.chars.as_str();
        let eaten = self.eat_while(|c| {
            c.is_ascii_digit()
        });
        u32::from_str_radix(&as_str[0..eaten], base).map_err(|err| {
            self.err(&format!("Invalid number: {}", err))
        })
    }

    fn tok(&self, ty: TokTy<'a>) -> Tok<'a> {
        Tok::new(self.line, ty)  
    }

    fn next_tok(&mut self) -> Result<Tok<'a>, Error> {
        self.eat_whitespace();
        match self.first() {
            c if is_id_start(c) => {
                let id = self.eat_id();
                if self.eat_char(':') {
                    Ok(self.tok(TokTy::Label(id)))
                } else {
                    Ok(self.tok(TokTy::Id(id)))
                }
            }
            c if c.is_ascii_digit() => self.eat_num().map(|num| {
                self.tok(TokTy::Num(num))
            }),
            '-' => {
                self.eat();
                self.eat_whitespace();
                if !self.first().is_ascii_digit() {
                    Err(self.err(&format!("Expected number after '-'")))
                } else {
                    self.eat_num().map(|num| {
                        let num = -(num as i32);
                        self.tok(TokTy::Num(num as u32))
                    })
                }
            }
            '.' => {
                self.eat();
                let section = match self.eat_id() {
                    "text" => Section::Text,
                    "data" => Section::Data,
                    id if id.is_empty() => {
                        return Err(self.err("Expected section after '.'"))
                    }
                    id => {
                        return Err(self.err(&format!("Invalid section .{}", id)))
                    }
                };
                Ok(self.tok(TokTy::Section(section)))
            }
            ',' => {
                self.eat();
                Ok(self.tok(TokTy::Comma))
            }
            '$' => {
                self.eat();
                let reg = if self.first().is_ascii_digit() {
                    let num = self.eat_num()?;
                    if !(0..32).contains(&num) {
                        return Err(self.err(
                            &format!("Invalid register '${}'", num)
                        ));
                    }
                    RegIdx::new(num)
                } else {
                    let id = self.eat_id();
                    let reg = REGISTER_NAMES.iter()
                        .position(|k| *k == id)
                        .ok_or_else(|| {
                            self.err(&format!("Invalid register '${}'", id))
                        })?;
                    RegIdx::new(reg as u32)
                };
                Ok(self.tok(TokTy::Reg(reg)))
            }
            '\0' => Ok(self.tok(TokTy::Eof)),
            c => {
                Err(self.err(&format!("Invalid token '{}'", c)))
            }
        }
    }
}

pub fn tokenize(input: &str) -> impl Iterator<Item = Result<Tok, Error>> + '_ {
    let mut lexer = Lexer::new(input);
    std::iter::from_fn(move || {
        match lexer.next_tok() {
            Ok(t) if t.ty == TokTy::Eof => None,
            t => Some(t),
        }
    })
}

#[test]
fn comment() {
    let input = r#"
        # Comment Comment Comment.
        add $t2, $t0, $t1 # Comment
        # Comment Comment Comment.
    "#;
    let expected = [
        TokTy::Id("add"),
        TokTy::Reg(RegIdx::new(10)),
        TokTy::Comma,
        TokTy::Reg(RegIdx::new(8)),
        TokTy::Comma,
        TokTy::Reg(RegIdx::new(9)),
    ];
    let res: Vec<TokTy> = tokenize(input).map(|t| t.unwrap().ty).collect();
    for (got, exp) in res.iter().zip(expected) {
        assert_eq!(*got, exp);
    }
}

#[test]
fn number() {
    let input = r#"
        42
        -0x42
        0b0101
    "#;
    let expected = [
        TokTy::Num(42),
        TokTy::Num(-0x42_i32 as u32),
        TokTy::Num(0b0101),
    ];
    let res: Vec<TokTy> = tokenize(input).map(|t| t.unwrap().ty).collect();
    for (got, exp) in res.iter().zip(expected) {
        assert_eq!(*got, exp);
    }
}

#[test]
fn general() {
    let input = r#"
        .text
        main:
            li $v0, 4
            la $a0, string1
            syscall

            li $v0, 5
            syscall
            
            move $t0, $v0
            
            li $v0, 4
            la $a0, endLine
            syscall
            
            li $v0, 4
            la $a0, string2
            syscall
            
            li $v0, 5
            syscall
            
            move $t1, $v0
            
            li $v0, 4
            la $a0, string3
            syscall
            
            add $t2, $t1, $t0
            li $v0, 1
            move $a0, $t2
            syscall
                 
            li $v0, 10
            syscall
    "#;
    let expected = [
        TokTy::Section(Section::Text),
        TokTy::Label("main"),

        TokTy::Id("li"),
        TokTy::Reg(RegIdx::V0),
        TokTy::Comma,
        TokTy::Num(4),

        TokTy::Id("la"),
        TokTy::Reg(RegIdx::A0),
        TokTy::Comma,
        TokTy::Id("string1"),

        TokTy::Id("syscall"),

        TokTy::Id("li"),
        TokTy::Reg(RegIdx::V0),
        TokTy::Comma,
        TokTy::Num(5),
        
        TokTy::Id("syscall"),

        TokTy::Id("move"),
        TokTy::Reg(RegIdx::T0),
        TokTy::Comma,
        TokTy::Reg(RegIdx::V0),

        TokTy::Id("li"),
        TokTy::Reg(RegIdx::V0),
        TokTy::Comma,
        TokTy::Num(4),

        TokTy::Id("la"),
        TokTy::Reg(RegIdx::A0),
        TokTy::Comma,
        TokTy::Id("endLine"),

        TokTy::Id("syscall"),

        TokTy::Id("li"),
        TokTy::Reg(RegIdx::V0),
        TokTy::Comma,
        TokTy::Num(4),

        TokTy::Id("la"),
        TokTy::Reg(RegIdx::A0),
        TokTy::Comma,
        TokTy::Id("string2"),

        TokTy::Id("syscall"),

        TokTy::Id("li"),
        TokTy::Reg(RegIdx::V0),
        TokTy::Comma,
        TokTy::Num(5),

        TokTy::Id("syscall"),

        TokTy::Id("move"),
        TokTy::Reg(RegIdx::T1),
        TokTy::Comma,
        TokTy::Reg(RegIdx::V0),

        TokTy::Id("li"),
        TokTy::Reg(RegIdx::V0),
        TokTy::Comma,
        TokTy::Num(4),

        TokTy::Id("la"),
        TokTy::Reg(RegIdx::A0),
        TokTy::Comma,
        TokTy::Id("string3"),

        TokTy::Id("syscall"),

        TokTy::Id("add"),
        TokTy::Reg(RegIdx::T2),
        TokTy::Comma,
        TokTy::Reg(RegIdx::T1),
        TokTy::Comma,
        TokTy::Reg(RegIdx::T0),

        TokTy::Id("li"),
        TokTy::Reg(RegIdx::V0),
        TokTy::Comma,
        TokTy::Num(1),

        TokTy::Id("move"),
        TokTy::Reg(RegIdx::A0),
        TokTy::Comma,
        TokTy::Reg(RegIdx::T2),

        TokTy::Id("syscall"),

        TokTy::Id("li"),
        TokTy::Reg(RegIdx::V0),
        TokTy::Comma,
        TokTy::Num(10),

        TokTy::Id("syscall"),
    ];
    let res: Vec<TokTy> = tokenize(input).map(|t| t.unwrap().ty).collect();
    for (got, exp) in res.iter().zip(expected) {
        assert_eq!(*got, exp);
    }
}
