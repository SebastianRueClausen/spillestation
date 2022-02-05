use crate::Error;
use crate::ir::{Label, Register, Directive, Ir, IrTy};
use crate::lex::{self, Tok, TokTy};

#[derive(Clone, Copy)]
enum Section {
    Text,
    Data,
}

/// Scan and parse 'input'. Returns Ir code for both text (first) and data (second) sections.
pub fn parse<'a>(input: &'a str) -> Result<(Vec<Ir<'a>>, Vec<Ir<'a>>), Error> {
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

    /// Parse a register. fx '$sp'.
    fn reg(&mut self) -> Result<Register, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Reg(idx) => Ok(idx),
            _ => Err(self.err("Expected register argument")),
        }
    }

    /// Parse a register and offset used as arguments for load and store instructions.
    ///
    /// '''ignore
    /// lw  $t1, 4($sp)
    /// '''
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

    /// Parse an 'address'. Could be either a reference to a label or an absolute address.
    fn addr(&mut self) -> Result<Label<'a>, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Id(id) => Ok(Label::Label(id)),
            TokTy::Num(num) => Ok(Label::Abs(num)),
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

    pub fn parse(&mut self) -> Result<(Vec<Ir<'a>>, Vec<Ir<'a>>), Error> {
        let mut text = Vec::with_capacity(64);
        let mut data = Vec::with_capacity(32);

        let mut push_ir = |sec, ir| {
            match sec {
                Section::Data => data.push(ir),
                Section::Text => text.push(ir),
            }
        };

        while let Some(tok) = self.input.next() {
            let tok = tok?;
            match tok.ty {
                TokTy::Directive(dir) => match dir {
                    Directive::Data => self.sec = Section::Data,
                    Directive::Text => self.sec = Section::Text,
                    Directive::Word => {
                        push_ir(self.sec, Ir::new(tok.line, IrTy::Word(self.num()?)));
                    }
                    Directive::HalfWord => {
                        let num: u16 = self.num()?.try_into().map_err(|err| {
                            self.err(&format!("{err}"))
                        })?;
                        push_ir(self.sec, Ir::new(tok.line, IrTy::HalfWord(num)))
                    }
                    Directive::Byte => {
                        let num: u8 = self.num()?.try_into().map_err(|err| {
                            self.err(&format!("{err}"))
                        })?;
                        push_ir(self.sec, Ir::new(tok.line, IrTy::Byte(num)))
                    }
                    ty @ Directive::Ascii | ty @ Directive::Asciiz => {
                        let tok = self.expect_some()?;
                        if let TokTy::Str(mut string) = tok.ty {
                            if let Directive::Asciiz = ty {
                                string.push('\0');
                            }
                            push_ir(self.sec, Ir::new(tok.line, IrTy::Ascii(string)))
                        } else {
                            return Err(self.err(
                                &format!("Expected string literal")
                            ));
                        }
                    }
                }
                TokTy::Label(id) => {
                    push_ir(self.sec, Ir::new(tok.line, IrTy::Label(id)));
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
                        "sll" => IrTy::Sll(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?, 
                        ),
                        "srl" => IrTy::Srl(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?, 
                        ),
                        "sra" => IrTy::Sra(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?, 
                        ),
                        "sllv" => IrTy::Sllv(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "srlv" => IrTy::Srlv(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "srav" => IrTy::Srav(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "jr" => IrTy::Jr(self.reg()?),
                        "jalr" => IrTy::Jalr(
                            self.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "syscall" => IrTy::Syscall(self.num()?),
                        "break" => IrTy::Break(self.num()?),
                        "mfhi" => IrTy::Mfhi(self.reg()?),
                        "mthi" => IrTy::Mthi(self.reg()?),
                        "mflo" => IrTy::Mflo(self.reg()?),
                        "mtlo" => IrTy::Mtlo(self.reg()?),
                        "mult" => IrTy::Mult(
                            self.reg()?,
                            self.comma()?.reg()?
                        ),
                        "multu" => IrTy::Multu(
                            self.reg()?,
                            self.comma()?.reg()?
                        ),
                        "div" => IrTy::Div(
                            self.reg()?,
                            self.comma()?.reg()?
                        ),
                        "divu" => IrTy::Divu(
                            self.reg()?,
                            self.comma()?.reg()?
                        ),
                        "add" => IrTy::Add(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "addu" => IrTy::Addu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "sub" => IrTy::Sub(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "subu" => IrTy::Subu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "and" => IrTy::And(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "or" => IrTy::Or(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "xor" => IrTy::Xor(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "nor" => IrTy::Nor(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "slt" => IrTy::Slt(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "sltu" => IrTy::Sltu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "bgez" => IrTy::Bgez(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bltz" => IrTy::Bltz(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bgezal" => IrTy::Bgezal(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bltzal" => IrTy::Bltzal(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "j" => IrTy::J(self.addr()?),
                        "jal" => IrTy::Jal(self.addr()?),
                        "beq" => IrTy::Beq(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bne" => IrTy::Bne(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "blez" => IrTy::Blez(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bgtz" => IrTy::Bgtz(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "addi" => IrTy::Addi(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "addiu" => IrTy::Addiu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "slti" => IrTy::Slti(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "sltiu" => IrTy::Sltiu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "andi" => IrTy::Andi(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "ori" => IrTy::Ori(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "xori" => IrTy::Xori(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lui" => IrTy::Lui(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lb" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Lb(rt, rd, offset)
                        }
                        "lh" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Lh(rt, rd, offset)
                        },
                        "lwl" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Lwl(rt, rd, offset)
                        },
                        "lw" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Lw(rt, rd, offset)
                        },
                        "lbu" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Lbu(rt, rd, offset)
                        },
                        "lhu" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Lhu(rt, rd, offset)
                        },
                        "lwr" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Lwr(rt, rd, offset)
                        },
                        "sb" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Sb(rt, rd, offset)
                        },
                        "sh" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Sh(rt, rd, offset)
                        },
                        "swl" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Swl(rt, rd, offset)
                        },
                        "sw" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Sw(rt, rd, offset)
                        },
                        "swr" => {
                            let rt = self.reg()?;
                            let (rd, offset) = self.comma()?.reg_offset()?;
                            IrTy::Swr(rt, rd, offset)
                        },
                        "mfc0" => IrTy::Mfc0(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "mtc0" => IrTy::Mtc0(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "mfc2" => IrTy::Mfc2(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "mtc2" => IrTy::Mtc2(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "nop" => IrTy::Nop,
                        "move" => IrTy::Move(
                            self.reg()?,
                            self.comma()?.reg()?,
                        ),
                        "li" => IrTy::Li(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "la" => IrTy::La(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "b" => IrTy::B(self.addr()?),
                        "beqz" => IrTy::Beqz(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        "bnez" => IrTy::Bnez(
                            self.reg()?,
                            self.comma()?.addr()?,
                        ),
                        id => return Err(self.err(
                            &format!("Unknown instruction '{}'", id)
                        )),
                    };
                    push_ir(self.sec, Ir::new(tok.line, ins));
                }
            }
        }
        Ok((text, data))
    }
}
