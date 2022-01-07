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
//!

use super::REGISTER_NAMES;
use crate::util::BitExtract;
use std::fmt;

#[derive(Clone, Copy)]
pub struct Opcode(pub(super) u32);

impl Opcode {
    pub fn new(opcode: u32) -> Self {
        Opcode(opcode)
    }

    /// Operation - bits 26..31.
    pub fn op(self) -> u32 {
        self.0.extract_bits(26, 31)
    }

    /// Sub operation - bits 0..5.
    pub fn special(self) -> u32 {
        self.0.extract_bits(0, 5)
    }

    /// Cop operation.
    pub fn cop_op(self) -> u32 {
        self.rs()
    }

    /// Immediate value - bits 0..15.
    pub fn imm(self) -> u32 {
        self.0.extract_bits(0, 15)
    }

    /// Signed immediate value - bits 0..15.
    pub fn signed_imm(self) -> u32 {
        let value = self.0.extract_bits(0, 15) as i16;
        value as u32
    }

    /// Target address - bits 0..25.
    pub fn target(self) -> u32 {
        self.0.extract_bits(0, 25)
    }

    /// Shift value - bits 6..10.
    pub fn shift(self) -> u32 {
        self.0.extract_bits(6, 10)
    }

    /// Destination register - bits 11..15.
    pub fn rd(self) -> u32 {
        self.0.extract_bits(11, 15)
    }

    /// Target register - bits 16..20.
    pub fn rt(self) -> u32 {
        self.0.extract_bits(16, 20)
    }

    /// Source register - bits 21..25.
    pub fn rs(self) -> u32 {
        self.0.extract_bits(21, 25)
    }

    /// Branch if greater or equal zero - BCONDZ needs this to determine the type of branching.
    pub fn bgez(self) -> bool {
        self.0.extract_bit(16) == 1
    }

    /// Set return register on branch - Also used by BCONDZ.
    pub fn set_ra_on_branch(self) -> bool {
        self.0.extract_bits(17, 20) == 0x8
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn reg(reg: u32) -> &'static str {
            REGISTER_NAMES[reg as usize]
        }
        match self.op() {
            0x0 => match self.special() {
                0x0 => write!(
                    f,
                    "sll ${} ${} {}",
                    reg(self.rt()),
                    reg(self.rd()),
                    self.shift()
                ),
                0x2 => write!(
                    f,
                    "srl ${} ${} {}",
                    reg(self.rt()),
                    reg(self.rd()),
                    self.shift()
                ),
                0x3 => write!(
                    f,
                    "sra ${} ${} {}",
                    reg(self.rt()),
                    reg(self.rd()),
                    self.shift()
                ),
                0x4 => write!(
                    f,
                    "sllv ${} ${} ${}",
                    reg(self.rt()),
                    reg(self.rs()),
                    reg(self.rd())
                ),
                0x6 => write!(
                    f,
                    "srlv ${} ${} ${}",
                    reg(self.rt()),
                    reg(self.rs()),
                    reg(self.rd())
                ),
                0x7 => write!(
                    f,
                    "srav ${} ${} ${}",
                    reg(self.rt()),
                    reg(self.rs()),
                    reg(self.rd())
                ),
                0x8 => write!(f, "jr ${}", reg(self.rs())),
                0x9 => write!(f, "jalr ${} ${}", reg(self.rs()), reg(self.rd())),
                0xc => write!(f, "syscall"),
                0xd => write!(f, "break"),
                0x10 => write!(f, "mfhi ${}", reg(self.rd())),
                0x11 => write!(f, "mthi ${}", reg(self.rs())),
                0x12 => write!(f, "mflo ${}", reg(self.rd())),
                0x13 => write!(f, "mtlo ${}", reg(self.rs())),
                0x18 => write!(f, "mult ${} ${}", reg(self.rs()), reg(self.rt())),
                0x19 => write!(f, "multu ${} ${}", reg(self.rs()), reg(self.rt())),
                0x1a => write!(f, "div ${} ${}", reg(self.rs()), reg(self.rt())),
                0x1b => write!(f, "divu ${} ${}", reg(self.rs()), reg(self.rt())),
                0x20 => write!(
                    f,
                    "add ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                0x21 => write!(
                    f,
                    "addu ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                0x22 => write!(
                    f,
                    "sub ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                0x23 => write!(
                    f,
                    "subu ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                0x24 => write!(
                    f,
                    "and ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                0x25 => write!(
                    f,
                    "or ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                0x26 => write!(
                    f,
                    "xor ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                0x27 => write!(
                    f,
                    "nor ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                0x2a => write!(
                    f,
                    "slt ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                0x2b => write!(
                    f,
                    "sltu ${} ${} ${}",
                    reg(self.rs()),
                    reg(self.rt()),
                    reg(self.rd())
                ),
                _ => write!(f, "illegal"),
            },
            0x1 => {
                let op = match (self.set_ra_on_branch(), self.bgez()) {
                    (true, true) => "bgezal",
                    (true, false) => "bltzal",
                    (false, true) => "bgez",
                    (false, false) => "bltz",
                };
                write!(f, "{} ${} {}", op, reg(self.rs()), self.signed_imm())
            }
            0x2 => write!(f, "j {:08x}", self.target()),
            0x3 => write!(f, "jal {:08x}", self.target()),
            0x4 => write!(
                f,
                "beq ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x5 => write!(
                f,
                "bne ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x6 => write!(f, "blez ${} {}", reg(self.rs()), self.signed_imm()),
            0x7 => write!(f, "bgtz ${} {}", reg(self.rs()), self.signed_imm()),
            0x8 => write!(
                f,
                "addi ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x9 => write!(
                f,
                "addiu ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0xa => write!(
                f,
                "slti ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0xb => write!(
                f,
                "sltui ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0xc => write!(
                f,
                "andi ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.imm()
            ),
            0xd => write!(
                f,
                "ori ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.imm()
            ),
            0xe => write!(
                f,
                "xori ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.imm()
            ),
            0xf => write!(f, "lui ${} {}", reg(self.rt()), self.imm()),
            // TODO: Make this better.
            0x10 => write!(f, "cop0"),
            0x11 => write!(f, "cop1"),
            0x12 => write!(f, "cop2"),
            0x13 => write!(f, "cop3"),
            0x20 => write!(
                f,
                "lb ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x21 => write!(
                f,
                "lh ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x22 => write!(
                f,
                "lwl ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x23 => write!(
                f,
                "lw ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x24 => write!(
                f,
                "lbu ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x25 => write!(
                f,
                "lhu ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x26 => write!(
                f,
                "lwr ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x28 => write!(
                f,
                "sb ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x29 => write!(
                f,
                "sh ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x2a => write!(
                f,
                "swl ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x2b => write!(
                f,
                "sw ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
            0x2e => write!(
                f,
                "swr ${} ${} {}",
                reg(self.rs()),
                reg(self.rt()),
                self.signed_imm()
            ),
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
