//! Emulation of the MIPS R3000 used by the original Sony Playstation.

pub mod cop0;
mod gte;

pub mod irq;
pub mod opcode;

use splst_util::Bit;

use crate::Cycle;
use crate::bus::{Event, AddrUnit, Bus, Byte, HalfWord, Word, bios::Bios};

use cop0::{Cop0, Exception};
use gte::Gte;

pub use opcode::{Opcode, RegIdx};
pub use irq::{IrqState, Irq};

#[derive(Default, Clone, Copy)]
struct DelaySlot {
    pub reg: RegIdx,
    pub val: u32,
}

#[derive(Default, Copy, Clone)]
struct CacheLine {
    pub tag: u32,
    pub data: u32,
}

pub struct Cpu {
    /// At the start of each instruction, this points to the last instruction executed. During
    /// the instruction, it points to the current opcode being executed.
    last_pc: u32,
    /// This points to the opcode about to be executed at the start of each
    /// instruction. During the instruction it points at the next opcode.
    pub pc: u32,
    /// Always one step ahead of 'pc'. This is used to emulate CPU pipelineing and more
    /// specifically branch delay.
    ///
    /// The MIPS R3000 pipelines one instruction ahead, meaning it loads the next
    /// opcode while the current instruction is being executed to speed up execution. This it not a
    /// problem when executing straight line code without branches, but it becomes problematic when
    /// a branch occours. Unlike modern processors, MIPS doesn't flush the pipeline and start
    /// over from the taken branch. This means that the processor always will execute the
    /// instruction right after a branch, no matter if the branch was taken or not.
    ///
    /// To emulate this, 'next_pc' is get's changed when branching instead of 'pc', which works
    /// well besides when entering and expception.
    pub next_pc: u32,
    /// Set to true if the current instruction is executed in the branch delay slot, in other words
    /// if the previous instruction branched.
    ///
    /// It's used when entering an exception. If the CPU is in a delay slot, it has to return one
    /// instruction behind 'last_pc'.
    in_branch_delay: bool,
    /// Set when a branch occours. Used to set 'in_branch_delay'.
    branched: bool,
    /// Results of multiply and divide instructions aren't stored in general purpose
    /// registers like normal instructions, but is instead stored in two special registers hi and
    /// lo.
    pub hi: u32,
    pub lo: u32,
    /// This stores the absolute cycle when the result of an multiply or divide instruction is
    /// ready since they take more than a single cycle to complete. The CPU can run while the
    /// result is being calculated, but if the result is being read before it's ready, the CPU will
    /// wait before continuing.
    hi_lo_ready: Cycle,
    /// # General Purpose Registers
    ///
    /// All registers of the MIPS R3000 are essentially general purpose besides $r0 which always
    /// contains the value 0. They are however used for specific purposes depending on convention.
    ///
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
    /// # Load Delay Slot
    ///
    /// This is used to emulate the pipeline of the MIPS R3000. When loading data from the BUS,
    /// it's not immediately ready to be read from the register, because fetching data takes more
    /// than a single cycle. Unlike modern CPUs, this doesn't wait or handle that automatically.
    ///
    /// When loading a value from the BUS to a register, the value is ready in the register after
    /// the next instruction has read the registers, but before it has written to them. This is
    /// emulated by calling 'fetch_load_slot' in (mostly) every instruction, after reading the
    /// data needed from the registers, but after writing to any. If the load delay slot
    /// alreay contains a pending load to the same register when a load instruction tries to add a
    /// new pending load, it ignores the first load.
    /// 
    /// This field contains the value and register index of any pending loads. It takes advantage
    /// of the fact that the $r0 register always contains the value 0 to avoid branching every
    /// instruction. If there is any pending load, it writes the data to the register, if there
    /// isn't any load, it writes 0 to the $r0 register. This seems to speeds up the CPU at
    /// least 15% compared to branching, most likely because the branch predicter has a hard time.
    load_delay: DelaySlot,
    /// Memory sections KUSEG and KSEG0 are cached for instructions.
    icache: Box<[CacheLine; 1024]>,
    pub bus: Bus,
    gte: Gte,
    cop0: Cop0,
}

const PC_START_ADDRESS: u32 = 0xbfc00000;

