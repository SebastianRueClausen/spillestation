//! Decoding of MIPS R3000 Opcodes.
//!
//! All Opcodes are incoded in 32 bits.
//!
//! There are three main opcode layouts:
//! - Immediate
//!     - 6-bit op.
//!     - 5-bit source register.
//!     - 5-bit target register.
//!     - 16-bit immediate value.
//!
//! - Jump
//!     - 6-bit op.
//!     - 26-bit target address.
//!
//! - Register
//!     - 6-bit op.
//!     - 5-bit source register.
//!     - 5-bit target register.
//!     - 5-bit destination register.
//!     - 5-bit shift value.
//!     - 6-bit function field.

use splst_util::Bit;
use splst_asm::Register;

use std::fmt;

#[derive(Clone, Copy)]
pub struct Opcode(pub(super) u32);

impl Opcode {
    pub fn new(opcode: u32) -> Self {
        Opcode(opcode)
    }

    /// Operation.
    pub fn op(self) -> u32 {
        self.0.bit_range(26, 31)
    }

    /// Sub operation / function.
    pub fn special(self) -> u32 {
        self.0.bit_range(0, 5)
    }

    /// COP operation.
    pub fn cop_op(self) -> u32 {
        self.rs().0.into()
    }

    /// Immediate value.
    pub fn imm(self) -> u32 {
        self.0.bit_range(0, 15)
    }

    /// Signed immediate value.
    pub fn signed_imm(self) -> u32 {
        let value = self.0.bit_range(0, 15) as i16;
        value as u32
    }

    /// Target address used for branch instructions.
    pub fn target(self) -> u32 {
        self.0.bit_range(0, 25)
    }

    pub fn shift(self) -> u32 {
        self.0.bit_range(6, 10)
    }

    /// Destination register.
    pub fn rd(self) -> Register {
        Register::from(self.0.bit_range(11, 15))
    }

    /// Target register.
    pub fn rt(self) -> Register {
        Register::from(self.0.bit_range(16, 20))
    }

    /// Source register.
    pub fn rs(self) -> Register {
        Register::from(self.0.bit_range(21, 25))
    }

    /// Branch if greater or equal zero. Used by BCONDZ to determine the type of branching.
    pub fn bgez(self) -> bool {
        self.0.bit(16)
    }

