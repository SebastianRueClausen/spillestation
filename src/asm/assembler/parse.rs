use super::{Error, Section};
use super::lex::{self, Tok, TokTy};
use crate::cpu::RegIdx;

#[derive(Clone, Copy)]
pub enum IrAddr<'a> {
    Label(&'a str),
    Abs(u32),
}

#[derive(Clone, Copy)]
pub enum IrTy<'a> {
    Sll(RegIdx, RegIdx, u32), 
    Srl(RegIdx, RegIdx, u32),
    Sra(RegIdx, RegIdx, u32),
    Sllv(RegIdx, RegIdx, RegIdx),
    Srlv(RegIdx, RegIdx, RegIdx),
    Srav(RegIdx, RegIdx, RegIdx),
    Jr(RegIdx),
    Jalr(RegIdx, RegIdx),
    Syscall(u32),
    Break(u32),
    Mfhi(RegIdx),
    Mthi(RegIdx),
    Mflo(RegIdx),
    Mtlo(RegIdx),
    Mult(RegIdx, RegIdx),
    Multu(RegIdx, RegIdx),
    Div(RegIdx, RegIdx),
    Divu(RegIdx, RegIdx),
    Add(RegIdx, RegIdx, RegIdx),
    Addu(RegIdx, RegIdx, RegIdx),
    Sub(RegIdx, RegIdx, RegIdx),
    Subu(RegIdx, RegIdx, RegIdx),
    And(RegIdx, RegIdx, RegIdx),
    Or(RegIdx, RegIdx, RegIdx),
    Xor(RegIdx, RegIdx, RegIdx),
    Nor(RegIdx, RegIdx, RegIdx),
    Slt(RegIdx, RegIdx, RegIdx),
    Sltu(RegIdx, RegIdx, RegIdx),
    Bgez(RegIdx, IrAddr<'a>),
    Bltz(RegIdx, IrAddr<'a>),
    Bgezal(RegIdx, IrAddr<'a>),
    Bltzal(RegIdx, IrAddr<'a>),
    J(IrAddr<'a>),
    Jal(IrAddr<'a>),
    Beq(RegIdx, RegIdx, IrAddr<'a>),
    Bne(RegIdx, RegIdx, IrAddr<'a>),
    Blez(RegIdx, IrAddr<'a>),
    Bgtz(RegIdx, IrAddr<'a>),
    Addi(RegIdx, RegIdx, u32),
    Addiu(RegIdx, RegIdx, u32),
    Slti(RegIdx, RegIdx, u32),
    Sltiu(RegIdx, RegIdx, u32),
    Andi(RegIdx, RegIdx, u32),
    Ori(RegIdx, RegIdx, u32),
    Xori(RegIdx, RegIdx, u32),
    Lui(RegIdx, u32),
    Lb(RegIdx, RegIdx, u32),
    Lh(RegIdx, RegIdx, u32),
    Lwl(RegIdx, RegIdx, u32),
    Lw(RegIdx, RegIdx, u32),
    Lbu(RegIdx, RegIdx, u32),
    Lhu(RegIdx, RegIdx, u32),
    Lwr(RegIdx, RegIdx, u32),
    Sb(RegIdx, RegIdx, u32),
    Sh(RegIdx, RegIdx, u32),
    Swl(RegIdx, RegIdx, u32),
    Sw(RegIdx, RegIdx, u32),
    Swr(RegIdx, RegIdx, u32),

    Mfc0(RegIdx, u32),
    Mtc0(RegIdx, u32),

    Label(&'a str),

    Nop,
    Move(RegIdx, RegIdx),
    Li(RegIdx, u32),
    La(RegIdx, IrAddr<'a>),
    B(IrAddr<'a>),
    Beqz(RegIdx, IrAddr<'a>),
    Bnez(RegIdx, IrAddr<'a>),
}

impl<'a> IrTy<'a> {
    pub fn size(&self) -> u32 {
        match *self {
            IrTy::Label(..) => 0,
            IrTy::La(..) => 8,
            IrTy::Li(_, val) => {
                let mut b = 0;
                if val & 0xffff_0000 != 0 {
                    b += 4;
                }
                if val & 0xffff != 0 || val == 0 {
                    b += 4;
                }
                b
            }
            _ => 4,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Ir<'a> {
    pub ty: IrTy<'a>,
    pub line: usize,
}

impl<'a> Ir<'a> {
    fn new(line: usize, ty: IrTy<'a>) -> Self {
        Self { ty, line }
    }
}

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

    fn reg(&mut self) -> Result<RegIdx, Error> {
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

    fn addr(&mut self) -> Result<IrAddr<'a>, Error> {
        let tok = self.expect_some()?;
        match tok.ty {
            TokTy::Id(id) => Ok(IrAddr::Label(id)),
            TokTy::Num(num) => Ok(IrAddr::Abs(num)),
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