impl Cpu {
    pub fn new(bios: Bios) -> Box<Self> {
        let bus = Bus::new(bios);
        let icache = Box::new([CacheLine::default(); 1024]);
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
            load_delay: DelaySlot::default(),
            gte: Gte::new(),
            cop0: Cop0::new(),
            icache,
            bus,
        })
    }

    fn read_reg(&self, idx: RegIdx) -> u32 {
        self.registers[idx.0 as usize]
    }

    fn set_reg(&mut self, idx: RegIdx, value: u32) {
        self.registers[idx.0 as usize] = value;
        self.registers[0] = 0;
    }

    fn load<T: AddrUnit>(&mut self, addr: u32) -> Result<u32, Exception> {
        self.bus.load::<T>(addr)
    }

    fn store<T: AddrUnit>(&mut self, addr: u32, val: u32) -> Result<(), Exception> {
        if !self.cop0.cache_isolated() {
            self.bus.store::<T>(addr, val)
        } else {
            // TODO: Write to scratchpad.
            Ok(())
        }
    }

    /// Add pending load. If there's already one pending, fetch it.
    fn add_load_slot(&mut self, reg: RegIdx, val: u32) {
        let (sreg, sval) = if self.load_delay.reg != reg {
            (self.load_delay.reg, self.load_delay.val)
        } else {
            (RegIdx::ZERO, 0) 
        };
        self.set_reg(sreg, sval);
        self.load_delay = DelaySlot { reg, val };
    }

    /// Fetch pending load if there is any.
    fn fetch_load_slot(&mut self) {
        self.set_reg(self.load_delay.reg, self.load_delay.val);
        self.load_delay = DelaySlot::default();
    }

    /// Add result to hi and lo register.
    fn add_pending_hi_lo(&mut self, cycles: Cycle, hi: u32, lo: u32) {
        self.hi_lo_ready = self.bus.schedule.cycle() + cycles;
        self.hi = hi;
        self.lo = lo;
    }

    /// Wait for hi lo results.
    fn fetch_pending_hi_lo(&mut self) {
        self.bus.schedule.skip_to(self.hi_lo_ready);
    }

    /// Branch to relative offset.
    fn branch(&mut self, offset: u32) {
        // Offset is shifted 2 bits since PC addresses must be 32-bit aligned.
        self.next_pc = self.pc.wrapping_add(offset << 2);
        self.branched = true;
    }

    /// Jump to absolute.
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
        trace!("Exception thrown: {:?}", ex);
        self.pc = self
            .cop0
            .enter_exception(self.last_pc, self.in_branch_delay, ex);
        self.next_pc = self.pc.wrapping_add(4);
    }

    fn irq_pending(&self) -> bool {
        let active = (self.bus.irq_state.active() as u32) << 10;
        let cause = self.cop0.read_reg(13) | active;
        let active = self.cop0.read_reg(12) & cause & 0xffff00;
        self.cop0.irq_enabled() && active != 0
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }

    pub fn bus_mut(&mut self) -> &mut Bus {
        &mut self.bus
    }

    // Used fot debug.
    pub fn curr_ins(&mut self) -> Opcode {
        Opcode::new(self.load::<Word>(self.pc).unwrap())
    }

    /// Move the program counter to the next instruction. Returns the address of the next
    /// instruction to be executed.
    fn next_pc(&mut self) -> u32 {
        self.last_pc = self.pc;
        self.pc = self.next_pc;
        self.next_pc = self.next_pc.wrapping_add(4);
        self.in_branch_delay = self.branched;
        self.branched = false;
        self.last_pc  
    }

    /// Fetch and execute next instruction.
    pub fn step(&mut self) {
        if let Some(event) = self.bus.schedule.pop_event() {
            if let Event::IrqTrigger(irq) = event {
                if self.irq_pending() {
                    warn!("IRQ pending when triggering IRQ of type: {}", irq);
                }
                self.bus.irq_state.trigger(irq);
                if self.irq_pending() {
                    self.next_pc();
                    self.fetch_load_slot();
                    self.throw_exception(Exception::Interrupt);
                }
            } else {
                self.bus.handle_event(event);
            }
        } else {
            let addr = self.next_pc();
            if addr < 0xa0000000 && self.bus.cache_ctrl.icache_enabled() {
                let tag = ((addr & 0xfffff000) >> 12) | 0x80000000;
                let index = ((addr & 0xffc) >> 2) as usize;
                let cache = self.icache[index];
                if cache.tag == tag {
                    self.exec(Opcode::new(cache.data));
                } else {
                    match self.load::<Word>(addr) {
                        Ok(data) => {
                            self.icache[index] = CacheLine { tag, data };
                            self.exec(Opcode::new(data));
                        }
                        Err(exp) if exp == Exception::AddressLoadError => {
                            // This might not be correct. Accessing at an unaligned address when loading
                            // an instruction could throw a BUS instruction error instead.
                            self.throw_exception(exp);
                        }
                        Err(..) => {
                            self.throw_exception(Exception::BusInstructionError);
                        }
                    }
                }
            } else {
                // Cache misses take about 4 cycles.
                self.bus.schedule.tick(4);
                if let Ok(val) = self.load::<Word>(addr) {
                    self.exec(Opcode::new(val));
                } else {
                    self.throw_exception(Exception::BusInstructionError);
                }
            }
            self.bus.schedule.tick(1);
        }
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
                0x18 => self.op_mult(opcode),
                0x19 => self.op_multu(opcode),
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
            0x32 => self.op_lwc2(opcode),
            0x33 => self.op_lwc3(),
            0x38 => self.op_swc0(),
            0x39 => self.op_swc1(),
            0x3a => self.op_swc2(opcode),
            0x3b => self.op_swc3(),
            _ => self.op_illegal(),
        }
    }
}

