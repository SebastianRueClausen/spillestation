//! Emulation of the MIPS R3000 used by the original Sony Playstation.

mod cop0;
pub mod irq;
pub mod opcode;

use super::memory::{AddrUnit, Bus, Byte, HalfWord, Word, bios::Bios};
use cop0::{Cop0, Exception};

pub use opcode::Opcode;
pub use irq::{IrqState, Irq};

/// Used to store pending loads from the Bus.
#[derive(Default, Copy, Clone)]
struct DelaySlot {
    pub register: u32,
    pub value: u32,
}

#[derive(Default, Copy, Clone)]
struct CacheLine {
    pub tag: u32,
    pub data: u32,
}

pub struct Cpu {
    /// At the start of each cycle, this points to the last instruction which was executed. During
    /// the cycle, it points to the current instruction being executed.
    last_pc: u32,
    /// This points to the instruction about to be executed at the start of each
    /// cycle. During the cycle it points at the next instruction.
    pub pc: u32,
    /// Always one step ahead of PC. Used to simulate branch delay.
    pub next_pc: u32,
    /// Set if instruction is in branch delay slot.
    in_branch_delay: bool,
    /// This is set if the last instruction branched.
    branched: bool,
    /// Multiply/divide result.
    pub hi: u32,
    pub lo: u32,
    /// The cycle number MUL/DIV result is ready.
    hi_lo_ready: u64,
    /// General purpose registers.
    /// - 0 - Always 0.
    /// - 1 - Assembler temporary.
    /// - 2..3 - Subroutine return values.
    /// - 4..7 - Subroutine arguments.
    /// - 8..15 - Temporaries.
    /// - 16..23 - Static variables.
    /// - 24..25 - Temporaries.
    /// - 26..27 - Reserved for kernel.
    /// - 28 - Global pointer.
    /// - 29 - Stack pointer.
    /// - 30 - Frame pointer, or static variable.
    /// - 31 - Return address.
    pub registers: [u32; 32],
    /// This takes advantage of the fact that the zero register always is zero. This means no
    /// branching is required to check of there is something in the load slot.
    load_slot: DelaySlot,
    /// Instruction cache.
    icache: Box<[CacheLine; 1024]>,
    // TODO: Some fields of the BUS is accessed a lot, which is causeing cache misses.
    bus: Bus,
    cop0: Cop0,
}

const PC_START_ADDRESS: u32 = 0xbfc00000;

impl Cpu {
    pub fn new(bios: Bios) -> Box<Self> {
        // Reset values of the CPU.
        Box::new(Cpu {
            last_pc: 0x0,
            pc: PC_START_ADDRESS,
            next_pc: PC_START_ADDRESS + 4,
            in_branch_delay: false,
            branched: false,
            hi: 0x0,
            lo: 0x0,
            hi_lo_ready: 0x0,
            registers: [0x0; 32],
            load_slot: DelaySlot::default(),
            icache: Box::new([CacheLine::default(); 1024]),
            bus: Bus::new(bios),
            cop0: Cop0::new(),
        })
    }

    /// Get value of register at index.
    fn read_reg(&self, index: u32) -> u32 {
        self.registers[index as usize]
    }

    /// Set register at index.
    fn set_reg(&mut self, index: u32, value: u32) {
        self.registers[index as usize] = value;
    }

    /// Load address from bus.
    fn load<T: AddrUnit>(&mut self, address: u32) -> u32 {
        self.bus.load::<T>(address)
    }

    /// Store address to bus.
    fn store<T: AddrUnit>(&mut self, address: u32, value: u32) {
        if !self.cop0.cache_isolated() {
            self.bus.store::<T>(address, value);
        } else {
            // TODO: Write to cache/scratchpad.
        }
    }

    /// Add pending load. If there's already one pending, fetch it.
    fn add_load_slot(&mut self, register: u32, value: u32) {
        // If there already a pending load to the same register, we avoid writing to that register.
        // The branching is optimized away.
        let eq = if register == self.load_slot.register {
            0
        } else {
            1
        };
        self.set_reg(self.load_slot.register * eq, self.load_slot.value * eq);
        self.load_slot = DelaySlot { register, value };
    }

