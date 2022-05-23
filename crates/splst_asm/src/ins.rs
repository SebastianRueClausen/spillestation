use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Register(pub u8);

impl From<u32> for Register {
    fn from(val: u32) -> Self {
        Register(val as u8)
    }
}

impl From<u8> for Register {
    fn from(val: u8) -> Self {
        Register(val)
    }
}

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const NAMES: [&str; 32] = [
            "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5",
            "t6", "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1",
            "gp", "sp", "fp", "ra",
        ];
        f.write_str(NAMES[self.0 as usize])
    }
}

impl Register {
    pub const ZERO: Register = Register(0);
    pub const AT: Register = Register(1);
    pub const V0: Register = Register(2);
    pub const V1: Register = Register(3);
    pub const A0: Register = Register(4);
    pub const A1: Register = Register(5);
    pub const A2: Register = Register(6);
    pub const A3: Register = Register(7);
    pub const T0: Register = Register(8);
    pub const T1: Register = Register(9);
    pub const T2: Register = Register(10);
    pub const T3: Register = Register(11);
    pub const T4: Register = Register(12);
    pub const T5: Register = Register(13);
    pub const T6: Register = Register(14);
    pub const T7: Register = Register(15);
    pub const S0: Register = Register(16);
    pub const S1: Register = Register(17);
    pub const S2: Register = Register(18);
    pub const S3: Register = Register(19);
    pub const S4: Register = Register(20);
    pub const S5: Register = Register(21);
    pub const S6: Register = Register(22);
    pub const S7: Register = Register(23);
    pub const T8: Register = Register(24);
    pub const T9: Register = Register(25);
    pub const K0: Register = Register(26);
    pub const K1: Register = Register(27);
    pub const GP: Register = Register(28);
    pub const SP: Register = Register(29);
    pub const FP: Register = Register(30);
    pub const RA: Register = Register(31);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Directive {
    Text,
    Data,
    Word,
    HalfWord,
    Byte,
    Ascii,
    /// Character array with a terminating '\0'.
    Asciiz,
}

/// Should maybe be called Address. It represents an address in memory,
/// given by either an absolute address or a Address reference.
#[derive(Clone, Copy)]
pub enum Address<'a> {
    Label(&'a str),
    Abs(u32),
}

#[derive(Clone)]
pub enum InsTy<'a> {
    Sll(Register, Register, u32),
    Srl(Register, Register, u32),
    Sra(Register, Register, u32),
    Sllv(Register, Register, Register),
    Srlv(Register, Register, Register),
    Srav(Register, Register, Register),
    Jr(Register),
    Jalr(Register, Register),
    Syscall(u32),
    Break(u32),
    Mfhi(Register),
    Mthi(Register),
    Mflo(Register),
    Mtlo(Register),
    Mult(Register, Register),
    Multu(Register, Register),
    Div(Register, Register),
    Divu(Register, Register),
    Add(Register, Register, Register),
    Addu(Register, Register, Register),
    Sub(Register, Register, Register),
    Subu(Register, Register, Register),
    And(Register, Register, Register),
    Or(Register, Register, Register),
    Xor(Register, Register, Register),
    Nor(Register, Register, Register),
    Slt(Register, Register, Register),
    Sltu(Register, Register, Register),
    Bgez(Register, Address<'a>),
    Bltz(Register, Address<'a>),
    Bgezal(Register, Address<'a>),
    Bltzal(Register, Address<'a>),
    J(Address<'a>),
    Jal(Address<'a>),
    Beq(Register, Register, Address<'a>),
    Bne(Register, Register, Address<'a>),
    Blez(Register, Address<'a>),
    Bgtz(Register, Address<'a>),
    Addi(Register, Register, u32),
    Addiu(Register, Register, u32),
    Slti(Register, Register, u32),
    Sltiu(Register, Register, u32),
    Andi(Register, Register, u32),
    Ori(Register, Register, u32),
    Xori(Register, Register, u32),
    Lui(Register, u32),
    Lb(Register, Register, u32),
    Lh(Register, Register, u32),
    Lwl(Register, Register, u32),
    Lw(Register, Register, u32),
    Lbu(Register, Register, u32),
    Lhu(Register, Register, u32),
    Lwr(Register, Register, u32),
    Sb(Register, Register, u32),
    Sh(Register, Register, u32),
    Swl(Register, Register, u32),
    Sw(Register, Register, u32),
    Swr(Register, Register, u32),

    Mfc0(Register, u32),
    Mtc0(Register, u32),

    Mfc2(Register, u32),
    Mtc2(Register, u32),

    /// A Address in memory. Doesn't take space in the binary.
    Label(&'a str),

    // Data.
    Word(u32),
    HalfWord(u16),
    Byte(u8),
    Ascii(String),

    // Pseudo instructions.
    Nop,
    Move(Register, Register),
    Li(Register, u32),
    La(Register, Address<'a>),
    B(Address<'a>),
    Beqz(Register, Address<'a>),
    Bnez(Register, Address<'a>),
}

impl<'a> InsTy<'a> {
    pub fn size(&self) -> u32 {
        match self {
            InsTy::Label(..) => 0,
            InsTy::La(..) => 8,
            InsTy::Li(_, val) => {
                let mut b = 0;
                if val & 0xffff_0000 != 0 {
                    b += 4;
                }
                if *val & 0xffff != 0 || *val == 0 {
                    b += 4;
                }
                b
            }
            _ => 4,
        }
    }
}

/// An immediate representation of assembly code. Since this assembler is multi-pass, ie. it's
/// possible to reference Addresss out of lexical order, the code has to be represented in some way
/// between parsing and code generation when symbols are getting resolved.
#[derive(Clone)]
pub struct Ins<'a> {
    pub ty: InsTy<'a>,
    /// The line of the source file containing the code.
    pub line: usize,
}

impl<'a> Ins<'a> {
    pub fn new(line: usize, ty: InsTy<'a>) -> Self {
        Self { ty, line }
    }
}