    /// Set return register on branch. Used by BCONDZ.
    pub fn update_ra_on_branch(self) -> bool {
        self.0.bit_range(17, 20) == 0x8
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.op() {
            0x0 => match self.special() {
                0x0 => write!(f, "sll {} {} {}", self.rd(), self.rt(), self.shift()),
                0x2 => write!(f, "srl {} {} {}", self.rd(), self.rt(), self.shift()),
                0x3 => write!(f, "sra {} {} {}", self.rd(), self.rt(), self.shift()),
                0x4 => write!(f, "sllv {} {} {}", self.rd(), self.rt(), self.rs()),
                0x6 => write!(f, "srlv {} {} {}", self.rd(), self.rt(), self.rs()),
                0x7 => write!(f, "srav {} {} {}", self.rd(), self.rt(), self.rs()),
                0x8 => write!(f, "jr {}", self.rs()),
                0x9 => write!(f, "jalr {} {}", self.rd(), self.rs()),
                0xc => write!(f, "syscall"),
                0xd => write!(f, "break"),
                0x10 => write!(f, "mfhi {}", self.rd()),
                0x11 => write!(f, "mthi {}", self.rs()),
                0x12 => write!(f, "mflo {}", self.rd()),
                0x13 => write!(f, "mtlo {}", self.rs()),
                0x18 => write!(f, "mult {} {}", self.rs(), self.rt()),
                0x19 => write!(f, "multu {} {}", self.rs(), self.rt()),
                0x1a => write!(f, "div {} {}", self.rs(), self.rt()),
                0x1b => write!(f, "divu {} {}", self.rs(), self.rt()),
                0x20 => write!(f, "add {} {} {}", self.rd(), self.rs(), self.rt()),
                0x21 => write!(f, "addu {} {} {}", self.rd(), self.rs(), self.rt()),
                0x22 => write!(f, "sub {} {} {}", self.rd(), self.rs(), self.rt()),
                0x23 => write!(f, "subu {} {} {}", self.rd(), self.rs(), self.rt()),
                0x24 => write!(f, "and {} {} {}", self.rd(), self.rs(), self.rt()),
                0x25 => write!(f, "or {} {} {}", self.rd(), self.rs(), self.rt()),
                0x26 => write!(f, "xor {} {} {}", self.rd(), self.rs(), self.rt()),
                0x27 => write!(f, "nor {} {} {}", self.rd(), self.rs(), self.rt()),
                0x2a => write!(f, "slt {} {} {}", self.rd(), self.rs(), self.rt()),
                0x2b => write!(f, "sltu {} {} {}", self.rd(), self.rs(), self.rt()),
                _ => write!(f, "illegal"),
            },
            0x1 => {
                let op = match (self.update_ra_on_branch(), self.bgez()) {
                    (true, true) => "bgezal",
                    (true, false) => "bltzal",
                    (false, true) => "bgez",
                    (false, false) => "bltz",
                };
                write!(f, "{} {} {}", op, self.rs(), self.signed_imm())
            }
            0x2 => write!(f, "j {:08x}", self.target()),
            0x3 => write!(f, "jal {:08x}", self.target()),
            0x4 => write!(f, "beq {} {} {}", self.rs(), self.rt(), self.signed_imm()),
            0x5 => write!(f, "bne {} {} {}", self.rs(), self.rt(), self.signed_imm()),
            0x6 => write!(f, "blez {} {}", self.rs(), self.signed_imm()),
            0x7 => write!(f, "bgtz {} {}", self.rs(), self.signed_imm()),
            0x8 => write!(f, "addi {} {} {}", self.rs(), self.rt(), self.signed_imm()),
            0x9 => write!(f, "addiu {} {} {}", self.rs(), self.rt(), self.signed_imm()),
            0xa => write!(f, "slti {} {} {}", self.rs(), self.rt(), self.signed_imm()),
            0xb => write!(f, "sltui {} {} {}", self.rs(), self.rt(), self.signed_imm()),
            0xc => write!(f, "andi {} {} {}", self.rs(), self.rt(), self.imm()),
            0xd => write!(f, "ori {} {} {}", self.rs(), self.rt(), self.imm()),
            0xe => write!(f, "xori {} {} {}", self.rs(), self.rt(), self.imm()),
            0xf => write!(f, "lui {} {}", self.rt(), self.imm()),
            // TODO: Make this better.
            0x10 => write!(f, "cop0"),
            0x11 => write!(f, "cop1"),
            0x12 => write!(f, "cop2"),
            0x13 => write!(f, "cop3"),

            0x20 => write!(f, "lb {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x21 => write!(f, "lh {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x22 => write!(f, "lwl {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x23 => write!(f, "lw {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x24 => write!(f, "lbu {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x25 => write!(f, "lhu {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x26 => write!(f, "lwr {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x28 => write!(f, "sb {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x29 => write!(f, "sh {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x2a => write!(f, "swl {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x2b => write!(f, "sw {} {}({})", self.rt(), self.signed_imm(), self.rs()),
            0x2e => write!(f, "swr {} {}({})", self.rt(), self.signed_imm(), self.rs()),

            0x30 => write!(f, "lwc0"),
            0x31 => write!(f, "lwc1"),
            0x32 => write!(f, "lwc2"),
            0x33 => write!(f, "lwc3"),
            0x38 => write!(f, "swc0"),
            0x39 => write!(f, "swc1"),
            0x3a => write!(f, "swc2"),
            0x3b => write!(f, "swc3"),
            _ => write!(f, "illegal"),
        }
    }
}