/// CPU opcode implementation.
impl Cpu {
    /// SLL - Shift left logical.
    fn op_sll(&mut self, op: Opcode) {
        let val = self.read_reg(op.rt()) << op.shift();
        self.fetch_load_slot();
        self.set_reg(op.rd(), val);
    }

    /// SRL - Shift right logical. Same as SRA, but unsigned.
    fn op_srl(&mut self, op: Opcode) {
        let val = self.read_reg(op.rt()) >> op.shift();
        self.fetch_load_slot();
        self.set_reg(op.rd(), val);
    }

    /// SRA - Shift right arithmetic.
    fn op_sra(&mut self, op: Opcode) {
        let val = (self.read_reg(op.rt()) as i32) >> op.shift();
        self.fetch_load_slot();
        self.set_reg(op.rd(), val as u32);
    }

    /// SLLV - Shift left logical variable.
    fn op_sllv(&mut self, op: Opcode) {
        let val = self.read_reg(op.rt()) << (self.read_reg(op.rs()) & 0x1f);
        self.fetch_load_slot();
        self.set_reg(op.rd(), val);
    }

    /// SRLV - Shift right logical variable.
    fn op_srlv(&mut self, op: Opcode) {
        let val = self.read_reg(op.rt()) >> (self.read_reg(op.rs()) & 0x1f);
        self.fetch_load_slot();
        self.set_reg(op.rd(), val as u32);
    }

    /// SRAV - Shift right arithmetic variable.
    fn op_srav(&mut self, op: Opcode) {
        let val = (self.read_reg(op.rt()) as i32) >> (self.read_reg(op.rs()) & 0x1f);
        self.fetch_load_slot();
        self.set_reg(op.rd(), val as u32);
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

    fn syscall_trace(&mut self) {
        match self.read_reg(RegIdx::V0) {
            1 => trace!("syscall: print {}", self.read_reg(RegIdx::A0)),
            2 | 3 => warn!("syscall: print float"),
            4 => {
                let mut addr = self.read_reg(RegIdx::A0);
                let mut print = String::new();
                loop {
                    let c = match self.load::<Byte>(addr) {
                        Ok(val) => (val as u8) as char,
                        Err(_) => break,
                    };
                    if c == '\n' {
                        break
                    }
                    print.push(c);
                    addr += 1;
                }
                debug!("syscall: print \"{}\"", print);
            }
            5 => debug!("syscall: read integer"),
            6 | 7 => trace!("syscall: read float"),
            8 => {
                debug!("syscall: read string"); 
            }
            9 => {
                debug!("syscall: allocate {} bytes", self.read_reg(RegIdx::A0));
            }
            10 => debug!("syscall: terminate"),
            val => debug!("syscall: type {}", val),
        }
    }

    /// SYSCALL - Throws syscall exception.
    fn op_syscall(&mut self) {
        self.fetch_load_slot();
        if log_enabled!(log::Level::Trace) {
            self.syscall_trace();
        }
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
        self.set_reg(op.rd(), self.hi);
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
        self.set_reg(op.rd(), self.lo);
    }

    /// MTLO - Move to low.
    fn op_mtlo(&mut self, op: Opcode) {
        self.lo = self.read_reg(op.rs());
        self.fetch_load_slot();
    }

    /// MULT - Signed multiplication.
    ///
    /// Multiplication takes different amount of cycles to complete dependent on the size of the
    /// inputs numbers.
    fn op_mult(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs()) as i32;
        let rhs = self.read_reg(op.rt()) as i32;
        let cycles = match if lhs < 0 { !lhs } else { lhs }.leading_zeros() {
            00..=11 => 13,
            12..=20 => 9,
            _ => 7,
        };
        let val = i64::from(lhs) * i64::from(rhs);
        let val = val as u64;
        self.fetch_load_slot();
        self.add_pending_hi_lo(cycles, (val >> 32) as u32, val as u32);
    }

