//! Decoding of MIPS R3000 Opcodes.
//!
//! All Opcodes are incoded in 32 bites, and are ofcourse little endian.
//!
//! There are three main opcode layouts:
//! - [immediate]:
//!     - 6-bit op.
//!     - 5-bit source register.
//!     - 5-bit target register.
//!     - 16-bit immediate value.
//!
//! - [jump]:
//!     - 6-bit op.
//!     - 26-bit target address.
//!
//! - [register]:
//!     - 6-bit op.
//!     - 5-bit source register.
//!     - 5-bit target register.
//!     - 5-bit destination register.
//!     - 5-bit shift value.
//!     - 6-bit function field.
//!

use std::fmt;

#[derive(Clone, Copy)]
pub struct Opcode(u32);

impl Opcode {
    pub fn new(opcode: u32) -> Self {
        Opcode(opcode)
    }

    /// Operation - bits 26..31.
    pub fn op(self) -> u32 {
        self.0 >> 26
    }

    /// Sub operation - bits 0..5.
    pub fn special(self) -> u32 {
        self.0 & 0x3f
    }

    /// Cop0 operation.
    /// This is the same as source reg, so this is just for clarity.
    pub fn cop0_op(self) -> u32 {
        self.rs()
    }

    /// Immediate value - bits 0..16.
    pub fn imm(self) -> u32 {
        self.0 & 0xffff
    }

    /// Signed immediate value - bits 0..16.
    pub fn signed_imm(self) -> u32 {
        let value = (self.0 & 0xffff) as i16;
        value as u32
    }

    /// Target address - bits 0..25.
    pub fn target(self) -> u32 {
        self.0 & 0x3ffffff
    }

    /// Shift value - bits 6..10.
    pub fn shift(self) -> u32 {
        (self.0 >> 6) & 0x1f
    }

    /// Destination register - bits 11..15.
    pub fn rd(self) -> u32 {
        (self.0 >> 11) & 0x1f
    }

    /// Target register - bits 16..20.
    pub fn rt(self) -> u32 {
        (self.0 >> 16) & 0x1f
    }

    /// Source register - bits 21..25.
    pub fn rs(self) -> u32 {
        (self.0 >> 21) & 0x1f
    }

    /// Branch if greater or equal zero - BCONDZ needs this to determine the type of branching.
    pub fn bgez(self) -> u32 {
        (self.0 >> 16) & 0x1
    }

    /// Set return register on branch - Also used by BCONDZ.
    pub fn set_ra_on_branch(self) -> bool {
        (self.0 >> 17) & 0xf == 0x8
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let op = match self.op() {
            0x0 => match self.special() {
                0x0 => "SLL",
                0x2 => "SRL",
                0x3 => "SRA",
                0x4 => "SLLV",
                0x6 => "SRLV",
                0x7 => "SRAV",
                0x8 => "JR",
                0x9 => "JALR",
                0xc => "SYSCALL",
                0xd => "BREAK",
                0x10 => "MFHI",
                0x11 => "MTHI",
                0x12 => "MFLO",
                0x13 => "MTLO",
                0x18 => "MULT",
                0x19 => "MULTU",
                0x1a => "DIV",
                0x1b => "DIVU",
                0x20 => "ADD",
                0x21 => "ADDU",
                0x22 => "SUB",
                0x23 => "SUBU",
                0x24 => "AND",
                0x25 => "OR",
                0x26 => "XOR",
                0x27 => "NOR",
                0x2a => "SLT",
                0x2b => "SLTU",
                _ => "Illegal",
            },
            0x1 => "BCONDZ",
            0x2 => "J",
            0x3 => "JAL",
            0x4 => "BEQ",
            0x5 => "BNE",
            0x6 => "BLEZ",
            0x7 => "BGTZ",
            0x8 => "ADDI",
            0x9 => "ADDIU",
            0xa => "SLTI",
            0xb => "SLTIU",
            0xc => "ANDI",
            0xd => "ORI",
            0xe => "XORI",
            0xf => "LUI",
            0x10 => "COP0",
            0x11 => "COP1",
            0x12 => "COP2",
            0x13 => "COP3",
            0x20 => "LB",
            0x21 => "LH",
            0x22 => "LWL",
            0x23 => "LW",
            0x24 => "LBU",
            0x25 => "LHU",
            0x26 => "LWR",
            0x28 => "SB",
            0x29 => "SH",
            0x2a => "SWL",
            0x2b => "SW",
            0x2e => "SWR",
            0x30 => "LWC0",
            0x31 => "LWC1",
            0x32 => "LWC2",
            0x33 => "LWC3",
            0x38 => "SWC0",
            0x39 => "SWC1",
            0x3a => "SWC2",
            0x3b => "SWC3",
            _ => "Illegal",
        };
        // TODO: Add more info.
        write!(f,"op: {}, imm: {:0x}, shift: {:0x}, target: {}, source: {}, destination: {}",
            op, self.imm(), self.shift(), self.rt(), self.rs(), self.rd()
        )
    }
}
