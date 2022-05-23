use splst_util::{Bit, BitSet};

use crate::parse::ParsedSource;
use crate::ins::{Ins, InsTy, Register, Address};
use crate::Error;

use std::collections::HashMap;

/// A builder struct to build binary instruction.
#[derive(Clone, Copy)]
struct InsBuilder(u32);

impl InsBuilder {
    fn op(val: u32) -> Self {
        Self(0_u32.set_bit_range(26, 31, val))
    }

    fn special(val: u32) -> Self {
        Self(0_u32.set_bit_range(0, 5, val))
    }

    fn cop_op(self, val: u32) -> Self {
        Self(self.0.set_bit_range(21, 25, val))
    }

    fn cop_reg(self, reg: u32) -> Self {
        Self(self.0.set_bit_range(11, 15, reg))
    }

    fn imm(self, val: u32) -> Self {
        Self(self.0.set_bit_range(0, 15, val))
    }

    fn target(self, val: u32) -> Self {
        Self(self.0.set_bit_range(0, 25, val))
    }

    fn shift(self, val: u32) -> Self {
        Self(self.0.set_bit_range(6, 10, val))
    }

    fn rd(self, val: Register) -> Self {
        Self(self.0.set_bit_range(11, 15, val.0.into()))
    }

    fn rt(self, val: Register) -> Self {
        Self(self.0.set_bit_range(16, 20, val.0.into()))
    }

    fn rs(self, val: Register) -> Self {
        Self(self.0.set_bit_range(21, 25, val.0.into()))
    }

    fn bgez(self, val: bool) -> Self {
        Self(self.0.set_bit(16, val))
    }

    fn link(self, val: bool) -> Self {
        Self(self.0.set_bit(20, val))
    }

    fn sys(self, val: u32) -> Self {
        Self(self.0.set_bit_range(6, 26, val))
    }
}

struct CodeGen<'a> {
    code: Vec<u8>,
    labels: HashMap<&'a str, u32>,
}

impl<'a> CodeGen<'a> {
    fn new() -> Self {
        Self {
            code: Vec::new(),
            labels: HashMap::new(),
        }
    }

    /// Resolve a [`Address`] by finding the address it's pointing to. Returns an error if the label
    /// can't be found.
    fn resolve_labels(&self, line: usize, addr: &Address) -> Result<u32, Error> {
        match addr {
            Address::Label(id) => match self.labels.get(id) {
                Some(addr) => Ok(*addr),
                None => Err(
                    Error::new(line, format!("Unresolved symbol '{}'", id))
                ),
            }
            Address::Abs(addr) => Ok(*addr),
        }
    }

    /// Find the branch offset of a [`Address`]. Must be called before adding any following
    /// instructions.
    fn branch_offset(&self, line: usize, addr: &Address) -> Result<i32, Error> {
        let loc = self.code.len() as i32 + 4;
        let dest = self.resolve_labels(line, addr)? as i32;
        Ok((dest - loc) >> 2)
    }

    /// Find the jump address of a [`Address`]. Points to the next instruction / data after the
    /// Address.
    fn jump_addr(&self, line: usize, addr: &Address) -> Result<u32, Error> {
        Ok(self.resolve_labels(line, addr)? >> 2)
    }

    fn gen_ins(&mut self, ins: InsBuilder) {
        self.code.extend_from_slice(&ins.0.to_le_bytes());
    }

