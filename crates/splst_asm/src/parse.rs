use crate::Error;
use crate::ir::{Label, Register, Section, Ir, IrTy};
use crate::lex::{self, Tok, TokTy};

pub fn parse<'a>(input: &'a str) -> Result<Vec<Ir<'a>>, Error> {
    Parser::new(lex::tokenize(input)).parse()
}

struct Parser<'a, Iter: Iterator<Item = Result<Tok<'a>, Error>>> {
    section: Option<Section>,
    prev_line: usize,
    input: Iter,
}

impl<'a, Iter: Iterator<Item = Result<Tok<'a>, Error>>> Parser<'a, Iter>  {
    fn new(input: Iter) -> Self {
        Self {
            section: None,
            prev_line: 1,
            input
        }
    }

    fn err(&mut self, msg: impl Into<String>) -> Error {
        Error::new(self.prev_line, msg)
    }
    
    fn expect_some(&mut self) -> Result<Tok<'a>, Error> {
        self.input.next().unwrap_or(
            Err(self.err("Unexpected end of input"))
        )
    }

    fn reg(&mut self) -> Result<Register, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Reg(idx) => Ok(idx),
            _ => Err(self.err("Expected register argument")),
        }
    }

    fn num(&mut self) -> Result<u32, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Num(num) => Ok(num),
            _ => Err(self.err("Expected immediate value")),
        }
    }

    fn addr(&mut self) -> Result<Label<'a>, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Id(id) => Ok(Label::Label(id)),
            TokTy::Num(num) => Ok(Label::Abs(num)),
            _ => Err(self.err("Expected immediate value")),
        }
    }

    fn comma(&mut self) -> Result<&mut Self, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Comma => Ok(self),
            _ => Err(self.err("Expected comma")),
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Ir<'a>>, Error> {
        let mut ir = Vec::with_capacity(64);
        while let Some(tok) = self.input.next() {
            let tok = tok?;
            self.prev_line = tok.line;
            match tok.ty {
                TokTy::Section(sec) => self.section = Some(sec),
                TokTy::Label(id) => {
                    ir.push(Ir::new(tok.line, IrTy::Label(id)));
                }
                TokTy::Num(..) | TokTy::Reg(..) | TokTy::Comma => return Err(
                    self.err(&format!("Expected label, section or instruction"))
                ),
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
                        "lb" => IrTy::Lb(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lh" => IrTy::Lh(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lwl" => IrTy::Lwl(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lw" => IrTy::Lw(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lbu" => IrTy::Lbu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lhu" => IrTy::Lhu(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "lwr" => IrTy::Lwr(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "sb" => IrTy::Sb(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "sh" => IrTy::Sh(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "swl" => IrTy::Swl(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "sw" => IrTy::Sw(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "swr" => IrTy::Swr(
                            self.reg()?,
                            self.comma()?.reg()?,
                            self.comma()?.num()?,
                        ),
                        "mfc0" => IrTy::Mfc0(
                            self.reg()?,
                            self.comma()?.num()?,
                        ),
                        "mtc0" => IrTy::Mtc0(
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
                        id => return Err(
                            self.err(&format!("Unknown instruction '{}'", id))
                        ),
                    };
                    ir.push(Ir::new(tok.line, ins));
                }
            }
        }
        Ok(ir)
    }
}
