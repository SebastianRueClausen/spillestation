use crate::Error;
use crate::ir::{Directive, Register};

use std::str::Chars;

/// The type of token and the data accosiated with it. 
#[derive(Debug, PartialEq, Eq)]
pub enum TokTy<'a> {
    /// Directives used to indicate data and sections. '.' followed by a keyword. fx '.ascii'.
    Directive(Directive),
    /// Identifier followed by a ':'.
    Label(&'a str),
    /// Identifier. Either an instruction or a reference to a label.
    Id(&'a str),
    /// Integer literal.
    Num(u32),
    /// String literal.
    Str(String),
    /// A register fx '$t0' or '$12'.
    Reg(Register), 
    Comma,
    LParan,
    RParan,
    Eof,
}

pub struct Tok<'a> {
    pub ty: TokTy<'a>,
    /// The line containing the token.
    pub line: usize,
}

impl<'a> Tok<'a> {
    fn new(line: usize, ty: TokTy<'a>) -> Self {
        Self { line, ty }
    }
}

#[derive(Clone)]
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

    /// Peak one character ahead.
    fn first(&self) -> char {
        self.chars.clone().next().unwrap_or('\0')
    }

    /// Peak two characters ahead.
    fn second(&self) -> char {
        let mut clone = self.chars.clone();
        clone.next();
        clone.next().unwrap_or('\0')
    }

    /// If the whole input has been consumed.
    fn is_done(&mut self) -> bool {
        self.chars.as_str().is_empty()
    }

    /// Consume as single character.
    fn eat(&mut self) -> Option<char> {
        self.chars.next()
    }

    fn eat_n(&mut self, n: usize) -> Option<char> {
        self.chars.nth(n - 1)
    }

    /// Consume a single character if it matches 'c'.
    fn eat_char(&mut self, c: char) -> bool {
        if self.first() == c {
            self.eat();
            true
        } else {
            false
        }
    }
   
    /// Consume characters until pred doesn't return true. Returns the amount of characters
    /// consumed.
    fn eat_while(&mut self, mut pred: impl FnMut(char) -> bool) -> usize {
        let mut eaten = 0;
        while pred(self.first()) && !self.is_done() {
            self.eat();
            eaten += 1;
        }
        eaten
    }

    /// Consume whitespace and comments.
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

    /// Consume an identifier. Returns a slice of the identifier.
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

    /// Consume and parse a number. Doesn't handle unary '-' and expects 'first' to be valid digit.
    fn eat_num(&mut self) -> Result<u32, Error> {
        debug_assert!(self.first().is_ascii_digit());
        let (base, eat_while): (u32, fn(char) -> bool) = if self.first() == '0' {
            if self.second() == 'x' {
                self.eat_n(2);
                (16, |c| c.is_ascii_hexdigit())
            } else if self.second() == 'b' {
                self.eat_n(2);
                (2, |c| matches!(c, '0' | '1'))
            } else {
                (10, |c| c.is_ascii_digit())
            }
        } else {
            (10, |c| c.is_ascii_digit())
        };
        let as_str = self.chars.as_str();
        let eaten = self.eat_while(eat_while);
        u32::from_str_radix(&as_str[0..eaten], base).map_err(|err| {
            self.err(&format!("Invalid number: {}", err))
        })
    }

    fn tok(&self, ty: TokTy<'a>) -> Tok<'a> {
        Tok::new(self.line, ty)  
    }

    /// Scan the next token. Returns 'TokTy::Eof' if the whole input has been consumed.
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
            '"' => {
                self.eat(); 
                let mut string = String::new();
                while !self.eat_char('"') {
                    let c = match self.first() {
                        '\\' => {
                            self.eat();
                            match self.first() {
                                't' => '\t',
                                'r' => '\r',
                                'n' => '\n',
                                '0' => '\0',
                                '\\' => '\\',
                                c => {
                                    return Err(self.err(
                                        &format!("Invalid escape sequence '\\{c}'")
                                    ));
                                }
                            }
                        }
                        '\n' => {
                            return Err(self.err(
                                &format!("Newline in string literal")
                            ));
                        }
                        c => c,
                    };
                    self.eat();
                    string.push(c);
                }
                Ok(self.tok(TokTy::Str(string)))
            }
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
                let directive = match self.eat_id() {
                    "text" => Directive::Text,
                    "data" => Directive::Data,
                    "word" => Directive::Word,
                    "halfword" => Directive::HalfWord,
                    "byte" => Directive::Byte,
                    "ascii" => Directive::Ascii,
                    "asciiz" => Directive::Asciiz,
                    id if id.is_empty() => {
                        return Err(self.err("Expected directive after '.'"))
                    }
                    id => {
                        return Err(self.err(&format!("Invalid directive '.{id}'")))
                    }
                };
                Ok(self.tok(TokTy::Directive(directive)))
            }
            ',' => {
                self.eat();
                Ok(self.tok(TokTy::Comma))
            }
            '(' => {
                self.eat();
                Ok(self.tok(TokTy::LParan))
            }
            ')' => {
                self.eat();
                Ok(self.tok(TokTy::RParan))
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
                    Register(num as u8)
                } else {
                    let id = self.eat_id();
                    let reg = REGISTER_NAMES.iter()
                        .position(|k| *k == id)
                        .ok_or_else(|| {
                            self.err(&format!("Invalid register '${}'", id))
                        })?;
                    Register(reg as u8)
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

/// Make an iterator of tokens of from input string.
pub fn tokenize(
    input: &str
) -> impl Iterator<Item = Result<Tok, Error>> + Clone + '_ {
    let mut lexer = Lexer::new(input);
    std::iter::from_fn(move || {
        match lexer.next_tok() {
            Ok(t) if t.ty == TokTy::Eof => None,
            t => Some(t),
        }
    })
}

