#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Register(pub u8);

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
/// given by either an absolute address or a label reference.
#[derive(Clone, Copy)]
pub enum Label<'a> {
    Label(&'a str),
    Abs(u32),
}

#[derive(Clone)]
pub enum IrTy<'a> {
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
    Bgez(Register, Label<'a>),
    Bltz(Register, Label<'a>),
    Bgezal(Register, Label<'a>),
    Bltzal(Register, Label<'a>),
    J(Label<'a>),
    Jal(Label<'a>),
    Beq(Register, Register, Label<'a>),
    Bne(Register, Register, Label<'a>),
    Blez(Register, Label<'a>),
    Bgtz(Register, Label<'a>),
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

    /// A label in memory. Doesn't take space in the binary.
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
    La(Register, Label<'a>),
    B(Label<'a>),
    Beqz(Register, Label<'a>),
    Bnez(Register, Label<'a>),
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

/// An immediate representation of assembly code. Since this assembler is multi-pass, ie. it's
/// possible to reference labels out of lexical order, the code has to be represented in some way
/// between parsing and code generation when symbols are getting resolved.
#[derive(Clone)]
pub struct Ir<'a> {
    pub ty: IrTy<'a>,
    /// The line of the source file containing the code.
    pub line: usize,
}

impl<'a> Ir<'a> {
    pub fn new(line: usize, ty: IrTy<'a>) -> Self {
        Self { ty, line }
    }
}
