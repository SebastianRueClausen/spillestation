//! Emulation of the MIPS R3000 used by the original sony playstation.

mod cop0;
mod opcode;

use std::fmt;

use super::memory::{AddrUnit, Bus, Byte, HalfWord, Word};
use cop0::Cop0;
use opcode::Opcode;

#[derive(Copy, Clone)]
struct DelaySlot {
    pub register: u32,
    pub value: u32,
}

pub struct Cpu {
    /// Program Counter.
    pc: u32,
    /// Next operation to be executed.
    next_op: Opcode,
    /// Multiply/divide result.
    // hi: u32,
    // lo: u32,
    /// General purpose registers - http://problemkaputt.de/psx-spx.htm#cpuspecifications
    /// - [R0/zero] - Constant (always 0) (this one isn't a real register).
    /// - [R1/at] - Assembler temporary (destroyed by some pseudo opcodes!).
    /// - [R2-R3/v0-v1] - Subroutine return values, may be changed by subroutines.
    /// - [R4-R7/a0-a3] - Subroutine arguments, may be changed by subroutines.
    /// - [R8-R15/t0-t7] - Temporaries, may be changed by subroutines.
    /// - [R16-R23/s0-s7] - Static variables, must be saved by subs.
    /// - [R24-R25/t8-t9] - Temporaries, may be changed by subroutines.
    /// - [R26-R27/k0-k1] - Reserved for kernel (destroyed by some IRQ handlers!).
    /// - [R28/gp] - Global pointer (rarely used).
    /// - [R29/sp] - Stack pointer.
    /// - [R30/fp(s8)] - Frame Pointer, or 9th Static variable, must be saved.
    /// - [R31/ra] - Return address (used so by JAL,BLTZAL,BGEZAL opcodes).
    registers: [u32; 32],
    pending_load: Option<DelaySlot>,
    /// CPU owns the bus for now.
    bus: Bus,
    cop0: Cop0,
    /// Instruction count for debugging.
    instruction_count: u64,
}

const PC_START_ADDRESS: u32 = 0xbfc00000;

impl Cpu {
    pub fn new(bus: Bus) -> Self {
        // Reset values of the CPU.
        Cpu {
            pc: PC_START_ADDRESS,
            next_op: Opcode::new(0x0),
            // hi: 0x0,
            // lo: 0x0,
            registers: [0x0; 32],
            pending_load: None,
            bus,
            cop0: Cop0::new(),
            instruction_count: 0,
        }
    }

    /// Get value of register at index.
    fn read_reg(&self, index: u32) -> u32 {
        self.registers[index as usize]
    }

    /// Set register at index.
    fn set_reg(&mut self, index: u32, value: u32) {
        self.registers[index as usize] = value;
        self.registers[0] = 0;
    }

    /// Load address from bus.
    fn load<T: AddrUnit>(&self, address: u32) -> u32 {
        self.bus.load::<T>(address)
    }

    /// Store address to bus.
    fn store<T: AddrUnit>(&mut self, address: u32, value: u32) {
        if !self.cop0.cache_is_isolated() {
            self.bus.store::<T>(address, value);
        } else {
            // TODO: Write to cache.
        }
    }

    /// Add pending load. If there's already one pending, fetch it
    fn add_pending_load(&mut self, register: u32, value: u32) {
        if let Some(load) = self.pending_load {
            self.set_reg(load.register, load.value);
        }
        self.pending_load = Some(DelaySlot { register, value });
    }

    /// Do pending load, if any.
    fn fetch_pending_load(&mut self) {
        if let Some(load) = self.pending_load {
            self.set_reg(load.register, load.value);
            self.pending_load = None;
        }
    }

    /// Branch to relative offset.
    fn branch(&mut self, offset: u32) {
        // Offset is shifted 2 bites since PC addresses must be 32-bit aligned.
        // The reason we subtract 4, is to compensate adding 4 in next fetch.
        self.pc = self.pc.wrapping_add(offset << 2).wrapping_sub(4);
    }

    /// Fetch and execute next instruction.
    pub fn fetch_and_exec(&mut self) {
        if self.instruction_count % 100 == 0 {
            println!("instruction count: {}", self.instruction_count);
        }

        self.instruction_count += 1;
        
        let op = self.next_op;
        self.next_op = Opcode::new(self.load::<Word>(self.pc));
        self.pc = self.pc.wrapping_add(4);
        self.exec(op);
    }