pub const REGISTER_NAMES: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6",
    "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp",
    "ra",
];

#[test]
fn comment() {
    let input = r#"
        # Comment Comment Comment.
        add $t2, $t0, $t1 # Comment
        # Comment Comment Comment.
    "#;
    let expected = [
        TokTy::Id("add"),
        TokTy::Reg(Register(10)),
        TokTy::Comma,
        TokTy::Reg(Register(8)),
        TokTy::Comma,
        TokTy::Reg(Register(9)),
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
        .data
        w: .word 32 
        hw: .halfword 65
        b: .byte 12
        a: .ascii "String\0"
        az: .asciiz "String"

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
        TokTy::Directive(Directive::Data),

        TokTy::Label("w"),
        TokTy::Directive(Directive::Word),
        TokTy::Num(32),

        TokTy::Label("hw"),
        TokTy::Directive(Directive::HalfWord),
        TokTy::Num(65),

        TokTy::Label("b"),
        TokTy::Directive(Directive::Byte),
        TokTy::Num(12),

        TokTy::Label("a"),
        TokTy::Directive(Directive::Ascii),
        TokTy::Str("String\0".to_owned()),
        
        TokTy::Label("az"),
        TokTy::Directive(Directive::Asciiz),
        TokTy::Str("String".to_owned()),

        TokTy::Directive(Directive::Text),
        TokTy::Label("main"),

        TokTy::Id("li"),
        TokTy::Reg(Register(2)),
        TokTy::Comma,
        TokTy::Num(4),

        TokTy::Id("la"),
        TokTy::Reg(Register(4)),
        TokTy::Comma,
        TokTy::Id("string1"),

        TokTy::Id("syscall"),

        TokTy::Id("li"),
        TokTy::Reg(Register(2)),
        TokTy::Comma,
        TokTy::Num(5),
        
        TokTy::Id("syscall"),

        TokTy::Id("move"),
        TokTy::Reg(Register(8)),
        TokTy::Comma,
        TokTy::Reg(Register(2)),

        TokTy::Id("li"),
        TokTy::Reg(Register(2)),
        TokTy::Comma,
        TokTy::Num(4),

        TokTy::Id("la"),
        TokTy::Reg(Register(4)),
        TokTy::Comma,
        TokTy::Id("endLine"),

        TokTy::Id("syscall"),

        TokTy::Id("li"),
        TokTy::Reg(Register(2)),
        TokTy::Comma,
        TokTy::Num(4),

        TokTy::Id("la"),
        TokTy::Reg(Register(4)),
        TokTy::Comma,
        TokTy::Id("string2"),

        TokTy::Id("syscall"),

        TokTy::Id("li"),
        TokTy::Reg(Register(2)),
        TokTy::Comma,
        TokTy::Num(5),

        TokTy::Id("syscall"),

        TokTy::Id("move"),
        TokTy::Reg(Register(9)),
        TokTy::Comma,
        TokTy::Reg(Register(2)),

        TokTy::Id("li"),
        TokTy::Reg(Register(2)),
        TokTy::Comma,
        TokTy::Num(4),

        TokTy::Id("la"),
        TokTy::Reg(Register(4)),
        TokTy::Comma,
        TokTy::Id("string3"),

        TokTy::Id("syscall"),

        TokTy::Id("add"),
        TokTy::Reg(Register(10)),
        TokTy::Comma,
        TokTy::Reg(Register(9)),
        TokTy::Comma,
        TokTy::Reg(Register(8)),

        TokTy::Id("li"),
        TokTy::Reg(Register(2)),
        TokTy::Comma,
        TokTy::Num(1),

        TokTy::Id("move"),
        TokTy::Reg(Register(4)),
        TokTy::Comma,
        TokTy::Reg(Register(10)),

        TokTy::Id("syscall"),

        TokTy::Id("li"),
        TokTy::Reg(Register(2)),
        TokTy::Comma,
        TokTy::Num(10),

        TokTy::Id("syscall"),
    ];
    let res: Vec<TokTy> = tokenize(input).map(|t| t.unwrap().ty).collect();
    for (got, exp) in res.iter().zip(expected) {
        assert_eq!(*got, exp);
    }
}