    /// MULTU - Unsigned multiplication.
    fn op_multu(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs());
        let rhs = self.read_reg(op.rt());
        let cycles = match lhs {
            0x00000000..=0x000007ff => 13,
            0x00000800..=0x000fffff => 9,
            _ => 7,
        };
        let val = u64::from(lhs) * u64::from(rhs);
        self.fetch_load_slot();
        self.add_pending_hi_lo(cycles, (val >> 32) as u32, val as u32);
    }

    /// DIV - Signed division.
    ///
    /// It always takes 36 cycles to complete. It doesn't throw an expception if dividing by 0, but
    /// instead returns garbage values.
    fn op_div(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs()) as i32;
        let rhs = self.read_reg(op.rt()) as i32;
        self.fetch_load_slot();
        if rhs == 0 {
            let lo: u32 = if lhs < 0 { 1 } else { 0xffff_ffff };
            self.add_pending_hi_lo(36, lhs as u32, lo);
        } else if rhs == -1 && lhs as u32 == 0x8000_0000 {
            self.add_pending_hi_lo(36, 0, 0x8000_0000);
        } else {
            self.add_pending_hi_lo(36, (lhs % rhs) as u32, (lhs / rhs) as u32);
        }
    }

    /// DIVU - Unsigned division.
    fn op_divu(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs());
        let rhs = self.read_reg(op.rt());
        self.fetch_load_slot();
        if rhs == 0 {
            self.add_pending_hi_lo(36, lhs, 0xffff_ffff);
        } else {
            self.add_pending_hi_lo(36, lhs % rhs, lhs / rhs);
        }
    }

    /// ADD - Add signed.
    fn op_add(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs()) as i32;
        let rhs = self.read_reg(op.rt()) as i32;
        self.fetch_load_slot();
        if let Some(val) = lhs.checked_add(rhs) {
            self.set_reg(op.rd(), val as u32);
        } else {
            self.throw_exception(Exception::ArithmeticOverflow);
        }
    }

    /// ADDU - Add unsigned.
    fn op_addu(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs());
        let rhs = self.read_reg(op.rt());
        let val = lhs.wrapping_add(rhs);
        self.fetch_load_slot();
        self.set_reg(op.rd(), val);
    }

    /// SUB - Signed subtraction.
    fn op_sub(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs()) as i32;
        let rhs = self.read_reg(op.rt()) as i32;
        if let Some(val) = lhs.checked_sub(rhs) {
            self.set_reg(op.rd(), val as u32);
        } else {
            self.throw_exception(Exception::ArithmeticOverflow);
        }
    }

    /// SUBU - Subtract unsigned.
    fn op_subu(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs());
        let rhs = self.read_reg(op.rt());
        let val = lhs.wrapping_sub(rhs);
        self.fetch_load_slot();
        self.set_reg(op.rd(), val);
    }

    /// AND - Bitwise and.
    fn op_and(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) & self.read_reg(op.rt());
        self.fetch_load_slot();
        self.set_reg(op.rd(), val);
    }

    /// OR - Bitwise or.
    fn op_or(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) | self.read_reg(op.rt());
        self.fetch_load_slot();
        self.set_reg(op.rd(), val);
    }

    /// XOR - Bitwise exclusive or.
    fn op_xor(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) ^ self.read_reg(op.rt());
        self.fetch_load_slot();
        self.set_reg(op.rd(), val);
    }

    /// NOR - Bitwise not or.
    fn op_nor(&mut self, op: Opcode) {
        let val = !(self.read_reg(op.rs()) | self.read_reg(op.rt()));
        self.fetch_load_slot();
        self.set_reg(op.rd(), val);
    }

    /// SLT - Set if less than.
    fn op_slt(&mut self, op: Opcode) {
        let lhs = self.read_reg(op.rs()) as i32;
        let rhs = self.read_reg(op.rt()) as i32;
        let val = lhs < rhs;
        self.fetch_load_slot();
        self.set_reg(op.rd(), val as u32);
    }

    /// SLTU - Set if less than unsigned.
    fn op_sltu(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) < self.read_reg(op.rt());
        self.fetch_load_slot();
        self.set_reg(op.rd(), val as u32);
    }

    /// BCONDZ - Conditional branching.
    ///
    /// Multiple conditional branch instructions combined into on opcode. If bit 16 of the opcode
    /// is set, then it set's the return value register. If bits 17..20 of the opcode equals 0x80,
    /// then it branches if the value is greater than or equal to zero, otherwise it branches if
    /// the values is less than zero. 
    ///
    /// - BLTZ - Branch if less than zero.
    /// - BLTZAL - Branch if less than zero and set return register.
    /// - BGEZ - Branch if greater than or equal to zero.
    /// - BGEZAL - Branch if greater than or equal to zero and set return register.
    fn op_bcondz(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) as i32;
        let cond = (val < 0) as u32;
        // If the instruction is to test greater or equal zero, we just
        // xor cond, since that's the oposite result.
        let cond = cond ^ op.bgez() as u32;
        self.fetch_load_slot();
        // Set return register if required.
        if op.update_ra_on_branch() {
            self.set_reg(RegIdx::new(31), self.next_pc);
        }
        if cond != 0 {
            self.branch(op.signed_imm());
        }
    }

    /// J - Jump.
    fn op_j(&mut self, op: Opcode) {
        self.jump((self.pc & 0xf000_0000) | ((op.target() << 2) & 0x0ffffffc));
        self.fetch_load_slot();
    }

    /// JAL - Jump and link.
    fn op_jal(&mut self, op: Opcode) {
        let pc = self.next_pc;
        self.op_j(op);
        self.set_reg(RegIdx::RA, pc);
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
        let val = self.read_reg(op.rs()) as i32;
        if val <= 0 {
            self.branch(op.signed_imm());
        }
        self.fetch_load_slot();
    }

    /// BGTZ - Branch if greater than zero.
    fn op_bgtz(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) as i32;
        if val > 0 {
            self.branch(op.signed_imm());
        }
        self.fetch_load_slot();
    }

    /// ADDI - Add immediate signed.
    fn op_addi(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) as i32;
        self.fetch_load_slot();
        if let Some(val) = val.checked_add(op.signed_imm() as i32) {
            self.set_reg(op.rt(), val as u32);
        } else {
            self.throw_exception(Exception::ArithmeticOverflow);
        }
    }

    /// ADDUI - Add immediate unsigned.
    ///
    /// Actually adding a signed int to target register, not unsigned.
    /// Unsigned in this case just means wrapping on overflow.
    fn op_addiu(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        self.fetch_load_slot();
        self.set_reg(op.rt(), val);
    }

    /// SLTI - Set if less than immediate signed.
    fn op_slti(&mut self, op: Opcode) {
        let val = (self.read_reg(op.rs()) as i32) < (op.signed_imm() as i32);
        self.fetch_load_slot();
        self.set_reg(op.rt(), val as u32);
    }

    /// SLTUI - Set if less than immediate unsigned.
    fn op_sltui(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) < op.signed_imm();
        self.fetch_load_slot();
        self.set_reg(op.rt(), val as u32);
    }

    /// ANDI - Bitwise and immediate.
    fn op_andi(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) & op.imm();
        self.fetch_load_slot();
        self.set_reg(op.rt(), val);
    }

    /// ORI - Bitwise or immediate.
    fn op_ori(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) | op.imm();
        self.fetch_load_slot();
        self.set_reg(op.rt(), val);
    }

    /// XORI - Bitwise exclusive Or immediate.
    fn op_xori(&mut self, op: Opcode) {
        let val = self.read_reg(op.rs()) ^ op.imm();
        self.fetch_load_slot();
        self.set_reg(op.rt(), val);
    }

    /// LUI - Load upper immediate.
    fn op_lui(&mut self, op: Opcode) {
        let val = op.imm() << 16;
        self.fetch_load_slot();
        self.set_reg(op.rt(), val);
    }

    /// COP0 - Coprocessor0 instruction.
    fn op_cop0(&mut self, op: Opcode) {
        match op.cop_op() {
            // MFC0 - Move from Co-Processor0.
            0x0 => {
                let reg = op.rd().0;
                if reg > 15 {
                    self.throw_exception(Exception::ReservedInstruction);
                } else {
                    self.add_load_slot(op.rt(), self.cop0.read_reg(reg.into()));
                }
            }
            // MTC0 - Move to Co-Processor0.
            0x4 => {
                self.fetch_load_slot();
                self.cop0.set_reg(op.rd().0.into(), self.read_reg(op.rt()));
            }
            // RFE - Restore from exception.
            0x10 => {
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
    fn op_cop2(&mut self, op: Opcode) {
        let cop = op.cop_op();
        if cop.bit(4) {
            self.gte.cmd(op.0);
        } else {
            match cop {
                // Load from COP2 data register.
                0x0 => {
                    let val = self.gte.data_load(op.rd().0.into());
                    self.add_load_slot(op.rt(), val);
                }
                // Load from COP2 control register.
                0x2 => {
                    todo!()
                }
                // Store to COP2 data register.
                0x4 => {
                    let val = self.read_reg(op.rt());
                    self.fetch_load_slot();
                    self.gte.data_store(op.rd().0.into(), val);
                }
                // Store to COP2 control register.
                0x6 => {
                    let val = self.read_reg(op.rt());
                    self.fetch_load_slot();
                    self.gte.ctrl_store(op.rd().0.into(), val);
                }
               _ => unreachable!(),
            }
        }
    }

    /// COP3 - Coprocessor3 instruction.
    fn op_cop3(&mut self) {
        // COP3 does not exist on the Playstation 1.
        self.throw_exception(Exception::CopUnusable);
    }

    /// LB - Load byte.
    fn op_lb(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        match self.load::<Byte>(addr) {
            Ok(val) => self.add_load_slot(op.rt(), (val as i8) as u32),
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// LH - Load half word.
    fn op_lh(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        match self.load::<HalfWord>(addr) {
            Ok(val) => self.add_load_slot(op.rt(), (val as i16) as u32),
            Err(exp) => {
                if let Exception::AddressLoadError = exp {
                    self.cop0.set_reg(8, addr);
                }
                self.throw_exception(exp);
            }
        }
    }

    /// LWL - Load word left.
    ///
    /// This is used to load words which aren't 4 byte aligned. It first fetches a base value from a
    /// given register, which doesn't wait for load delays for some reason?. It then fetches the word in
    /// memory which contain the unaligned address. The result is a combination (bitwise or) of the
    /// base value and the loaded word, where the combination depend on the alignment of the
    /// address.
    fn op_lwl(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let val = if self.load_delay.reg == op.rt() {
            self.load_delay.val
        } else {
            self.read_reg(op.rt())
        };
        // Get word containing unaligned address.
        match self.load::<Word>(addr & !0x3) {
            Ok(word) => {
                // Extract 1, 2, 3, 4 bytes dependent on the address alignment.
                let val = match addr & 0x3 {
                    0 => (val & 0x00ff_ffff) | (word << 24),
                    1 => (val & 0x0000_ffff) | (word << 16),
                    2 => (val & 0x0000_00ff) | (word << 8),
                    3 => word,
                    _ => unreachable!(),
                };
                self.add_load_slot(op.rt(), val);
            }
            Err(exp) => {
                // This should never be an alignment problem, so there should be no need to store
                // the address in COP0 bad virtual address register.
                self.throw_exception(exp);
            }
        };
    }

    /// LW - Load word.
    fn op_lw(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        match self.load::<Word>(addr) {
            Ok(val) => self.add_load_slot(op.rt(), val),
            Err(exp) => {
                if let Exception::AddressLoadError = exp {
                    self.cop0.set_reg(8, addr);
                }
                self.throw_exception(exp);
            }
        }
    }

    /// LBU - Load byte unsigned.
    fn op_lbu(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        match self.load::<Byte>(addr) {
            Ok(val) => self.add_load_slot(op.rt(), val),
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// LHU - Load half word unsigned.
    fn op_lhu(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        match self.load::<Byte>(addr) {
            Ok(val) => self.add_load_slot(op.rt(), val),
            Err(exp) => {
                if let Exception::AddressLoadError = exp {
                    self.cop0.set_reg(8, addr);
                }
                self.throw_exception(exp);
            }
        }
    }

    /// LWR - Load word right.
    ///
    /// See 'op_lwl'.
    fn op_lwr(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let val = if self.load_delay.reg == op.rt() {
            self.load_delay.val
        } else {
            self.read_reg(op.rt())
        };
        match self.load::<Word>(addr & !0x3) {
            Ok(word) => {
                let val = match addr & 0x3 {
                    0 => word,
                    1 => (val & 0xff00_0000) | (word >> 8),
                    2 => (val & 0xffff_0000) | (word >> 16),
                    3 => (val & 0xffff_ff00) | (word >> 24),
                    _ => unreachable!(),
                };
                self.add_load_slot(op.rt(), val);
            }
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// SB - Store byte.
    fn op_sb(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let val = self.read_reg(op.rt());
        self.fetch_load_slot();
        if let Err(exp) = self.store::<Byte>(addr, val) {
            self.throw_exception(exp);
        }
    }

    /// SH - Store half word.
    fn op_sh(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let val = self.read_reg(op.rt());
        self.fetch_load_slot();
        if let Err(exp) = self.store::<HalfWord>(addr, val) {
            if let Exception::AddressStoreError = exp {
                self.cop0.set_reg(8, addr);
            }
            self.throw_exception(Exception::AddressStoreError);
        }
    }

    /// SWL - Store world left.
    ///
    /// This is used to store words to addresses which aren't 32-aligned. It's the  same idea
    /// as 'op_lwl' and 'op_lwr'.
    fn op_swl(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let val = self.read_reg(op.rt());
        // Get address of whole word containing unaligned address.
        let aligned = addr & !3;
        match self.load::<Word>(aligned) {
            Ok(word) => {
                let val = match addr & 3 {
                    0 => (word & 0xffff_ff00) | (val >> 24),
                    1 => (word & 0xffff_0000) | (val >> 16),
                    2 => (word & 0xff00_0000) | (val >> 8),
                    3 => word,
                    _ => unreachable!(),
                };
                self.fetch_load_slot();
                if let Err(exp) = self.store::<Word>(aligned, val) {
                    if let Exception::AddressStoreError = exp {
                        self.cop0.set_reg(8, addr);
                    }
                    self.throw_exception(Exception::AddressStoreError);
                }
            }
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// SW - Store word.
    /// Store word from target register at address from source register + signed immediate value.
    fn op_sw(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let val = self.read_reg(op.rt());
        self.fetch_load_slot();
        if let Err(exp) = self.store::<Word>(addr, val) {
            if let Exception::AddressStoreError = exp {
                self.cop0.set_reg(8, addr);
            }
            self.throw_exception(Exception::AddressStoreError);
        }
    }

    /// SWR - Store world right.
    ///
    /// See 'op_swl'.
    fn op_swr(&mut self, op: Opcode) {
        let addr = self.read_reg(op.rs()).wrapping_add(op.signed_imm());
        let val = self.read_reg(op.rt());
        let aligned = addr & !3;
        match self.load::<Word>(aligned) {
            Ok(word) => {
                let val = match addr & 3 {
                    0 => word,
                    1 => (word & 0x0000_00ff) | (val << 8),
                    2 => (word & 0x0000_ffff) | (val << 16),
                    3 => (word & 0x00ff_ffff) | (val << 24),
                    _ => unreachable!(),
                };
                self.fetch_load_slot();
                if let Err(exp) = self.store::<Word>(aligned, val) {
                    if let Exception::AddressStoreError = exp {
                        self.cop0.set_reg(8, addr);
                    }
                    self.throw_exception(Exception::AddressStoreError);
                }
            }
            Err(exp) => self.throw_exception(exp),
        }
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
    fn op_lwc2(&mut self, _: Opcode) {
        todo!()
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
    fn op_swc2(&mut self, _: Opcode) {
        todo!()
    }

    /// SWC3 - Store world in Coprocessor0.
    fn op_swc3(&mut self) {
        self.throw_exception(Exception::CopUnusable);
    }

    /// Illegal/Undefined opcode.
    fn op_illegal(&mut self) {
        self.throw_exception(Exception::ReservedInstruction);
    }
}

pub const REGISTER_NAMES: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6",
    "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp",
    "ra",
];

#[cfg(test)]
mod tests {
    use crate::bus::bios::Bios;

    use super::opcode::RegIdx;
    use super::Cpu;

    fn run_cpu(cpu: &mut Cpu) {
        loop {
            let ins = cpu.curr_ins();
            // Stop if the current instruction is break.
            if ins.op() == 0x0 && ins.special() == 0xd {
                break;
            }
            cpu.step();
        }
    }

    fn run(input: &str) -> Box<Cpu> {
        let base = 0x1fc00000;
        let code = match splst_asm::assemble(input, base) {
            Ok(code) => code,
            Err(error) => panic!("{error}"),
        };
        let bios = Bios::from_code(base, &code);
        let mut cpu = Cpu::new(bios);
        run_cpu(&mut cpu);
        cpu
    }

    #[test]
    fn zero_reg() {
        let cpu = run(r#"
            .text
                li $zero, 1
                break 0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::ZERO), 0);
    }

    #[test]
    fn branch_delay() {
        let cpu = run(r#"
                li      $v0, 0
                j       l1  
                addiu   $v0, $v0, 1
            l1:
                break   0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::V0), 1);
    }

    #[test]
    fn load_delay() {
        let cpu = run(r#"
                li      $v0, 42
                li      $s1, 43
                la      $v1, 0x0
                sw      $v0, 0($v1)
                lw      $s1, 0($v1)
                break   0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::S1), 43);
    }

    #[test]
    fn simple_loop() {
        let cpu = run(r#"
            main:
                li      $v0, 1
            l2:
                sll     $v0, $v0, 1
                slti    $v1, $v0, 1024
                bne     $v1, $zero, l2
                nop

                break   0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::V0), 1024);
    }

    #[test]
    fn sll() {
        let cpu = run(r#"
                li      $v0, 8
                sll     $v0, $v0, 2
                break   0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::V0), 8 << 2);
    }

    #[test]
    fn srl() {
        let cpu = run(r#"
                li      $v0, 8
                srl     $v0, $v0, 2
                break   0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::V0), 8 >> 2);
    }

    #[test]
    fn sra() {
        let cpu = run(r#"
                li      $v0, -8
                sra     $v0, $v0, 2
                break   0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::V0), (-8_i32 >> 2) as u32);
    }

    #[test]
    fn sllv() {
        let cpu = run(r#"
                li      $v0, 8
                li      $v1, 2
                sllv    $v0, $v0, $v1
                break   0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::V0), 8 << 2);
    }

    #[test]
    fn srlv() {
        let cpu = run(r#"
                li      $v0, 8
                li      $v1, 2
                srlv    $v0, $v0, $v1
                break   0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::V0), 8 >> 2);
    }

    #[test]
    fn jalr() {
        let cpu = run(r#"
                la      $v0, l1
                li      $ra, 0
                li      $a0, 0
                li      $a1, 0

                jalr    $ra, $v0

                li      $a0, 3
                li      $a1, 4

            l1:
                break   0
        "#);
        assert_ne!(cpu.read_reg(RegIdx::RA), 0);
        assert_eq!(cpu.read_reg(RegIdx::A0), 3);
        assert_ne!(cpu.read_reg(RegIdx::A1), 4);
    }

    #[test]
    fn bltzal() {
        let cpu = run(r#"
                li      $t0, -1
                bltzal  $t0, l1 
                nop
                li      $t0, 1
            l1:
                break   0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::T0), (-1_i32) as u32);
        assert_ne!(cpu.read_reg(RegIdx::RA), 0);
    }

    #[test]
    fn addiu() {
        let cpu = run(r#"
                li      $v0, 0
                addiu   $v0, $v0, -1

                li      $v1, -1
                addiu   $v1, $v1, 1

                break 0
        "#);
        assert_eq!(cpu.read_reg(RegIdx::V0), (-1_i32) as u32);
        assert_eq!(cpu.read_reg(RegIdx::V1), 0);
    }
}