    /// Do pending load, if any.
    fn fetch_load_slot(&mut self) {
        self.set_reg(self.load_slot.register, self.load_slot.value);
        self.load_slot = DelaySlot::default();
    }

    /// MUL/DIV takes more than one cycle, but other instructions can run while the result is
    /// pending. This is an attempt to simulate that.
    fn add_pending_hi_lo(&mut self, cycles: u32, hi: u32, lo: u32) {
        self.hi_lo_ready = self.bus.cycle_count + cycles as u64;
        self.hi = hi;
        self.lo = lo;
    }

    /// If there is a DIV/MUL result pending, wait the required until it's ready.
    fn fetch_pending_hi_lo(&mut self) {
        self.bus.cycle_count = u64::max(self.bus.cycle_count, self.hi_lo_ready);
    }

    /// Branch to relative offset.
    fn branch(&mut self, offset: u32) {
        // Offset is shifted 2 bits since PC addresses must be 32-bit aligned.
        self.next_pc = self.pc.wrapping_add(offset << 2);
        self.branched = true;
        if !Word::is_aligned(self.next_pc) {
            // This is probably not required.
            self.cop0.set_reg(8, self.next_pc);
            self.throw_exception(Exception::AddressLoadError);
        }
    }

    /// Jump sets pc to address,
    fn jump(&mut self, address: u32) {
        self.next_pc = address;
        self.branched = true;
        if !Word::is_aligned(self.next_pc) {
            self.cop0.set_reg(8, self.next_pc);
            self.throw_exception(Exception::AddressLoadError);
        }
    }

    /// Start handeling an exception, and jumps to exception handling code in bios.
    fn throw_exception(&mut self, ex: Exception) {
        self.pc = self
            .cop0
            .enter_exception(self.last_pc, self.in_branch_delay, ex);
        self.next_pc = self.pc.wrapping_add(4);
    }

