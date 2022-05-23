use crate::Error;
use crate::ins::{Address, Register, Directive, Ins, InsTy};
use crate::lex::{self, Tok, TokTy};

#[derive(Clone, Copy)]
enum Section {
    Text,
    Data,
}

pub struct ParsedSource<'a> {
    pub text: Vec<Ins<'a>>,
    pub data: Vec<Ins<'a>>,
}

impl<'a> ParsedSource<'a> {
    fn new() -> Self {
        Self {
            text: Vec::with_capacity(64),
            data: Vec::with_capacity(32),
        }
    }
}

/// Scan and parse `input`. Returns Ir code for both text (first) and data (second) sections.
pub fn parse<'a>(input: &'a str) -> Result<ParsedSource<'a>, Error> {
    Parser::new(lex::tokenize(input)).parse()
}

struct Parser<'a, Iter: Iterator<Item = Result<Tok<'a>, Error>>> {
    /// The current section.
    sec: Section,
    /// The line number of the previous token.
    line: usize,
    /// Input token iterator.
    input: Iter,
}

impl<'a, Iter: Clone + Iterator<Item = Result<Tok<'a>, Error>>> Parser<'a, Iter>  {
    fn new(input: Iter) -> Self {
        Self {
            sec: Section::Text,
            line: 1,
            input
        }
    }

    fn err(&mut self, msg: impl Into<String>) -> Error {
        Error::new(self.line, msg)
    }
   
    /// Expect some kind of token. Returns an error if the whole input has been consumed.
    fn expect_some(&mut self) -> Result<Tok<'a>, Error> {
        self.input.next()
            .unwrap_or_else(|| {
                Err(self.err("Unexpected end of input"))
            })
            .and_then(|tok| {
                self.line = tok.line;
                Ok(tok)
            })
    }

    /// Parse a register. fx `$sp`.
    fn reg(&mut self) -> Result<Register, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Reg(idx) => Ok(idx),
            _ => Err(self.err("Expected register argument")),
        }
    }

    /// Parse a register and offset used as arguments for load and store instructions.
    ///
    /// ```ignore
    /// lw  $t1, 4($sp)
    /// ```
    fn reg_offset(&mut self) -> Result<(Register, u32), Error> {
        let num = self.num()?;
        if !matches!(self.expect_some()?.ty, TokTy::LParan) {
            return Err(self.err("Expected '('")); 
        }
        let reg = self.reg()?;
        if !matches!(self.expect_some()?.ty, TokTy::RParan) {
            return Err(self.err("Expected ')'")); 
        }
        Ok((reg, num))
    }

    /// Parse a number.
    fn num(&mut self) -> Result<u32, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Num(num) => Ok(num),
            _ => Err(self.err("Expected immediate value")),
        }
    }

    /// Parse an address. Could be either a reference to a label or an absolute address.
    fn addr(&mut self) -> Result<Address<'a>, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Id(id) => Ok(Address::Label(id)),
            TokTy::Num(num) => Ok(Address::Abs(num)),
            _ => Err(self.err("Expected label or address")),
        }
    }

    fn comma(&mut self) -> Result<&mut Self, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Comma => Ok(self),
            _ => Err(self.err("Expected comma")),
        }
    }

    pub fn parse(&mut self) -> Result<ParsedSource<'a>, Error> {
        let mut parsed_source = ParsedSource::new();

        let mut push_ins = |sec, ir| {
            match sec {
                Section::Data => parsed_source.data.push(ir),
                Section::Text => parsed_source.text.push(ir),
            }
        };

        while let Some(tok) = self.input.next() {
            let tok = tok?;
            match tok.ty {
                TokTy::Directive(dir) => match dir {
                    Directive::Data => self.sec = Section::Data,
                    Directive::Text => self.sec = Section::Text,
                    Directive::Word => {
                        push_ins(self.sec, Ins::new(tok.line, InsTy::Word(self.num()?)));
                    }
                    Directive::HalfWord => {
                        let num: u16 = self.num()?.try_into().map_err(|err| {
                            self.err(&format!("{err}"))
                        })?;
                        push_ins(self.sec, Ins::new(tok.line, InsTy::HalfWord(num)))
                    }
                    Directive::Byte => {
                        let num: u8 = self.num()?.try_into().map_err(|err| {
                            self.err(&format!("{err}"))
                        })?;
                        push_ins(self.sec, Ins::new(tok.line, InsTy::Byte(num)))
                    }
                    ty @ Directive::Ascii | ty @ Directive::Asciiz => {
                        let tok = self.expect_some()?;
                        if let TokTy::Str(mut string) = tok.ty {
                            if let Directive::Asciiz = ty {
                                string.push('\0');
                            }
                            push_ins(self.sec, Ins::new(tok.line, InsTy::Ascii(string)))
                        } else {
                            return Err(self.err(
                                &format!("Expected string literal")
                            ));
                        }
                    }
                }
                TokTy::Label(id) => {
                    push_ins(self.sec, Ins::new(tok.line, InsTy::Label(id)));
                }
                TokTy::Num(..)
                | TokTy::Str(..)
                | TokTy::Reg(..)
                | TokTy::LParan
                | TokTy::RParan
                | TokTy::Comma => {
                    return Err(self.err(
                        &format!("Expected label, section or instruction")
                    ));
                }
                // The lexer should catch any EOF tokens and return None instead.
                TokTy::Eof => unreachable!(),
                TokTy::Id(id) => {
                    let ins = match id {
                        "sll" => InsTy::Sll(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?, 
                        ),
                        "srl" => InsTy::Srl(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?, 
                        ),
                        "sra" => InsTy::Sra(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?, 
                        ),
                        "sllv" => InsTy::Sllv(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "srlv" => InsTy::Srlv(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "srav" => InsTy::Srav(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "jr" => InsTy::Jr(self.reg()?),
                        "jalr" => InsTy::Jalr(
                            self.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "syscall" => InsTy::Syscall(self.num()?),
                        "break" => InsTy::Break(self.num()?),
                        "mfhi" => InsTy::Mfhi(self.reg()?),
                        "mthi" => InsTy::Mthi(self.reg()?),
                        "mflo" => InsTy::Mflo(self.reg()?),
                        "mtlo" => InsTy::Mtlo(self.reg()?),
                        "mult" => InsTy::Mult(
                            self.reg()?,
                            self.comma()?.reg()?
                        ),
                        "multu" => InsTy::Multu(
                            self.reg()?,
                            self.comma()?.reg()?
                        ),
                        "div" => InsTy::Div(
                            self.reg()?,
                            self.comma()?.reg()?
                        ),
                        "divu" => InsTy::Divu(
                            self.reg()?,
                            self.comma()?.reg()?
                        ),
                        "add" => InsTy::Add(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "addu" => InsTy::Addu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "sub" => InsTy::Sub(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "subu" => InsTy::Subu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "and" => InsTy::And(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "or" => InsTy::Or(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "xor" => InsTy::Xor(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "nor" => InsTy::Nor(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "slt" => InsTy::Slt(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "sltu" => InsTy::Sltu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "bgez" => InsTy::Bgez(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bltz" => InsTy::Bltz(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bgezal" => InsTy::Bgezal(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bltzal" => InsTy::Bltzal(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "j" => InsTy::J(self.addr()?),
                        "jal" => InsTy::Jal(self.addr()?),
                        "beq" => InsTy::Beq(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bne" => InsTy::Bne(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "blez" => InsTy::Blez(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bgtz" => InsTy::Bgtz(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "addi" => InsTy::Addi(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "addiu" => InsTy::Addiu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "slti" => InsTy::Slti(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "sltiu" => InsTy::Sltiu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "andi" => InsTy::Andi(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "ori" => InsTy::Ori(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "xori" => InsTy::Xori(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lui" => InsTy::Lui(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lb" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Lb(rt, rd, offset)
                        }
                        "lh" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Lh(rt, rd, offset)
                        },
                        "lwl" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Lwl(rt, rd, offset)
                        },
                        "lw" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Lw(rt, rd, offset)
                        },
                        "lbu" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Lbu(rt, rd, offset)
                        },
                        "lhu" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Lhu(rt, rd, offset)
                        },
                        "lwr" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Lwr(rt, rd, offset)
                        },
                        "sb" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Sb(rt, rd, offset)
                        },
                        "sh" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Sh(rt, rd, offset)
                        },
                        "swl" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Swl(rt, rd, offset)
                        },
                        "sw" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Sw(rt, rd, offset)
                        },
                        "swr" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            InsTy::Swr(rt, rd, offset)
                        },
                        "mfc0" => InsTy::Mfc0(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "mtc0" => InsTy::Mtc0(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "mfc2" => InsTy::Mfc2(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "mtc2" => InsTy::Mtc2(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "nop" => InsTy::Nop,
                        "move" => InsTy::Move(
                            self.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "li" => InsTy::Li(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "la" => InsTy::La(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "b" => InsTy::B(self.addr()?),
                        "beqz" => InsTy::Beqz(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bnez" => InsTy::Bnez(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        id => return Err(self.err(
                            &format!("Unknown instruction '{}'", id)
                        )),
                    };
                    push_ins(self.sec, Ins::new(tok.line, ins));
                }
            }
        }

        Ok(parsed_source)
    }
}
