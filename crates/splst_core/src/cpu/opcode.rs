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

use super::REGISTER_NAMES;
use splst_util::Bit;

use std::fmt;

/// The index of a register. Used for better type safety.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct RegIdx(pub u8);

impl RegIdx {
    pub fn new(idx: u32) -> Self {
        Self(idx as u8)
    }

    #[allow(dead_code)]
    pub const ZERO: Self = Self(0);

    #[allow(dead_code)]
    pub const AT: Self = Self(1);

    #[allow(dead_code)]
    pub const V0: Self = Self(2);

    #[allow(dead_code)]
    pub const V1: Self = Self(3);

    #[allow(dead_code)]
    pub const A0: Self = Self(4);

    #[allow(dead_code)]
    pub const A1: Self = Self(5);

    #[allow(dead_code)]
    pub const A2: Self = Self(6);

    #[allow(dead_code)]
    pub const A3: Self = Self(7);

    #[allow(dead_code)]
    pub const T0: Self = Self(8);

    #[allow(dead_code)]
    pub const T1: Self = Self(9);

    #[allow(dead_code)]
    pub const T2: Self = Self(10);

    #[allow(dead_code)]
    pub const T3: Self = Self(11);

    #[allow(dead_code)]
    pub const T4: Self = Self(12);

    #[allow(dead_code)]
    pub const T5: Self = Self(13);

    #[allow(dead_code)]
    pub const T6: Self = Self(14);

    #[allow(dead_code)]
    pub const T7: Self = Self(15);

    #[allow(dead_code)]
    pub const S0: Self = Self(16);

    #[allow(dead_code)]
    pub const S1: Self = Self(17);

    #[allow(dead_code)]
    pub const S2: Self = Self(18);

    #[allow(dead_code)]
    pub const S3: Self = Self(19);

    #[allow(dead_code)]
    pub const S4: Self = Self(20);

    #[allow(dead_code)]
    pub const S5: Self = Self(21);

    #[allow(dead_code)]
    pub const S6: Self = Self(22);

    #[allow(dead_code)]
    pub const S7: Self = Self(23);

    #[allow(dead_code)]
    pub const T8: Self = Self(24);

    #[allow(dead_code)]
    pub const T9: Self = Self(25);

    #[allow(dead_code)]
    pub const K0: Self = Self(26);

    #[allow(dead_code)]
    pub const K1: Self = Self(27);

    #[allow(dead_code)]
    pub const GP: Self = Self(28);

    #[allow(dead_code)]
    pub const SP: Self = Self(29);

    #[allow(dead_code)]
    pub const FP: Self = Self(30);

    #[allow(dead_code)]
    pub const RA: Self = Self(31);
}

impl fmt::Display for RegIdx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "${}", REGISTER_NAMES[self.0 as usize])
    }
}


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
    pub fn rd(self) -> RegIdx {
        RegIdx::new(self.0.bit_range(11, 15))
    }

    /// Target register.
    pub fn rt(self) -> RegIdx {
        RegIdx::new(self.0.bit_range(16, 20))
    }

    /// Source register.
    pub fn rs(self) -> RegIdx {
        RegIdx::new(self.0.bit_range(21, 25))
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