    fn assemble_ins(&mut self, ins: &Ins<'a>) -> Result<(), Error> {
        match ins.ty {
            InsTy::Sll(rd, rt, shift) => {
                self.gen_ins(InsBuilder::special(0x0).rd(rd).rt(rt).shift(shift));
            }
            InsTy::Srl(rd, rt, shift) => {
                self.gen_ins(InsBuilder::special(0x2).rd(rd).rt(rt).shift(shift));
            }
            InsTy::Sra(rd, rt, shift) => {
                self.gen_ins(InsBuilder::special(0x3).rd(rd).rt(rt).shift(shift));
            }
            InsTy::Sllv(rd, rt, rs) => {
               self.gen_ins(InsBuilder::special(0x4).rd(rd).rt(rt).rs(rs));
            }
            InsTy::Srlv(rd, rt, rs) => {
               self.gen_ins(InsBuilder::special(0x6).rd(rd).rt(rt).rs(rs));
            }
            InsTy::Srav(rd, rt, rs) => {
               self.gen_ins(InsBuilder::special(0x7).rd(rd).rt(rt).rs(rs));
            }
            InsTy::Jr(rs) => {
                self.gen_ins(InsBuilder::special(0x8).rs(rs));
            }
            InsTy::Jalr(rd, rs) => {
                self.gen_ins(InsBuilder::special(0x9).rd(rd).rs(rs));
            }
            InsTy::Syscall(val) => {
                self.gen_ins(InsBuilder::special(0xc).sys(val));
            }
            InsTy::Break(val) => {
                self.gen_ins(InsBuilder::special(0xd).sys(val));
            }
            InsTy::Mfhi(rd) => {
                self.gen_ins(InsBuilder::special(0x10).rd(rd));
            }
            InsTy::Mthi(rs) => {
                self.gen_ins(InsBuilder::special(0x11).rs(rs));
            }
            InsTy::Mflo(rd) => {
                self.gen_ins(InsBuilder::special(0x12).rd(rd));
            }
            InsTy::Mtlo(rs) => {
                self.gen_ins(InsBuilder::special(0x13).rs(rs));
            }
            InsTy::Mult(rs, rt) => {
                self.gen_ins(InsBuilder::special(0x18).rs(rs).rt(rt));
            }
            InsTy::Multu(rs, rt) => {
                self.gen_ins(InsBuilder::special(0x19).rs(rs).rt(rt));
            }
            InsTy::Div(rs, rt) => {
                self.gen_ins(InsBuilder::special(0x1a).rs(rs).rt(rt));
            }
            InsTy::Divu(rs, rt) => {
                self.gen_ins(InsBuilder::special(0x1b).rs(rs).rt(rt));
            }
            InsTy::Add(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x20).rd(rd).rs(rs).rt(rt));
            }
            InsTy::Addu(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x21).rd(rd).rs(rs).rt(rt));
            }
            InsTy::Sub(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x22).rd(rd).rs(rs).rt(rt));
            }
            InsTy::Subu(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x23).rd(rd).rs(rs).rt(rt));
            }
            InsTy::And(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x24).rd(rd).rs(rs).rt(rt));
            }
            InsTy::Or(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x25).rd(rd).rs(rs).rt(rt));
            }
            InsTy::Xor(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x26).rd(rd).rs(rs).rt(rt));
            }
            InsTy::Nor(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x27).rd(rd).rs(rs).rt(rt));
            }
            InsTy::Slt(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x2a).rd(rd).rs(rs).rt(rt));
            }
            InsTy::Sltu(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x2b).rd(rd).rs(rs).rt(rt));
            }
            InsTy::Bgez(rs, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x1)
                    .link(false)
                    .bgez(true)
                    .rs(rs)
                    .imm(off as u32));
            }
            InsTy::Bltz(rs, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x1)
                    .link(false)
                    .bgez(false)
                    .rs(rs)
                    .imm(off as u32));
            }
            InsTy::Bgezal(rs, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x1)
                    .link(true)
                    .bgez(true)
                    .rs(rs)
                    .imm(off as u32));
            }
            InsTy::Bltzal(rs, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x1)
                    .link(true)
                    .bgez(false)
                    .rs(rs)
                    .imm(off as u32));
            }
            InsTy::J(addr) => {
                let addr = self.jump_addr(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x2).target(addr));
            }
            InsTy::Jal(addr) => {
                let addr = self.jump_addr(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x3).target(addr));
            }
            InsTy::Beq(rs, rt, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x4).rs(rs).rt(rt).imm(off as u32));
            }
            InsTy::Bne(rs, rt, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x5).rs(rs).rt(rt).imm(off as u32));
            }
            InsTy::Blez(rs, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x6).rs(rs).imm(off as u32));
            }
            InsTy::Bgtz(rs, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x7).rs(rs).imm(off as u32));
            }
            InsTy::Addi(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x8).rt(rt).rs(rs).imm(val));
            }
            InsTy::Addiu(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x9).rt(rt).rs(rs).imm(val));
            }
            InsTy::Slti(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xa).rt(rt).rs(rs).imm(val));
            }
            InsTy::Sltiu(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xb).rt(rt).rs(rs).imm(val));
            }
            InsTy::Andi(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xc).rt(rt).rs(rs).imm(val));
            }
            InsTy::Ori(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xd).rt(rt).rs(rs).imm(val));
            }
            InsTy::Xori(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xe).rt(rt).rs(rs).imm(val));
            }
            InsTy::Lui(rt, val) => {
                self.gen_ins(InsBuilder::op(0xf).rt(rt).imm(val));
            }
            InsTy::Mfc0(rt, reg) => {
                self.gen_ins(InsBuilder::op(0x10).cop_op(0).rt(rt).cop_reg(reg));
            }
            InsTy::Mtc0(rt, reg) => {
                self.gen_ins(InsBuilder::op(0x10).cop_op(0x4).rt(rt).cop_reg(reg));
            }
            InsTy::Mfc2(rt, reg) => {
                self.gen_ins(InsBuilder::op(0x12).cop_op(0).rt(rt).cop_reg(reg));
            }
            InsTy::Mtc2(rt, reg) => {
                self.gen_ins(InsBuilder::op(0x12).cop_op(0x4).rt(rt).cop_reg(reg));
            }
            InsTy::Lb(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x20).rt(rt).rs(rs).imm(val));
            }
            InsTy::Lh(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x21).rt(rt).rs(rs).imm(val));
            }
            InsTy::Lwl(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x22).rt(rt).rs(rs).imm(val));
            }
            InsTy::Lw(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x23).rt(rt).rs(rs).imm(val));
            }
            InsTy::Lbu(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x24).rt(rt).rs(rs).imm(val));
            }
            InsTy::Lhu(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x25).rt(rt).rs(rs).imm(val));
            }
            InsTy::Lwr(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x26).rt(rt).rs(rs).imm(val));
            }
            InsTy::Sb(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x28).rt(rt).rs(rs).imm(val));
            }
            InsTy::Sh(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x29).rt(rt).rs(rs).imm(val));
            }
            InsTy::Swl(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x2a).rt(rt).rs(rs).imm(val));
            }
            InsTy::Sw(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x2b).rt(rt).rs(rs).imm(val));
            }
            InsTy::Swr(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x2e).rt(rt).rs(rs).imm(val));
            }
            InsTy::Word(val) => {
                self.code.extend_from_slice(&val.to_le_bytes());
            }
            InsTy::HalfWord(val) => {
                self.code.extend_from_slice(&val.to_le_bytes());
            }
            InsTy::Byte(val) => {
                self.code.extend_from_slice(&val.to_le_bytes());
            }
            InsTy::Ascii(ref string) => {
                self.code.extend_from_slice(string.as_bytes());
            }
            InsTy::Nop => {
                self.gen_ins(InsBuilder::special(0x0)
                    .rd(Register(0))
                    .rt(Register(0))
                    .shift(0));
            }
            InsTy::Move(rd, rs) => {
                self.gen_ins(InsBuilder::special(0x21)
                    .rd(rd)
                    .rs(rs)
                    .rt(Register(0)));
            }
            InsTy::Li(reg, val) => {
                let (hi, lo) = (val.bit_range(16, 31), val.bit_range(0, 15));
                if hi != 0 {
                    self.gen_ins(InsBuilder::op(0xf).rt(reg).imm(hi));
                    if lo != 0 {
                        self.gen_ins(InsBuilder::op(0xd)
                            .rt(reg)
                            .rs(reg)
                            .imm(lo));
                    }
                } else {
                    self.gen_ins(InsBuilder::op(0xd)
                        .rt(reg)
                        .rs(Register(0))
                        .imm(lo));
                }
            }
            InsTy::La(reg, addr) => {
                let val = self.resolve_labels(ins.line, &addr)?;
                let (hi, lo) = (val.bit_range(16, 31), val.bit_range(0, 15));
                self.gen_ins(InsBuilder::op(0xf).rt(reg).imm(hi));
                self.gen_ins(InsBuilder::op(0xd).rs(reg).rt(reg).imm(lo));
            }
            InsTy::B(addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x4)
                    .rs(Register(0))
                    .rt(Register(0))
                    .imm(off as u32));
            }
            InsTy::Beqz(reg, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x4)
                    .rs(reg)
                    .rt(Register(0))
                    .imm(off as u32));
            }
            InsTy::Bnez(reg, addr) => {
                let off = self.branch_offset(ins.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x5)
                    .rs(reg)
                    .rt(Register(0))
                    .imm(off as u32));
            }
            InsTy::Label(_) => (),
        }
        Ok(())
    }
}

/// Generate binary machine code from [`Ins`] instructions.
pub fn gen_machine_code<'a>(
    parsed: Vec<ParsedSource<'a>>,
    base: u32
) -> Result<(Vec<u8>, u32), Error> {
    gen_ins(parsed.into_iter().flat_map(|s| s.text.into_iter().chain(s.data.into_iter())), base)
}

pub fn gen_ins<'a>(
    mut ins: impl Iterator<Item = Ins<'a>>,
    base: u32,
) -> Result<(Vec<u8>, u32), Error> {
    let mut gen = CodeGen::new();
    let mut addr = base;

    for ins in ins.by_ref() {
        if let InsTy::Label(id) = ins.ty {
            if gen.labels.insert(id, addr).is_some() {
                return Err(Error::new(
                    ins.line,
                    format!("Address '{}' redeclared", id),  
                ));
            }
        } else {
            addr += ins.ty.size();
        }
    }

    for ins in ins {
        gen.assemble_ins(&ins)?;
    }

    let main = match gen.labels.get("main") {
        Some(main) => *main,
        None => {
            return Err(Error::new(0, "No Address 'main'"));
        }
    };

    Ok((gen.code, main))
}