    fn irq_pending(&self) -> bool {
        let cause = self.cop0.read_reg(13)
            | ((self.bus.irq_state.active() as u32) << 10);
        let active = self.cop0.read_reg(12) & cause & 0xffff00;
        self.cop0.irq_enabled() && active != 0
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    pub fn bus_mut(&mut self) -> &mut Bus {
        &mut self.bus
    }

    pub fn current_instruction(&mut self) -> Opcode {
        Opcode::new(self.load::<Word>(self.pc))
    }

    /// Fetch and execute next instruction.
    pub fn step(&mut self) {
        let op = self.fetch_ins(self.pc);
        self.last_pc = self.pc;
        self.pc = self.next_pc;
        self.next_pc = self.next_pc.wrapping_add(4);
        self.in_branch_delay = self.branched;
        self.branched = false;
        self.exec(op);
        self.bus.cycle_count += 1;
    }

    /// Fetch an instruction from either memory or the icache. Also update the icache if required.
    fn fetch_ins(&mut self, address: u32) -> Opcode {
        // TODO: Avoid having to check this every cycle.
        if self.irq_pending() {
            self.fetch_load_slot();
            self.throw_exception(Exception::Interrupt); 
        }
        // Only KUSEG and KSEG0 are cached.
        let data = if address < 0xa0000000 && self.bus.cache_ctrl.icache_enabled() {
            // Only if this tag matches the tag of the cacheline is the cache valid.
            // This makes sure the valid flag is set, and it points at the right address.
            let tag = ((address & 0xfffff000) >> 12) | 0x80000000;
            let index = ((address & 0xffc) >> 2) as usize;
            let cache = self.icache[index as usize];
            // If the cacheline is valid, we just read from it, otherwise we fetch the instruction
            // from memory and store it in the cache.
            if cache.tag == tag {
                cache.data
            } else {
                let data = self.load::<Word>(address);
                // Update cacheline.
                self.icache[index] = CacheLine { tag, data };
                data
            }
        } else {
            // Cache misses take about 4 cycles.
            self.bus.cycle_count += 4;
            self.load::<Word>(address)
        };
        Opcode::new(data)
    }

    /// Execute opcode.
    fn exec(&mut self, opcode: Opcode) {
        match opcode.op() {
            0x0 => match opcode.special() {
                0x0 => self.op_sll(opcode),
                0x2 => self.op_srl(opcode),
                0x3 => self.op_sra(opcode),
                0x4 => self.op_sllv(opcode),
                0x6 => self.op_srlv(opcode),
                0x7 => self.op_srav(opcode),
                0x8 => self.op_jr(opcode),
                0x9 => self.op_jalr(opcode),
                0xc => self.op_syscall(),
                0xd => self.op_break(),
                0x10 => self.op_mfhi(opcode),
                0x11 => self.op_mthi(opcode),
                0x12 => self.op_mflo(opcode),
                0x13 => self.op_mtlo(opcode),
                0x18 => self.op_mul(opcode),
                0x19 => self.op_mulu(opcode),
                0x1a => self.op_div(opcode),
                0x1b => self.op_divu(opcode),
                0x20 => self.op_add(opcode),
                0x21 => self.op_addu(opcode),
                0x22 => self.op_sub(opcode),
                0x23 => self.op_subu(opcode),
                0x24 => self.op_and(opcode),
                0x25 => self.op_or(opcode),
                0x26 => self.op_xor(opcode),
                0x27 => self.op_nor(opcode),
                0x2a => self.op_slt(opcode),
                0x2b => self.op_sltu(opcode),
                _ => self.op_illegal(),
            },
            0x1 => self.op_bcondz(opcode),
            0x2 => self.op_j(opcode),
            0x3 => self.op_jal(opcode),
            0x4 => self.op_beq(opcode),
            0x5 => self.op_bne(opcode),
            0x6 => self.op_blez(opcode),
            0x7 => self.op_bgtz(opcode),
            0x8 => self.op_addi(opcode),
            0x9 => self.op_addiu(opcode),
            0xa => self.op_slti(opcode),
            0xb => self.op_sltui(opcode),
            0xc => self.op_andi(opcode),
            0xd => self.op_ori(opcode),
            0xe => self.op_xori(opcode),
            0xf => self.op_lui(opcode),
            0x10 => self.op_cop0(opcode),
            0x11 => self.op_cop1(),
            0x12 => self.op_cop2(opcode),
            0x13 => self.op_cop3(),
            0x20 => self.op_lb(opcode),
            0x21 => self.op_lh(opcode),
            0x22 => self.op_lwl(opcode),
            0x23 => self.op_lw(opcode),
            0x24 => self.op_lbu(opcode),
            0x25 => self.op_lhu(opcode),
            0x26 => self.op_lwr(opcode),
            0x28 => self.op_sb(opcode),
            0x29 => self.op_sh(opcode),
            0x2a => self.op_swl(opcode),
            0x2b => self.op_sw(opcode),
            0x2e => self.op_swr(opcode),
            0x30 => self.op_lwc0(),
            0x31 => self.op_lwc1(),
            0x32 => self.op_lwc2(),
            0x33 => self.op_lwc3(),
            0x38 => self.op_swc0(),
            0x39 => self.op_swc1(),
            0x3a => self.op_swc2(),
            0x3b => self.op_swc3(),
            _ => self.op_illegal(),
        }
    }

    /// SLL - Shift left logical.
    fn op_sll(&mut self, op: Opcode) {
        let value = self.read_reg(op.rt()) << op.shift();
        self.fetch_load_slot();
        self.set_reg(op.rd(), value);
    }

    /// SRL - Shift right logical. Same as SRA, but unsigned.
    fn op_srl(&mut self, op: Opcode) {
        let value = self.read_reg(op.rt()) >> op.shift();
        self.fetch_load_slot();
        self.set_reg(op.rd(), value);
    }

    /// SRA - Shift right arithmetic.
    fn op_sra(&mut self, op: Opcode) {
        let value = (self.read_reg(op.rt()) as i32) >> op.shift();
        self.fetch_load_slot();
        self.set_reg(op.rd(), value as u32);
    }

    /// SLLV - Shift left logical variable.
    fn op_sllv(&mut self, op: Opcode) {
        // The 0x1f mask is to ensure you shift 32 bits.
        let value = self.read_reg(op.rt()) << (self.read_reg(op.rs() & 0x1f));
        self.fetch_load_slot();
        self.set_reg(op.rd(), value);
    }

    /// SRLV - Shift right logical variable.
    fn op_srlv(&mut self, op: Opcode) {
        let value = self.read_reg(op.rt()) >> (self.read_reg(op.rs()) & 0x1f);
        self.fetch_load_slot();
        self.set_reg(op.rd(), value as u32);
    }

    /// SRAV - Shift right arithmetic variable.
    fn op_srav(&mut self, op: Opcode) {
        let value = (self.read_reg(op.rt()) as i32) >> (self.read_reg(op.rs()) & 0x1f);
        self.fetch_load_slot();
        self.set_reg(op.rd(), value as u32);
    }

    /// JR - Jump register.
    fn op_jr(&mut self, op: Opcode) {
        self.jump(self.read_reg(op.rs()));
        self.fetch_load_slot();
    }

    /// JALR - Jump and link register.
    fn op_jalr(&mut self, op: Opcode) {
        let pc = self.next_pc;
        self.jump(self.read_reg(op.rs()));
        self.fetch_load_slot();
        self.set_reg(op.rd(), pc);
    }

    /// SYSCALL - Throws an syscall exception.
    fn op_syscall(&mut self) {
        self.fetch_load_slot();
        self.throw_exception(Exception::Syscall);
    }

    /// BREAK - Throws an break exception.
    fn op_break(&mut self) {
        self.fetch_load_slot();
        self.throw_exception(Exception::Breakpoint);
    }

    /// MFHI - Move from high.
    fn op_mfhi(&mut self, op: Opcode) {
        self.fetch_pending_hi_lo();
        self.fetch_load_slot();
        let value = self.hi;
        self.set_reg(op.rd(), value);
    }

    /// MTHI - Move to high.
    fn op_mthi(&mut self, op: Opcode) {
        self.hi = self.read_reg(op.rs());
        self.fetch_load_slot();
    }

    /// MFLO - Move from low.
    fn op_mflo(&mut self, op: Opcode) {
        self.fetch_pending_hi_lo();
        self.fetch_load_slot();
        let value = self.lo;
        self.set_reg(op.rd(), value);
    }

    /// MTLO - Move to low.
    fn op_mtlo(&mut self, op: Opcode) {
        self.lo = self.read_reg(op.rs());
        self.fetch_load_slot();
    }

    /// MUL - Signed multiplication.
    /// Multiplication takes different amount of cycles to complete dependent on the size of the
    /// inputs.
    fn op_mul(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs()) as i32;
        let rhs = self.read_reg(op.rt()) as i32;
        let cycles = match if lhs < 0 { !lhs } else { lhs }.leading_zeros() {
            00..=11 => 13,
            12..=20 => 9,
            _ => 7,
        };
        let value = (lhs as i64) * (rhs as i64);
        self.fetch_load_slot();
        self.add_pending_hi_lo(cycles, (value >> 32) as u32, value as u32);
    }