    /// Execute opcode.
    fn exec(&mut self, opcode: Opcode) {
        match opcode.op() {
            0x0 => match opcode.sub_op() {
                0x0 => self.op_sll(opcode),
                0x8 => self.op_jr(opcode),
                0x9 => self.op_jalr(opcode),
                0x24 => self.op_and(opcode),
                0x25 => self.op_or(opcode),
                0x20 => self.op_add(opcode),
                0x21 => self.op_addu(opcode),
                0x2b => self.op_sltu(opcode),
                _ => panic!("Unexpected subop {}", opcode),
            },
            0x2 => self.op_j(opcode),
            0x3 => self.op_jal(opcode),
            0x4 => self.op_beq(opcode),
            0x5 => self.op_bne(opcode),
            0x6 => self.op_blez(opcode),
            0x7 => self.op_bgtz(opcode),
            0x8 => self.op_addi(opcode),
            0x9 => self.op_addiu(opcode),
            0x10 => self.op_cop0(opcode),
            0xc => self.op_andi(opcode),
            0xd => self.op_ori(opcode),
            0xf => self.op_lui(opcode),
            0x20 => self.op_lb(opcode),
            0x21 => self.op_lh(opcode),
            0x23 => self.op_lw(opcode),
            0x24 => self.op_lbu(opcode),
            0x28 => self.op_sb(opcode),
            0x29 => self.op_sh(opcode),
            0x2b => self.op_sw(opcode),
            _ => panic!("Unexpected op {}", opcode),
        }
    }
}

/// Instructions
impl Cpu {
    /// [SLL] - Shift left logical.
    fn op_sll(&mut self, op: Opcode) {
        let value = self.read_reg(op.target_reg()) << op.shift();
        self.fetch_pending_load();
        self.set_reg(op.destination_reg(), value);
    }

    /// [AND] - Bitwise and
    fn op_and(&mut self, op: Opcode) {
        let value = self.read_reg(op.source_reg()) & self.read_reg(op.target_reg());
        self.fetch_pending_load();
        self.set_reg(op.destination_reg(), value);
    }

    /// [OR] - Birwise or.
    fn op_or(&mut self, op: Opcode) {
        let value = self.read_reg(op.source_reg()) | self.read_reg(op.target_reg());
        self.fetch_pending_load();
        self.set_reg(op.destination_reg(), value);
    }

    /// [JAL] - Jump and link.
    fn op_jal(&mut self, op: Opcode) {
        // Store PC in return register
        self.set_reg(31, self.pc);
        // J fetches pending load.
        self.op_j(op); 
    }

    /// [JALR] - Jump and link register.
    fn op_jalr(&mut self, op: Opcode) {
        self.set_reg(op.destination_reg(), self.pc);
        self.pc = self.read_reg(op.source_reg());
        self.fetch_pending_load();
    }

    /// [JR] - Jump register.
    fn op_jr(&mut self, op: Opcode) {
        self.pc = self.read_reg(op.source_reg());
        self.fetch_pending_load();
    }

    /// [BEQ] - Branch if equal.
    fn op_beq(&mut self, op: Opcode) {
        if self.read_reg(op.source_reg()) == self.read_reg(op.target_reg()) {
            self.branch(op.signed_imm());
        }
        self.fetch_pending_load();
    }

    /// [SLTU] - Less than unsigned.
    fn op_sltu(&mut self, op: Opcode) {
        let value = self.read_reg(op.source_reg()) < self.read_reg(op.target_reg());
        self.fetch_pending_load();
        self.set_reg(op.destination_reg(), value as u32);
    }

    /// [ADD] - Add signed.
    /// Throws on overflow.
    fn op_add(&mut self, op: Opcode) {
        let value = match self.read_reg(op.source_reg()).checked_add(self.read_reg(op.target_reg())) {
            Some(value) => value as u32, 
            None => panic!("ADD: overflow"),
        };
        self.fetch_pending_load();
        self.set_reg(op.destination_reg(), value);
    }

    /// [ADDU] - Add unsigned.
    fn op_addu(&mut self, op: Opcode) {
        let value = self.read_reg(op.source_reg()).wrapping_add(self.read_reg(op.target_reg()));
        self.fetch_pending_load();
        self.set_reg(op.destination_reg(), value);
    }

    /// [J] - Jump.
    fn op_j(&mut self, op: Opcode) {
        self.pc = (self.pc & 0xf0000000) | (op.target() << 2);
        self.fetch_pending_load();
    }

    /// [ADDUI] - Add immediate unsigned.
    /// Actually adding a signed int to target register, not unsigned.
    /// Unsigned in this case just means wrapping on overflow.
    fn op_addiu(&mut self, op: Opcode) {
        let value = self.read_reg(op.source_reg()).wrapping_add(op.signed_imm());
        self.fetch_pending_load();
        self.set_reg(op.target_reg(), value);
    }

    /// [BLEZ] - Branch if less than or equal to zero
    fn op_blez(&mut self, op: Opcode) {
        let value = self.read_reg(op.source_reg()) as i32; 
        if value <= 0 {
            self.branch(op.signed_imm());
        }
        self.fetch_pending_load();
    }

