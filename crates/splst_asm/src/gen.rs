use splst_util::{Bit, BitSet};

use crate::parse::ParsedSource;
use crate::ir::{Ir, IrTy, Register, Label};
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

    /// Resolve a ['Label'] by finding the address it's pointing to. Returns an error if the label
    /// can't be found.
    fn resolve_label(&self, line: usize, addr: &Label) -> Result<u32, Error> {
        match addr {
            Label::Label(id) => match self.labels.get(id) {
                Some(addr) => Ok(*addr),
                None => Err(
                    Error::new(line, format!("Unresolved symbol '{}'", id))
                ),
            }
            Label::Abs(addr) => Ok(*addr),
        }
    }

    /// Find the branch offset of a ['Label']. Must be called before adding any following
    /// instructions.
    fn branch_offset(&self, line: usize, addr: &Label) -> Result<i32, Error> {
        let loc = self.code.len() as i32 + 4;
        let dest = self.resolve_label(line, addr)? as i32;
        Ok((dest - loc) >> 2)
    }

    /// Find the jump address of a ['Label']. Points to the next instruction / data after the
    /// label.
    fn jump_addr(&self, line: usize, addr: &Label) -> Result<u32, Error> {
        Ok(self.resolve_label(line, addr)? >> 2)
    }

    fn gen_ins(&mut self, ins: InsBuilder) {
        self.code.extend_from_slice(&ins.0.to_le_bytes());
    }

    fn assemble_ir(&mut self, ir: &Ir<'a>) -> Result<(), Error> {
        match ir.ty {
            IrTy::Sll(rd, rt, shift) => {
                self.gen_ins(InsBuilder::special(0x0).rd(rd).rt(rt).shift(shift));
            }
            IrTy::Srl(rd, rt, shift) => {
                self.gen_ins(InsBuilder::special(0x2).rd(rd).rt(rt).shift(shift));
            }
            IrTy::Sra(rd, rt, shift) => {
                self.gen_ins(InsBuilder::special(0x3).rd(rd).rt(rt).shift(shift));
            }
            IrTy::Sllv(rd, rt, rs) => {
               self.gen_ins(InsBuilder::special(0x4).rd(rd).rt(rt).rs(rs));
            }
            IrTy::Srlv(rd, rt, rs) => {
               self.gen_ins(InsBuilder::special(0x6).rd(rd).rt(rt).rs(rs));
            }
            IrTy::Srav(rd, rt, rs) => {
               self.gen_ins(InsBuilder::special(0x7).rd(rd).rt(rt).rs(rs));
            }
            IrTy::Jr(rs) => {
                self.gen_ins(InsBuilder::special(0x8).rs(rs));
            }
            IrTy::Jalr(rd, rs) => {
                self.gen_ins(InsBuilder::special(0x9).rd(rd).rs(rs));
            }
            IrTy::Syscall(val) => {
                self.gen_ins(InsBuilder::special(0xc).sys(val));
            }
            IrTy::Break(val) => {
                self.gen_ins(InsBuilder::special(0xd).sys(val));
            }
            IrTy::Mfhi(rd) => {
                self.gen_ins(InsBuilder::special(0x10).rd(rd));
            }
            IrTy::Mthi(rs) => {
                self.gen_ins(InsBuilder::special(0x11).rs(rs));
            }
            IrTy::Mflo(rd) => {
                self.gen_ins(InsBuilder::special(0x12).rd(rd));
            }
            IrTy::Mtlo(rs) => {
                self.gen_ins(InsBuilder::special(0x13).rs(rs));
            }
            IrTy::Mult(rs, rt) => {
                self.gen_ins(InsBuilder::special(0x18).rs(rs).rt(rt));
            }
            IrTy::Multu(rs, rt) => {
                self.gen_ins(InsBuilder::special(0x19).rs(rs).rt(rt));
            }
            IrTy::Div(rs, rt) => {
                self.gen_ins(InsBuilder::special(0x1a).rs(rs).rt(rt));
            }
            IrTy::Divu(rs, rt) => {
                self.gen_ins(InsBuilder::special(0x1b).rs(rs).rt(rt));
            }
            IrTy::Add(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x20).rd(rd).rs(rs).rt(rt));
            }
            IrTy::Addu(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x21).rd(rd).rs(rs).rt(rt));
            }
            IrTy::Sub(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x22).rd(rd).rs(rs).rt(rt));
            }
            IrTy::Subu(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x23).rd(rd).rs(rs).rt(rt));
            }
            IrTy::And(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x24).rd(rd).rs(rs).rt(rt));
            }
            IrTy::Or(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x25).rd(rd).rs(rs).rt(rt));
            }
            IrTy::Xor(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x26).rd(rd).rs(rs).rt(rt));
            }
            IrTy::Nor(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x27).rd(rd).rs(rs).rt(rt));
            }
            IrTy::Slt(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x2a).rd(rd).rs(rs).rt(rt));
            }
            IrTy::Sltu(rd, rs, rt) => {
                self.gen_ins(InsBuilder::special(0x2b).rd(rd).rs(rs).rt(rt));
            }
            IrTy::Bgez(rs, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x1)
                    .link(false)
                    .bgez(true)
                    .rs(rs)
                    .imm(off as u32));
            }
            IrTy::Bltz(rs, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x1)
                    .link(false)
                    .bgez(false)
                    .rs(rs)
                    .imm(off as u32));
            }
            IrTy::Bgezal(rs, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x1)
                    .link(true)
                    .bgez(true)
                    .rs(rs)
                    .imm(off as u32));
            }
            IrTy::Bltzal(rs, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x1)
                    .link(true)
                    .bgez(false)
                    .rs(rs)
                    .imm(off as u32));
            }
            IrTy::J(addr) => {
                let addr = self.jump_addr(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x2).target(addr));
            }
            IrTy::Jal(addr) => {
                let addr = self.jump_addr(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x3).target(addr));
            }
            IrTy::Beq(rs, rt, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x4).rs(rs).rt(rt).imm(off as u32));
            }
            IrTy::Bne(rs, rt, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x5).rs(rs).rt(rt).imm(off as u32));
            }
            IrTy::Blez(rs, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x6).rs(rs).imm(off as u32));
            }
            IrTy::Bgtz(rs, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x7).rs(rs).imm(off as u32));
            }
            IrTy::Addi(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x8).rt(rt).rs(rs).imm(val));
            }
            IrTy::Addiu(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x9).rt(rt).rs(rs).imm(val));
            }
            IrTy::Slti(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xa).rt(rt).rs(rs).imm(val));
            }
            IrTy::Sltiu(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xb).rt(rt).rs(rs).imm(val));
            }
            IrTy::Andi(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xc).rt(rt).rs(rs).imm(val));
            }
            IrTy::Ori(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xd).rt(rt).rs(rs).imm(val));
            }
            IrTy::Xori(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0xe).rt(rt).rs(rs).imm(val));
            }
            IrTy::Lui(rt, val) => {
                self.gen_ins(InsBuilder::op(0xf).rt(rt).imm(val));
            }
            IrTy::Mfc0(rt, reg) => {
                self.gen_ins(InsBuilder::op(0x10).cop_op(0).rt(rt).cop_reg(reg));
            }
            IrTy::Mtc0(rt, reg) => {
                self.gen_ins(InsBuilder::op(0x10).cop_op(0x4).rt(rt).cop_reg(reg));
            }
            IrTy::Mfc2(rt, reg) => {
                self.gen_ins(InsBuilder::op(0x12).cop_op(0).rt(rt).cop_reg(reg));
            }
            IrTy::Mtc2(rt, reg) => {
                self.gen_ins(InsBuilder::op(0x12).cop_op(0x4).rt(rt).cop_reg(reg));
            }
            IrTy::Lb(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x20).rt(rt).rs(rs).imm(val));
            }
            IrTy::Lh(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x21).rt(rt).rs(rs).imm(val));
            }
            IrTy::Lwl(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x22).rt(rt).rs(rs).imm(val));
            }
            IrTy::Lw(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x23).rt(rt).rs(rs).imm(val));
            }
            IrTy::Lbu(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x24).rt(rt).rs(rs).imm(val));
            }
            IrTy::Lhu(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x25).rt(rt).rs(rs).imm(val));
            }
            IrTy::Lwr(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x26).rt(rt).rs(rs).imm(val));
            }
            IrTy::Sb(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x28).rt(rt).rs(rs).imm(val));
            }
            IrTy::Sh(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x29).rt(rt).rs(rs).imm(val));
            }
            IrTy::Swl(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x2a).rt(rt).rs(rs).imm(val));
            }
            IrTy::Sw(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x2b).rt(rt).rs(rs).imm(val));
            }
            IrTy::Swr(rt, rs, val) => {
                self.gen_ins(InsBuilder::op(0x2e).rt(rt).rs(rs).imm(val));
            }
            IrTy::Word(val) => {
                self.code.extend_from_slice(&val.to_le_bytes());
            }
            IrTy::HalfWord(val) => {
                self.code.extend_from_slice(&val.to_le_bytes());
            }
            IrTy::Byte(val) => {
                self.code.extend_from_slice(&val.to_le_bytes());
            }
            IrTy::Ascii(ref string) => {
                self.code.extend_from_slice(string.as_bytes());
            }
            IrTy::Nop => {
                self.gen_ins(InsBuilder::special(0x0)
                    .rd(Register(0))
                    .rt(Register(0))
                    .shift(0));
            }
            IrTy::Move(rd, rs) => {
                self.gen_ins(InsBuilder::special(0x21)
                    .rd(rd)
                    .rs(rs)
                    .rt(Register(0)));
            }
            IrTy::Li(reg, val) => {
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
            IrTy::La(reg, label) => {
                let val = self.resolve_label(ir.line, &label)?;
                let (hi, lo) = (val.bit_range(16, 31), val.bit_range(0, 15));
                self.gen_ins(InsBuilder::op(0xf).rt(reg).imm(hi));
                self.gen_ins(InsBuilder::op(0xd).rs(reg).rt(reg).imm(lo));
            }
            IrTy::B(addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x4)
                    .rs(Register(0))
                    .rt(Register(0))
                    .imm(off as u32));
            }
            IrTy::Beqz(reg, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x4)
                    .rs(reg)
                    .rt(Register(0))
                    .imm(off as u32));
            }
            IrTy::Bnez(reg, addr) => {
                let off = self.branch_offset(ir.line, &addr)?;
                self.gen_ins(InsBuilder::op(0x5)
                    .rs(reg)
                    .rt(Register(0))
                    .imm(off as u32));
            }
            IrTy::Label(_) => (),
        }
        Ok(())
    }
}

/// Generate binary machine code from ['Ir'] instructions.
pub fn gen_machine_code<'a>(
    parsed: Vec<ParsedSource<'a>>,
    base: u32
) -> Result<(Vec<u8>, u32), Error> {
    let mut gen = CodeGen::new();
    let mut addr = base;
   
    for s in &parsed {
        for ins in s.text.iter().chain(s.data.iter()) {
            if let IrTy::Label(id) = ins.ty {
                if gen.labels.insert(id, addr).is_some() {
                    return Err(Error::new(
                        ins.line,
                        format!("Label '{}' redeclared", id),  
                    ));
                }
            } else {
                addr += ins.ty.size();
            }
        }
    }

    for s in &parsed {
        for ins in s.text.iter().chain(s.data.iter()) {
            gen.assemble_ir(ins)?;
        }
    }

    let main = match gen.labels.get("main") {
        Some(main) => *main,
        None => {
            return Err(Error::new(0, "No label 'main'"));
        }
    };

    Ok((gen.code, main))
}