    /// MULU - Unsigned multiplication.
    fn op_mulu(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs());
        let rhs = self.read_reg(op.rt());
        let cycles = match lhs {
            0x00000000..=0x000007ff => 13,
            0x00000800..=0x000fffff => 9,
            _ => 7,
        };
        let value = (lhs as u64) * (rhs as u64);
        self.fetch_load_slot();
        self.add_pending_hi_lo(cycles, (value >> 32) as u32, value as u32);
    }

    /// DIV - Signed division.
    /// Stores result in hi/low registers. Division doesn't throw when dviding by 0 or overflow,
    /// instead it gives garbage values. This takes 36 cycles to complete, but continues executing.
    /// It only halts if hi/low registers are fetched.
    fn op_div(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs()) as i32;
        let rhs = self.read_reg(op.rt()) as i32;
        self.fetch_load_slot();
        if rhs == 0 {
            let lo: u32 = if lhs < 0 { 1 } else { 0xffffffff };
            self.add_pending_hi_lo(36, lhs as u32, lo);
        } else if rhs == -1 && lhs as u32 == 0x80000000 {
            self.add_pending_hi_lo(36, 0, 0x80000000);
        } else {
            self.add_pending_hi_lo(36, (lhs % rhs) as u32, (lhs / rhs) as u32);
        }
    }

    /// DIVU - Unsigned division.
    /// Almost same as DIV, but only one error case.
    fn op_divu(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs());
        let rhs = self.read_reg(op.rt());
        self.fetch_load_slot();
        if rhs == 0 {
            self.add_pending_hi_lo(36, lhs, 0xffffffff);
        } else {
            self.add_pending_hi_lo(36, lhs % rhs, lhs / rhs);
        }
    }

    /// ADD - Add signed.
    /// Throws on overflow.
    fn op_add(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs()) as i32;
        let rhs = self.read_reg(op.rt()) as i32;
        self.fetch_load_slot();
        if let Some(value) = lhs.checked_add(rhs) {
            self.set_reg(op.rd(), value as u32);
        } else {
            self.throw_exception(Exception::ArithmeticOverflow);
        }
    }

    /// ADDU - Add unsigned.
    fn op_addu(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()).wrapping_add(self.read_reg(op.rt()));
        self.fetch_load_slot();
        self.set_reg(op.rd(), value);
    }

    /// SUB - Signed subtraction. Throws on underflow.
    fn op_sub(&mut self, op: Opcode) {
        let rhs = self.read_reg(op.rs()) as i32;
        let lhs = self.read_reg(op.rt()) as i32;
        if let Some(value) = rhs.checked_sub(lhs) {
            self.set_reg(op.rd(), value as u32);
        } else {
            self.throw_exception(Exception::ArithmeticOverflow);
        }
    }

    /// SUBU - Subtract unsigned.
    fn op_subu(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()).wrapping_sub(self.read_reg(op.rt()));
        self.fetch_load_slot();
        self.set_reg(op.rd(), value);
    }

    /// AND - Bitwise and.
    fn op_and(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) & self.read_reg(op.rt());
        self.fetch_load_slot();
        self.set_reg(op.rd(), value);
    }

    /// OR - Bitwise or.
    fn op_or(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) | self.read_reg(op.rt());
        self.fetch_load_slot();
        self.set_reg(op.rd(), value);
    }

    /// XOR - Bitwise exclusive or.
    fn op_xor(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) ^ self.read_reg(op.rt());
        self.fetch_load_slot();
        self.set_reg(op.rd(), value);
    }

    /// NOR - Bitwise not or.
    fn op_nor(&mut self, op: Opcode) {
        let value = !(self.read_reg(op.rs()) | self.read_reg(op.rt()));
        self.fetch_load_slot();
        self.set_reg(op.rd(), value);
    }

    /// SLT - Set if less than.
    fn op_slt(&mut self, op: Opcode) {
        let value = (self.read_reg(op.rs()) as i32) < (self.read_reg(op.rt()) as i32);
        self.fetch_load_slot();
        self.set_reg(op.rd(), value as u32);
    }

    /// SLTU - Set if less than unsigned.
    fn op_sltu(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) < self.read_reg(op.rt());
        self.fetch_load_slot();
        self.set_reg(op.rd(), value as u32);
    }

    /// BCONDZ - Multiply branch instructions.
    /// BLTZ - Branch if less than zero.
    ///     - If bit 16 of the opcode is not set, and bit 17..20 doesn't equal 0x80.
    /// BLTZAL - Branch if less than zero and set return register.
    ///     - If bit 16 of the opcode is not set, but bit 17..20 equals 0x80.
    /// BGEZ - Branch if greater than or equal to zero.
    ///     - If bit 16 of the opcode is set, but bit 17..20 doesn't equal 0x80.
    /// BGEZAL - Branch if greater than or equal to zero and set return register.
    ///     - If bit 16 of the opcode is set, and bit 17..20 equals 0x80.
    fn op_bcondz(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) as i32;
        let cond = (value < 0) as u32;
        // If the instruction is to test greater or equal zero, we just
        // xor cond, since that's the oposite result.
        let cond = cond ^ op.bgez() as u32;
        self.fetch_load_slot();
        // Set return register if required.
        if op.set_ra_on_branch() {
            self.set_reg(31, self.next_pc);
        }
        if cond != 0 {
            self.branch(op.signed_imm());
        }
    }

    /// J - Jump.
    fn op_j(&mut self, op: Opcode) {
        self.jump((self.pc & 0xf0000000) | (op.target() << 2));
        self.fetch_load_slot();
    }

    /// JAL - Jump and link.
    fn op_jal(&mut self, op: Opcode) {
        let pc = self.next_pc;
        self.op_j(op);
        // Store PC in return register.
        self.set_reg(31, pc);
    }

    /// BEQ - Branch if equal.
    fn op_beq(&mut self, op: Opcode) {
        if self.read_reg(op.rs()) == self.read_reg(op.rt()) {
            self.branch(op.signed_imm());
        }
        self.fetch_load_slot();
    }

    /// BNE - Branch if not equal.
    fn op_bne(&mut self, op: Opcode) {
        if self.read_reg(op.rs()) != self.read_reg(op.rt()) {
            self.branch(op.signed_imm());
        }
        self.fetch_load_slot();
    }

    /// BLEZ - Branch if less than or equal to zero.
    fn op_blez(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) as i32;
        if value <= 0 {
            self.branch(op.signed_imm());
        }
        self.fetch_load_slot();
    }

    /// BGTZ - Branch if greater than zero.
    fn op_bgtz(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) as i32;
        if value > 0 {
            self.branch(op.signed_imm());
        }
        self.fetch_load_slot();
    }

    /// ADDI - Add immediate signed.
    /// Same as ADDUI but throw exception on overflow.
    fn op_addi(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) as i32;
        self.fetch_load_slot();
        if let Some(value) = value.checked_add(op.signed_imm() as i32) {
            self.set_reg(op.rt(), value as u32);
        } else {
            self.throw_exception(Exception::ArithmeticOverflow);
        }
    }

    /// ADDUI - Add immediate unsigned.
    /// Actually adding a signed int to target register, not unsigned.
    /// Unsigned in this case just means wrapping on overflow.
    fn op_addiu(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        self.fetch_load_slot();
        self.set_reg(op.rt(), value);
    }

    /// SLTI - Set if less than immediate signed.
    fn op_slti(&mut self, op: Opcode) {
        let value = (self.read_reg(op.rs()) as i32) < (op.signed_imm() as i32);
        self.fetch_load_slot();
        self.set_reg(op.rt(), value as u32);
    }

    /// SLTI - Set if less than immediate unsigned.
    fn op_sltui(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) < op.signed_imm();
        self.fetch_load_slot();
        self.set_reg(op.rt(), value as u32);
    }

    /// ANDI - Bitwise and immediate.
    fn op_andi(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) & op.imm();
        self.fetch_load_slot();
        self.set_reg(op.rt(), value);
    }

    /// ORI - Bitwise or immediate.
    fn op_ori(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) | op.imm();
        self.fetch_load_slot();
        self.set_reg(op.rt(), value);
    }

    /// XORI - Bitwise exclusive Or immediate.
    fn op_xori(&mut self, op: Opcode) {
        let value = self.read_reg(op.rs()) ^ op.imm();
        self.fetch_load_slot();
        self.set_reg(op.rt(), value);
    }

    /// LUI - Load upper immediate.
    fn op_lui(&mut self, op: Opcode) {
        self.set_reg(op.rt(), op.imm() << 16);
        self.fetch_load_slot();
    }

    /// COP0 - Coprocessor0 instruction.
    fn op_cop0(&mut self, op: Opcode) {
        match op.cop0_op() {
            // MFC0 - Move from Co-Processor0.
            0x0 => {
                let register = op.rd();
                if register > 15 {
                    self.throw_exception(Exception::ReservedInstruction);
                } else {
                    self.add_load_slot(op.rt(), self.cop0.read_reg(register));
                }
            }
            // MTC0 - Move to Co-Processor0.
            0x4 => {
                self.fetch_load_slot();
                self.cop0.set_reg(op.rd(), self.read_reg(op.rt()));
            }
            // RFE - Restore from exception.
            0b10000 => {
                self.fetch_load_slot();
                self.cop0.exit_exception();
            }
            _ => self.throw_exception(Exception::ReservedInstruction),
        }
    }

    /// COP1 - Coprocessor1 instruction.
    fn op_cop1(&mut self) {
        // COP1 does not exist on the Playstation 1.
        self.throw_exception(Exception::CopUnusable);
    }

    /// COP2 - GME instruction.
    fn op_cop2(&mut self, _: Opcode) {
        todo!();
    }

    /// COP3 - Coprocessor3 instruction.
    fn op_cop3(&mut self) {
        // COP3 does not exist on the Playstation 1.
        self.throw_exception(Exception::CopUnusable);
    }

    /// LB - Load byte.
    fn op_lb(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        // Note: Byte is always aligned.
        let value = self.load::<Byte>(address) as i8;
        self.add_load_slot(op.rt(), value as u32);
    }

    /// LH - Load half word.
    fn op_lh(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        if HalfWord::is_aligned(address) {
            // This casting is apperently required to avoid messing up sign extension.
            let value = self.load::<HalfWord>(address) as i16;
            self.add_load_slot(op.rt(), value as u32);
        } else {
            self.cop0.set_reg(8, address);
            self.throw_exception(Exception::AddressLoadError);
        }
    }

    /// LWL - Load word left.
    /// This is used to load words which aren't 32-bit aligned.
    fn op_lwl(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        // This instruction somehow doesn't wait on load delay.
        let value = if self.load_slot.register == op.rt() {
            self.load_slot.value
        } else {
            self.read_reg(op.rt())
        };
        // Get word containing address.
        let word = self.load::<Word>(address & !0x3);
        // Extract 1, 2, 3, 4 bytes dependent on the address alignment.
        let value = match address & 0x3 {
            0 => (value & 0x00ffffff) | (word << 24),
            1 => (value & 0x0000ffff) | (word << 16),
            2 => (value & 0x000000ff) | (word << 8),
            3 => word,
            _ => unreachable!(),
        };
        self.add_load_slot(op.rt(), value);
    }

    /// LW - Load word.
    fn op_lw(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        if Word::is_aligned(address) {
            let value = self.load::<Word>(address);
            self.add_load_slot(op.rt(), value);
        } else {
            self.cop0.set_reg(8, address);
            self.throw_exception(Exception::AddressLoadError);
        }
    }

    /// LBU - Load byte unsigned.
    fn op_lbu(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        // Note: Byte is always aligned.
        let value = self.load::<Byte>(address);
        self.add_load_slot(op.rt(), value);
    }

    /// LHU - Load half word unsigned.
    fn op_lhu(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        if HalfWord::is_aligned(address) {
            let value = self.load::<Byte>(address);
            self.add_load_slot(op.rt(), value);
        } else {
            self.cop0.set_reg(8, address);
            self.throw_exception(Exception::AddressLoadError);
        }
    }

    /// LWR - Load word right.
    fn op_lwr(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        // This instruction somehow doesn't wait on load delay.
        let value = if self.load_slot.register == op.rt() {
            self.load_slot.value
        } else {
            self.read_reg(op.rt())
        };
        // Get word containing address.
        let word = self.load::<Word>(address & !0x3);
        // Extract 1, 2, 3, 4 bytes dependent on the address alignment.
        let value = match address & 0x3 {
            0 => word,
            1 => (value & 0xff000000) | (word >> 8),
            2 => (value & 0xffff0000) | (word >> 16),
            3 => (value & 0xffffff00) | (word >> 24),
            _ => unreachable!(),
        };
        self.add_load_slot(op.rt(), value);
    }

    /// SB - Store byte.
    /// Store byte from target register at address from source register + immediate value.
    fn op_sb(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let value = self.read_reg(op.rt());
        self.fetch_load_slot();
        self.store::<Byte>(address, value);
    }

    /// SH - Store half word.
    /// Store half word from target register at address from source register + immediate value.
    fn op_sh(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let value = self.read_reg(op.rt());
        self.fetch_load_slot();
        if HalfWord::is_aligned(address) {
            self.store::<HalfWord>(address, value);
        } else {
            self.cop0.set_reg(8, address);
            self.throw_exception(Exception::AddressStoreError);
        }
    }

    /// SWL - Store world left.
    /// This is used to store words to addresses which aren't 32-aligned.
    fn op_swl(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let value = self.read_reg(op.rt());
        // Get address of whole word containing unaligned address.
        let aligned = address & !3;
        let word = self.load::<Word>(aligned);
        // Extract 1, 2, 3, 4 bytes dependent on the address alignment.
        let value = match address & 3 {
            0 => (word & 0xffffff00) | (value >> 24),
            1 => (word & 0xffff0000) | (value >> 16),
            2 => (word & 0xffffff00) | (value >> 8),
            3 => word,
            _ => unreachable!(),
        };
        self.fetch_load_slot();
        self.store::<Word>(aligned, value);
    }

    /// SW - Store word.
    /// Store word from target register at address from source register + signed immediate value.
    fn op_sw(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let value = self.read_reg(op.rt());
        self.fetch_load_slot();
        if Word::is_aligned(address) {
            self.store::<Word>(address, value);
        } else {
            self.cop0.set_reg(8, address);
            self.throw_exception(Exception::AddressStoreError);
        }
    }

    /// SWR - Store world right.
    fn op_swr(&mut self, op: Opcode) {
        let address = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let value = self.read_reg(op.rt());
        // Get address of whole word containing unaligned address.
        let aligned = address & !3;
        let word = self.load::<Word>(aligned);
        // Extract 1, 2, 3, 4 bytes dependent on the address alignment.
        let value = match address & 3 {
            0 => word,
            1 => (word & 0x000000ff) | (value << 8),
            2 => (word & 0x0000ffff) | (value << 16),
            3 => (word & 0x00ffffff) | (value << 24),
            _ => unreachable!(),
        };
        self.fetch_load_slot();
        self.store::<Word>(aligned, value);
    }

    /// LWC0 - Load word from Coprocessor0.
    fn op_lwc0(&mut self) {
        // This doesn't work on the COP0.
        self.throw_exception(Exception::CopUnusable);
    }

    /// LWC1 - Load word from Coprocessor1.
    fn op_lwc1(&mut self) {
        self.throw_exception(Exception::CopUnusable);
    }

    /// LWC2 - Load word from Coprocessor2.
    fn op_lwc2(&mut self) {
        todo!();
    }

    /// LWC3 - Load word from Coprocessor3.
    fn op_lwc3(&mut self) {
        self.throw_exception(Exception::CopUnusable);
    }

    /// SWC0 - Store world in Coprocessor0.
    fn op_swc0(&mut self) {
        self.throw_exception(Exception::CopUnusable);
    }

    /// SWC1 - Store world in Coprocessor0.
    fn op_swc1(&mut self) {
        self.throw_exception(Exception::CopUnusable);
    }

    /// SWC2 - Store world in Coprocessor0.
    fn op_swc2(&mut self) {
        todo!();
    }

    /// SWC3 - Store world in Coprocessor0.
    fn op_swc3(&mut self) {
        self.throw_exception(Exception::CopUnusable);
    }

    /// ILLEGAL - Illegal/Undefined opcode.
    fn op_illegal(&mut self) {
        self.throw_exception(Exception::ReservedInstruction);
    }
}

pub const REGISTER_NAMES: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6",
    "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp",
    "ra",
];