    /// [BGTZ] - Branch if greater than zero
    fn op_bgtz(&mut self, op: Opcode) {
        let value = self.read_reg(op.source_reg()) as i32; 
        if value > 0 {
            self.branch(op.signed_imm());
        }
        self.fetch_pending_load();
    }

    /// [ADDI] - Add immediate signed.
    /// Same as ADDUI but throw exception on overflow.
    fn op_addi(&mut self, op: Opcode) {
        let source = self.read_reg(op.source_reg()) as i32;
        let value = match source.checked_add(op.signed_imm() as i32) {
            Some(value) => value,
            // This should of course be an exception.
            None => panic!("ADDI: overflow"),
        };
        self.fetch_pending_load();
        self.set_reg(op.target_reg(), value as u32);
    }

    /// [BNE] - Branch if not equal.
    fn op_bne(&mut self, op: Opcode) {
        if self.read_reg(op.source_reg()) != self.read_reg(op.target_reg()) {
            self.branch(op.signed_imm());
        }
        self.fetch_pending_load();
    }

    /// [ANDI] - Bitwise and immediate.
    fn op_andi(&mut self, op: Opcode) {
        let value = self.read_reg(op.source_reg()) & op.imm();
        self.fetch_pending_load();
        self.set_reg(op.target_reg(), value);
    }

    /// [ORI] - Or immediate.
    fn op_ori(&mut self, op: Opcode) {
        let value = self.read_reg(op.source_reg()) | op.imm();
        self.fetch_pending_load();
        self.set_reg(op.target_reg(), value);
    }

    /// [LUI] - Load upper immediate.
    fn op_lui(&mut self, op: Opcode) {
        self.set_reg(op.target_reg(), op.imm() << 16);
        self.fetch_pending_load();
    }

    /// [LW] - Load word.
    fn op_lw(&mut self, op: Opcode) {
        let address = self.read_reg(op.source_reg()).wrapping_add(op.signed_imm());
        self.add_pending_load(op.target_reg(), self.load::<Word>(address));
    }

    /// [LH] - Load half word.
    fn op_lh(&mut self, op: Opcode) {
        let address = self.read_reg(op.source_reg()).wrapping_add(op.signed_imm());
        let value = self.load::<HalfWord>(address) as i16;
        self.add_pending_load(op.target_reg(), value as u32);
    }

    /// [LB] - Load byte.
    fn op_lb(&mut self, op: Opcode) {
        let address = self.read_reg(op.source_reg()).wrapping_add(op.signed_imm());
        let value = self.load::<Byte>(address) as i8;
        self.add_pending_load(op.target_reg(), value as u32);
    }

    /// [LBU] - Load byte unsigned.
    fn op_lbu(&mut self, op: Opcode) {
        let address = self.read_reg(op.source_reg()).wrapping_add(op.signed_imm());
        let value = self.load::<Byte>(address);
        self.add_pending_load(op.target_reg(), value);
    }

    /// [SB] - Store byte.
    /// Store byte from target register at address from source register + immediate value.
    fn op_sb(&mut self, op: Opcode) {
        let address = self.read_reg(op.source_reg()).wrapping_add(op.signed_imm());
        let value = self.read_reg(op.target_reg());
        self.fetch_pending_load();
        self.store::<Byte>(address, value);
    }

    /// [SH] - Store half word.
    /// Store half word from target register at address from source register + immediate value.
    fn op_sh(&mut self, op: Opcode) {
        let address = self.read_reg(op.source_reg()).wrapping_add(op.signed_imm());
        let value = self.read_reg(op.target_reg());
        self.fetch_pending_load();
        self.store::<HalfWord>(address, value);
    }

    /// [SW] - Store word.
    /// Store word from target register at address from source register + signed immediate value.
    fn op_sw(&mut self, op: Opcode) {
        let address = self.read_reg(op.source_reg()).wrapping_add(op.signed_imm());
        let value = self.read_reg(op.target_reg());
        self.fetch_pending_load();
        self.store::<Word>(address, value);
    }

    /// [COP0] - Coprocessor0 instruction.
    fn op_cop0(&mut self, op: Opcode) {
        match op.cop0_op() {
            // [MFC0] - Move from Co-Processor0.
            0x0 => {
                let value = self.cop0.read_reg(op.destination_reg());
                self.add_pending_load(op.destination_reg(), value);
            }
            // [MTC0] - Move to Co-Processor0.
            0x4 => {
                self.cop0.set_reg(op.destination_reg(), self.read_reg(op.target_reg()));
                // TODO Break point flags things.
            }
            // [RFE] - Restore from exception.
            0xf => {}
            // TODO: This should cause an exception.
            _ => panic!("Invalid COP0 instruction {:08x}", op.cop0_op()),
        }
    }
}

impl fmt::Display for Cpu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "pc: {}, registers: {:?}", self.pc, self.registers)
    }
}
