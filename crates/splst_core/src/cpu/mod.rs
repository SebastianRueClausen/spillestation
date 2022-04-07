//! Emulation of the MIPS R3000 used by the original Sony Playstation.
//!
//! # TODO
//!
//! - Perhaps checking for active interrupts isn't good enough, since writes to the COP0 cause and
//!   active registers can change 'irq_active' status. Checking every cycle doesn't seem to do
//!   anything for now, but herhaps there could be a problem.
//!
//! - More testing of edge cases.
//!
//! - Test that store and load functions get's inlined properly to avoid branching on Exceptions.

mod cop0;
mod gte;

pub mod irq;
pub mod opcode;

use crate::io_port::Controllers;
use crate::cdrom::Disc;
use crate::bus::bios::Bios;
use crate::bus::scratchpad::ScratchPad;
use crate::bus::{self, AddrUnit, Bus, BusMap};
use crate::schedule;
use crate::{SysTime, Debugger, VideoOutput};
use splst_util::Bit;

use cop0::{Cop0, Exception};
use gte::Gte;

use std::cell::RefCell;
use std::rc::Rc;

pub use irq::{Irq, IrqState};
pub use opcode::{Opcode, RegIdx};

#[derive(Default, Clone, Copy)]
struct DelaySlot {
    reg: RegIdx,
    /// The amount of time since startup that the delay slot is ready.
    ready: SysTime,
    val: u32,
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
    /// Set to true if the current instruction is executed in the branch delay slot, ie.
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
    /// This stores the system time since startup that the result of an multiply or divide
    /// instruction is ready since they take more than a single cycle to complete. The
    /// CPU can run while the result is being calculated, but if the result is being read
    /// before it's ready, the CPU will wait before continuing.
    hi_lo_ready: SysTime,
    /// # Registers
    ///
    /// All registers of the MIPS R3000 are essentially general purpose besides $r0 which always
    /// contains the value 0. They are however used for specific purposes depending on convention.
    ///
    /// | Number  | Name    | Usage                 |
    /// |---------|---------|-----------------------|
    /// | r0      | $zero   | Always 0              |
    /// | r1      | $at     | Reserved by assembler |
    /// | r2-r3   | $v0-$v1 | Results               |
    /// | r4-r7   | $a0-$a3 | Arguments             |
    /// | r8-r15  | $t0-$t7 | Temporaries           |
    /// | r16-r23 | $s0-$s7 | Storing               |
    /// | r24-r25 | $t8-$t9 | Temporaries           |
    /// | r26-r27 | $k0-$k1 | Reserved by kernel    |
    /// | r28     | $gp     | Global pointer        |
    /// | r29     | $sp     | Stack pointer         |
    /// | r30     | $fp     | Frame pointer         |
    /// | r31     | $ra     | Return address        |
    ///
    pub registers: [u32; 32],
    /// # Load Delay Slot
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
    /// isn't any load, it writes to the $r0 register which does nothing.
    ///
    /// The 'ready' field contains the cycle when the delayed load is ready to be read. Loads can
    /// execute in the background while the processor is doing calculations or loading instructions
    /// from memory. However, if an instruction is reading from or writing to the register in the
    /// pipeline or starting a new load from memory, then the processor will sync and pause until
    /// the load is done. This is only for loading from memory. Loading from the Coprocessors takes
    /// can run while the loading from memory.
    load_delay: DelaySlot,
    /// Memory sections KUSEG and KSEG0 are cached for instructions.
    icache: Box<[ICacheLine; 0x100]>,
    icache_misses: u64,
    pub(super) bus: Bus,
    gte: Gte,
    cop0: Cop0,
}

const PC_START_ADDRESS: u32 = 0xbfc00000;

impl Cpu {
    pub fn new(
        bios: Bios,
        renderer: Rc<RefCell<dyn VideoOutput>>,
        disc: Rc<RefCell<Disc>>,
        controllers: Rc<RefCell<Controllers>>,
    ) -> Box<Self> {
        let bus = Bus::new(
            bios,
            renderer,
            disc,
            controllers,
        );
        let icache = Box::new([ICacheLine::valid(); 0x100]);
        Box::new(Cpu {
            last_pc: 0x0,
            pc: PC_START_ADDRESS,
            next_pc: PC_START_ADDRESS + 4,
            in_branch_delay: false,
            branched: false,
            hi: 0x0,
            lo: 0x0,
            hi_lo_ready: SysTime::ZERO,
            registers: [0x0; 32],
            load_delay: DelaySlot::default(),
            gte: Gte::new(),
            cop0: Cop0::new(),
            icache,
            icache_misses: 0,
            bus,
        })
    }

    pub fn read_reg(&self, idx: RegIdx) -> u32 {
        self.registers[idx.0 as usize]
    }

    fn set_reg(&mut self, idx: RegIdx, val: u32) {
        self.registers[idx.0 as usize] = val;
        self.registers[0] = 0;
    }

    /// Load data from the bus. Must not be called when loading code.
    fn load<T: AddrUnit>(&mut self, addr: u32) -> Result<(T, SysTime), Exception> {
        self.bus.schedule.skip_to(self.load_delay.ready);

        if !bus::is_aligned_to::<T>(addr) {
            self.cop0.set_reg(8, addr);
            return Err(Exception::AddressLoadError);
        }

        let addr = bus::regioned_addr(addr);

        if let Some(offset) = ScratchPad::offset(addr) {
            Ok((self.bus.scratchpad.load::<T>(offset), SysTime::ZERO))
        } else {
            self.bus.load::<T>(addr).ok_or(Exception::BusDataError)
        }
    }

    /// Load an instruction from memory.
    fn load_code<T: AddrUnit>(&mut self, addr: u32) -> Result<T, Exception> {
        if !bus::is_aligned_to::<T>(addr) {
            self.cop0.set_reg(8, addr);
            return Err(Exception::AddressLoadError);
        }

        let addr = bus::regioned_addr(addr);

        self.bus
            .load::<T>(addr)
            .map(|(val, time)| {
                self.bus.schedule.advance(time);
                val
            })
            .ok_or(Exception::BusInstructionError)
    }

    fn store<T: AddrUnit>(&mut self, addr: u32, val: T) -> Result<(), Exception> {
        if !bus::is_aligned_to::<T>(addr) {
            self.cop0.set_reg(8, addr);
            return Err(Exception::AddressStoreError);
        }

        let addr = bus::regioned_addr(addr);

        if !self.cop0.cache_isolated() {
            self.bus
                .store(addr, val)
                .ok_or(Exception::BusDataError)
        } else if self.bus.cache_ctrl.icache_enabled() {
            let line_idx = addr.bit_range(4, 11) as usize;
            let mut line = self.icache[line_idx];

            if self.bus.cache_ctrl.tag_test_enabled() {
                line.invalidate();
            } else {
                let index = addr.bit_range(2, 3) as usize;
                line.data[index] = val.as_u32();
            }

            self.icache[line_idx] = line;
            Ok(())
        } else {
            warn!("store with cache isolated but not enabled");
            Ok(())
        }
    }

    /// Add a load from memory to the CPU pipeline. It fetches the pending load from the pipeline
    /// if there is any. If the pipeline already contains a load to the same register, then the
    /// value get's replaced with the new one, effectively throwing away the old value.
    ///
    /// When loading from memory the CPU has to wait for the previous load to be done before the
    /// new load can begin.
    fn pipeline_bus_load(&mut self, reg: RegIdx, val: u32, time: SysTime) {
        // Check if the current load in the pipeline is different from the new one. If there aren't
        // any loads then 'self.load_delay.reg' points to the zero register.
        let diff = (self.load_delay.reg != reg) as u8;

        // If the load in the pipeline is the same as 'reg' then 'lreg' points to the zero
        // register, meaning writing to it would do nothing.
        let lreg = RegIdx::from(self.load_delay.reg.0 * diff);

        // This works since 'skip_to' ignores cycles less than the current cycle (take max of the
        // two).
        self.bus.schedule.skip_to(self.load_delay.ready);
        self.set_reg(lreg, self.load_delay.val);

        let ready = self.bus.schedule.since_startup() + time;
        self.load_delay = DelaySlot { reg, val, ready };
    }

    /// Almost the same as 'pipeline_bus_load' but doesn't have to wait for any previous loads to
    /// come in. It will however still execute the next instruction in the load delay slot.
    fn pipeline_cop_load(&mut self, reg: RegIdx, val: u32) {
        let diff = (self.load_delay.reg != reg) as u8;

        let lreg = RegIdx::from(self.load_delay.reg.0 * diff);

        self.set_reg(lreg, self.load_delay.val);
        self.load_delay = DelaySlot { reg, val, ready: SysTime::ZERO };
    }

    /// Access a register, meaning that it's either gonna be get written to or read from. If there
    /// is a load in the pipeline to the same register, the processor has to wait for it to be
    /// done.
    fn access_reg(&mut self, reg: RegIdx) -> RegIdx {
        let same = self.load_delay.reg == reg;
        self.bus.schedule.skip_to(self.load_delay.ready * same as u64);
        reg
    }

    /// Fetch pending load if there is any. It doesn't wait for the load to be finished, so the
    /// register will contain the value before it's done loading. Only if the register is actually
    /// read or written to will the CPU wait through 'access_reg'.
    fn fetch_load_slot(&mut self) {
        self.set_reg(self.load_delay.reg, self.load_delay.val);
        self.load_delay = DelaySlot::default();
    }

    /// Add result to hi and lo register.
    fn add_pending_hi_lo(&mut self, time: SysTime, hi: u32, lo: u32) {
        self.hi_lo_ready = self.bus.schedule.since_startup() + time;
        self.hi = hi;
        self.lo = lo;
    }

    /// Wait for hi lo results.
    fn fetch_pending_hi_lo(&mut self) {
        self.bus.schedule.skip_to(self.hi_lo_ready);
    }

    /// Branch to relative offset.
    fn branch(&mut self, offset: u32) {
        // Offset is shifted 2 bits since PC addresses must be 32-bit aligned anyway.
        self.next_pc = self.pc.wrapping_add(offset << 2);
        self.branched = true;
    }

    /// Jump to absolute address.
    fn jump(&mut self, address: u32) {
        self.next_pc = address;
        self.branched = true;
    }

    /// Start handeling an exception, and jumps to exception handling code in bios.
    fn throw_exception(&mut self, ex: Exception) {
        trace!("Exception thrown: {:?}", ex);

        let pc = self.cop0.enter_exception(
            &mut self.bus.schedule,
            self.last_pc,
            self.in_branch_delay,
            ex,
        );
        
        self.pc = pc;
        self.next_pc = pc.wrapping_add(4);
    }

    fn irq_pending(&self) -> bool {
        let active = (self.bus.irq_state.active() as u32) << 10;
        let cause = self.cop0.read_reg(13) | active;
        let active = self.cop0.read_reg(12) & cause & 0xffff00;

        self.cop0.irq_enabled() && active != 0
    }

    pub fn curr_ins(&mut self) -> Opcode {
        Opcode::new(self.bus.peek(self.pc).unwrap_or(0xffff_ffff))
    }

    pub fn icache_misses(&mut self) -> u64 {
        self.icache_misses
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

    fn fetch_cachline(
        &mut self,
        line: &mut ICacheLine,
        idx: usize,
        addr: u32,
    ) -> Result<(), Exception> {
        self.bus.schedule.advance(SysTime::new(4 - idx as u64));
        for (i, j) in (idx..4).enumerate() {
            line.data[j] = self.load_code(addr + (i as u32 * 4))?;
        }
        Ok(())
    }

    /// Check if there is any irq pending and throw exception if there is.
    pub fn check_for_pending_irq(&mut self) {
        if self.irq_pending() {
            self.next_pc();
            self.fetch_load_slot();
            self.throw_exception(Exception::Interrupt);
        }
    }

    /// Fetch and execute next instruction.
    pub fn step(&mut self, dbg: &mut impl Debugger) {
        match self.bus.schedule.get_pending_event() {
            Some(event) => {
                schedule::trigger_event(self, event);
            }
            // Run the next instruction if there is no event this cycle.
            None => {
                let addr = self.next_pc();

                dbg.instruction_load(addr);

                if bus::addr_cached(addr) && self.bus.cache_ctrl.icache_enabled() {
                    let tag = addr.bit_range(12, 30);
                    let word_idx = addr.bit_range(2, 3) as usize;
                    let line_idx = addr.bit_range(4, 11) as usize;

                    let mut line = self.icache[line_idx];

                    if line.tag() != tag || line.valid_word_idx() > word_idx {
                        self.bus.schedule.skip_to(self.load_delay.ready);

                        match self.fetch_cachline(&mut line, word_idx, addr) {
                            Ok(()) => self.exec(dbg, Opcode::new(line.data[word_idx])),
                            Err(exp) => self.throw_exception(exp),
                        }

                        line.set_tag(addr);

                        self.icache[line_idx] = line;
                        self.icache_misses += 1;
                    } else {
                        self.exec(dbg, Opcode::new(line.data[word_idx]));
                    }
                } else {
                    self.bus.schedule.skip_to(self.load_delay.ready);

                    // Cache misses take about 4 cycles.
                    self.bus.schedule.advance(SysTime::new(4));

                    match self.load_code::<u32>(addr) {
                        Ok(val) => self.exec(dbg, Opcode::new(val)),
                        Err(exp) => self.throw_exception(exp),
                    }
                }
                self.bus.schedule.advance(SysTime::new(1));
            }
        }
    }

    /// Execute opcode.
    fn exec(&mut self, dbg: &mut impl Debugger, opcode: Opcode) {
        match opcode.op() {
            0x0 => match opcode.special() {
                0x00 => self.op_sll(opcode),
                0x02 => self.op_srl(opcode),
                0x03 => self.op_sra(opcode),
                0x04 => self.op_sllv(opcode),
                0x06 => self.op_srlv(opcode),
                0x07 => self.op_srav(opcode),
                0x08 => self.op_jr(opcode),
                0x09 => self.op_jalr(opcode),
                0x0c => self.op_syscall(),
                0x0d => self.op_break(),
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
            0x01 => self.op_bcondz(opcode),
            0x02 => self.op_j(opcode),
            0x03 => self.op_jal(opcode),
            0x04 => self.op_beq(opcode),
            0x05 => self.op_bne(opcode),
            0x06 => self.op_blez(opcode),
            0x07 => self.op_bgtz(opcode),
            0x08 => self.op_addi(opcode),
            0x09 => self.op_addiu(opcode),
            0x0a => self.op_slti(opcode),
            0x0b => self.op_sltui(opcode),
            0x0c => self.op_andi(opcode),
            0x0d => self.op_ori(opcode),
            0x0e => self.op_xori(opcode),
            0x0f => self.op_lui(opcode),
            0x10 => self.op_cop0(opcode),
            0x11 => self.op_cop1(),
            0x12 => self.op_cop2(opcode),
            0x13 => self.op_cop3(),
            0x20 => self.op_lb(dbg, opcode),
            0x21 => self.op_lh(dbg, opcode),
            0x22 => self.op_lwl(dbg, opcode),
            0x23 => self.op_lw(dbg, opcode),
            0x24 => self.op_lbu(dbg, opcode),
            0x25 => self.op_lhu(dbg, opcode),
            0x26 => self.op_lwr(dbg, opcode),
            0x28 => self.op_sb(dbg, opcode),
            0x29 => self.op_sh(dbg, opcode),
            0x2a => self.op_swl(dbg, opcode),
            0x2b => self.op_sw(dbg, opcode),
            0x2e => self.op_swr(dbg, opcode),
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
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let val = self.read_reg(rt) << op.shift();

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// SRL - Shift right logical. Same as SRA, but unsigned.
    fn op_srl(&mut self, op: Opcode) {
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let val = self.read_reg(rt) >> op.shift();

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// SRA - Shift right arithmetic.
    fn op_sra(&mut self, op: Opcode) {
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let val = self.read_reg(rt) as i32;
        let val = val >> op.shift();

        self.fetch_load_slot();
        self.set_reg(rd, val as u32);
    }

    /// SLLV - Shift left logical variable.
    fn op_sllv(&mut self, op: Opcode) {
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());
        let rs = self.access_reg(op.rs());

        let val = self.read_reg(rt) << self.read_reg(rs).bit_range(0, 4);

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// SRLV - Shift right logical variable.
    fn op_srlv(&mut self, op: Opcode) {
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());
        let rs = self.access_reg(op.rs());

        let val = self.read_reg(rt) >> self.read_reg(rs).bit_range(0, 4);

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// SRAV - Shift right arithmetic variable.
    fn op_srav(&mut self, op: Opcode) {
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());
        let rs = self.access_reg(op.rs());

        let val = self.read_reg(rt) as i32;
        let val = val >> self.read_reg(rs).bit_range(0, 4);

        self.fetch_load_slot();
        self.set_reg(rd, val as u32);
    }

    /// JR - Jump register.
    fn op_jr(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());

        self.jump(self.read_reg(rs));
        self.fetch_load_slot();
    }

    /// JALR - Jump and link register.
    fn op_jalr(&mut self, op: Opcode) {
        let pc = self.next_pc;
        let rs = self.access_reg(op.rs());
        let rd = self.access_reg(op.rd());

        self.jump(self.read_reg(rs));

        self.fetch_load_slot();
        self.set_reg(rd, pc);
    }

    // TODO: Add more syscall commands.
    fn syscall_trace(&mut self) {
        match self.read_reg(RegIdx::V0) {
            1 => {
                trace!("syscall: print {}", self.read_reg(RegIdx::A0));
            }
            2 | 3 => {
                warn!("syscall: print float");
            }
            4 => {
                let mut addr = self.read_reg(RegIdx::A0);
                let mut print = String::new();
                loop {
                    let c = match self.load::<u8>(addr) {
                        Ok((val, _)) => (val as u8) as char,
                        Err(_) => break,
                    };
                    if c == '\n' {
                        break;
                    }
                    print.push(c);
                    addr += 1;
                }
                debug!("syscall: print \"{}\"", print);
            }
            5 => {
                debug!("syscall: read integer")
            }
            6 | 7 => {
                trace!("syscall: read float")
            }
            8 => {
                debug!("syscall: read string");
            }
            9 => {
                debug!("syscall: allocate {} bytes", self.read_reg(RegIdx::A0));
            }
            10 => {
                debug!("syscall: terminate");
            }
            val => {
                debug!("syscall: type {}", val)
            }
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
        let rd = self.access_reg(op.rd());

        self.fetch_pending_hi_lo();

        self.fetch_load_slot();
        self.set_reg(rd, self.hi);
    }

    /// MTHI - Move to high.
    fn op_mthi(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());

        self.hi = self.read_reg(rs);
        self.fetch_load_slot();
    }

    /// MFLO - Move from low.
    fn op_mflo(&mut self, op: Opcode) {
        let rd = self.access_reg(op.rd());

        self.fetch_pending_hi_lo();

        self.fetch_load_slot();
        self.set_reg(rd, self.lo);
    }

    /// MTLO - Move to low.
    fn op_mtlo(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());

        self.lo = self.read_reg(rs);

        self.fetch_load_slot();
    }

    /// # MULT - Signed multiplication
    ///
    /// Multiplication takes different amount of cycles to complete dependent on the size of the
    /// inputs numbers.
    fn op_mult(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let lhs = self.read_reg(rs) as i32;
        let rhs = self.read_reg(rt) as i32;

        let time = match if lhs < 0 { !lhs } else { lhs }.leading_zeros() {
            00..=11 => SysTime::new(13),
            12..=20 => SysTime::new(9),
            _ => SysTime::new(7),
        };

        let val = i64::from(lhs) * i64::from(rhs);
        let val = val as u64;

        self.fetch_load_slot();
        self.add_pending_hi_lo(time, (val >> 32) as u32, val as u32);
    }

    /// MULTU - Unsigned multiplication.
    fn op_multu(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let lhs = self.read_reg(rs);
        let rhs = self.read_reg(rt);

        let time = match lhs {
            0x00000000..=0x000007ff => SysTime::new(13),
            0x00000800..=0x000fffff => SysTime::new(9),
            _ => SysTime::new(7),
        };

        let val = u64::from(lhs) * u64::from(rhs);

        self.fetch_load_slot();
        self.add_pending_hi_lo(time, (val >> 32) as u32, val as u32);
    }

    /// # DIV - Signed division
    ///
    /// It always takes 36 cycles to complete. It doesn't throw an expception if dividing by 0, but
    /// instead returns garbage values.
    fn op_div(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let lhs = self.read_reg(rs) as i32;
        let rhs = self.read_reg(rt) as i32;

        self.fetch_load_slot();

        if rhs == 0 {
            let lo: u32 = if lhs < 0 { 1 } else { 0xffff_ffff };
            self.add_pending_hi_lo(SysTime::new(36), lhs as u32, lo);
        } else if rhs == -1 && lhs as u32 == 0x8000_0000 {
            self.add_pending_hi_lo(SysTime::new(36), 0, 0x8000_0000);
        } else {
            let hi = lhs % rhs;
            let lo = lhs / rhs;

            self.add_pending_hi_lo(SysTime::new(36), hi as u32, lo as u32);
        }
    }

    /// DIVU - Unsigned division.
    fn op_divu(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let lhs = self.read_reg(rs);
        let rhs = self.read_reg(rt);

        self.fetch_load_slot();

        if rhs == 0 {
            self.add_pending_hi_lo(SysTime::new(36), lhs, 0xffff_ffff);
        } else {
            self.add_pending_hi_lo(SysTime::new(36), lhs % rhs, lhs / rhs);
        }
    }

    /// ADD - Add signed.
    fn op_add(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let lhs = self.read_reg(rs) as i32;
        let rhs = self.read_reg(rt) as i32;

        self.fetch_load_slot();

        if let Some(val) = lhs.checked_add(rhs) {
            self.set_reg(rd, val as u32);
        } else {
            self.throw_exception(Exception::ArithmeticOverflow);
        }
    }

    /// ADDU - Add unsigned.
    fn op_addu(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let lhs = self.read_reg(rs);
        let rhs = self.read_reg(rt);

        let val = lhs.wrapping_add(rhs);

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// SUB - Signed subtraction.
    fn op_sub(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let lhs = self.read_reg(rs) as i32;
        let rhs = self.read_reg(rt) as i32;

        self.fetch_load_slot();

        if let Some(val) = lhs.checked_sub(rhs) {
            self.set_reg(rd, val as u32);
        } else {
            self.throw_exception(Exception::ArithmeticOverflow);
        }
    }

    /// SUBU - Subtract unsigned.
    fn op_subu(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let lhs = self.read_reg(rs);
        let rhs = self.read_reg(rt);

        let val = lhs.wrapping_sub(rhs);

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// AND - Bitwise and.
    fn op_and(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let val = self.read_reg(rs) & self.read_reg(rt);

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// OR - Bitwise or.
    fn op_or(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let val = self.read_reg(rs) | self.read_reg(rt);

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// XOR - Bitwise exclusive or.
    fn op_xor(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let val = self.read_reg(rs) ^ self.read_reg(rt);

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// NOR - Bitwise not or.
    fn op_nor(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let val = !(self.read_reg(rs) | self.read_reg(rt));

        self.fetch_load_slot();
        self.set_reg(rd, val);
    }

    /// SLT - Set if less than.
    fn op_slt(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let lhs = self.read_reg(rs) as i32;
        let rhs = self.read_reg(rt) as i32;

        let val = lhs < rhs;

        self.fetch_load_slot();
        self.set_reg(rd, val as u32);
    }

    /// SLTU - Set if less than unsigned.
    fn op_sltu(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());
        let rd = self.access_reg(op.rd());

        let val = self.read_reg(rs) < self.read_reg(rt);

        self.fetch_load_slot();
        self.set_reg(rd, val as u32);
    }

    /// # BCONDZ - Conditional branching
    ///
    /// Multiple conditional branch instructions combined into on opcode. If bit 16 of the opcode
    /// is set, then it set's the return value register. If bits 17..20 of the opcode equals 0x80,
    /// then it branches if the value is greater than or equal to zero, otherwise it branches if
    /// the values is less than zero.
    ///
    /// - BLTZ: Branch if less than zero.
    /// - BLTZAL: Branch if less than zero and set return register.
    /// - BGEZ: Branch if greater than or equal to zero.
    /// - BGEZAL: Branch if greater than or equal to zero and set return register.
    fn op_bcondz(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());

        let val = self.read_reg(rs) as i32;
        let cond = (val < 0) as u32;

        // If the instruction is to test greater or equal zero, we just
        // xor cond, since that's the oposite result.
        let cond = cond ^ op.bgez() as u32;

        self.fetch_load_slot();

        // Set return register if required.
        if op.update_ra_on_branch() {
            self.set_reg(RegIdx::RA, self.next_pc);
        }

        if cond != 0 {
            self.branch(op.signed_imm());
        }
    }

    /// J - Jump.
    fn op_j(&mut self, op: Opcode) {
        self.jump((self.pc & 0xf000_0000) | (op.target() << 2));

        self.fetch_load_slot();
    }

    /// JAL - Jump and link.
    fn op_jal(&mut self, op: Opcode) {
        let pc = self.next_pc;
        self.access_reg(RegIdx::RA);

        self.op_j(op);
        self.set_reg(RegIdx::RA, pc);
    }

    /// BEQ - Branch if equal.
    fn op_beq(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        if self.read_reg(rs) == self.read_reg(rt) {
            self.branch(op.signed_imm());
        }

        self.fetch_load_slot();
    }

    /// BNE - Branch if not equal.
    fn op_bne(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        if self.read_reg(rs) != self.read_reg(rt) {
            self.branch(op.signed_imm());
        }

        self.fetch_load_slot();
    }

    /// BLEZ - Branch if less than or equal to zero.
    fn op_blez(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let val = self.read_reg(rs) as i32;

        if val <= 0 {
            self.branch(op.signed_imm());
        }

        self.fetch_load_slot();
    }

    /// BGTZ - Branch if greater than zero.
    fn op_bgtz(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let val = self.read_reg(rs) as i32;

        if val > 0 {
            self.branch(op.signed_imm());
        }

        self.fetch_load_slot();
    }

    /// ADDI - Add immediate signed.
    fn op_addi(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let val = self.read_reg(rs) as i32;

        self.fetch_load_slot();

        if let Some(val) = val.checked_add(op.signed_imm() as i32) {
            self.set_reg(rt, val as u32);
        } else {
            self.throw_exception(Exception::ArithmeticOverflow);
        }
    }

    /// # ADDUI - Add immediate unsigned
    ///
    /// Actually adding a signed int to target register, not unsigned.
    /// Unsigned in this case just means wrapping on overflow.
    fn op_addiu(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let val = self.read_reg(rs).wrapping_add(op.signed_imm());

        self.fetch_load_slot();
        self.set_reg(rt, val);
    }

    /// SLTI - Set if less than immediate signed.
    fn op_slti(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let val = (self.read_reg(rs) as i32) < (op.signed_imm() as i32);

        self.fetch_load_slot();
        self.set_reg(rt, val as u32);
    }

    /// SLTUI - Set if less than immediate unsigned.
    fn op_sltui(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let val = self.read_reg(rs) < op.signed_imm();

        self.fetch_load_slot();
        self.set_reg(rt, val as u32);
    }

    /// ANDI - Bitwise and immediate.
    fn op_andi(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let val = self.read_reg(rs) & op.imm();

        self.fetch_load_slot();
        self.set_reg(rt, val);
    }

    /// ORI - Bitwise or immediate.
    fn op_ori(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let val = self.read_reg(rs) | op.imm();

        self.fetch_load_slot();
        self.set_reg(rt, val);
    }

    /// XORI - Bitwise exclusive Or immediate.
    fn op_xori(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let val = self.read_reg(rs) ^ op.imm();

        self.fetch_load_slot();
        self.set_reg(rt, val);
    }

    /// LUI - Load upper immediate.
    fn op_lui(&mut self, op: Opcode) {
        let rt = self.access_reg(op.rt());
        let val = op.imm() << 16;

        self.fetch_load_slot();
        self.set_reg(rt, val);
    }

    /// COP0 - Coprocessor0 instruction.
    fn op_cop0(&mut self, op: Opcode) {
        match op.cop_op() {
            // MFC0 - Move from Co-Processor0.
            0x0 => {
                let reg = op.rd().0;
                let rt = self.access_reg(op.rt());

                if reg > 15 {
                    self.throw_exception(Exception::ReservedInstruction);
                } else {
                    self.pipeline_cop_load(rt, self.cop0.read_reg(reg.into()));
                }
            }
            // MTC0 - Move to Co-Processor0.
            0x4 => {
                let rt = self.access_reg(op.rt());

                self.fetch_load_slot();
                self.cop0.set_reg(op.rd().0.into(), self.read_reg(rt));
            }
            // RFE - Restore from exception.
            0x10 => {
                self.fetch_load_slot();
                self.cop0.exit_exception(&mut self.bus.schedule);
            }
            _ => {
                unreachable!();
                // self.throw_exception(Exception::ReservedInstruction)
            }
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
            self.fetch_load_slot();
            self.gte.cmd(op.0);
        } else {
            match cop {
                // Load from COP2 data register.
                0x0 => {
                    let rt = self.access_reg(op.rt());
                    let val = self.gte.data_load(op.rd().0.into());

                    self.pipeline_cop_load(rt, val);
                }
                // Load from COP2 control register.
                0x2 => {
                    let rt = self.access_reg(op.rt());
                    let val = self.gte.ctrl_load(op.rd().0.into());

                    self.pipeline_cop_load(rt, val);
                }
                // Store to COP2 data register.
                0x4 => {
                    let rt = self.access_reg(op.rt());
                    let val = self.read_reg(rt);

                    self.fetch_load_slot();
                    self.gte.data_store(op.rd().0.into(), val);
                }
                // Store to COP2 control register.
                0x6 => {
                    let rt = self.access_reg(op.rt());
                    let val = self.read_reg(rt);

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
    fn op_lb(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        dbg.data_load(addr);

        match self.load::<u8>(addr) {
            Ok((val, time)) => {
                let val = val as i8;
                self.pipeline_bus_load(rt, val as u32, time)
            }
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// LH - Load half word.
    fn op_lh(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        dbg.data_load(addr);

        match self.load::<u16>(addr) {
            Err(exp) => self.throw_exception(exp),
            Ok((val, time)) => {
                let val = val as i16;
                self.pipeline_bus_load(rt, val as u32, time)
            }
        }
    }

    /// # LWL - Load word left
    ///
    /// This is used to load words which aren't 4 byte aligned. It first fetches a base value from a
    /// given register, which doesn't wait for load delays for some reason?. It then fetches the word in
    /// memory which contain the unaligned address. The result is a combination (bitwise or) of the
    /// base value and the loaded word, where the combination depend on the alignment of the
    /// address.
    fn op_lwl(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());
        let aligned = addr & !0x3;

        dbg.data_load(aligned);

        let val = if self.load_delay.reg == rt {
            debug_assert_ne!(self.load_delay.reg, RegIdx::ZERO);
            self.load_delay.val
        } else {
            self.read_reg(rt)
        };

        // Get word containing unaligned address.
        match self.load::<u32>(aligned) {
            Ok((word, time)) => {
                // Extract 1, 2, 3, 4 bytes dependent on the address alignment.
                let val = match addr & 0x3 {
                    0 => (val & 0x00ff_ffff) | (word << 24),
                    1 => (val & 0x0000_ffff) | (word << 16),
                    2 => (val & 0x0000_00ff) | (word << 8),
                    3 => word,
                    _ => unreachable!(),
                };

                self.pipeline_bus_load(rt, val, time);
            }
            Err(exp) => {
                // This should never be an alignment problem, so there should be no need to store
                // the address in COP0 bad virtual address register.
                self.throw_exception(exp);
            }
        };
    }

    /// LW - Load word.
    fn op_lw(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        dbg.data_load(addr);

        match self.load::<u32>(addr) {
            Ok((val, time)) => self.pipeline_bus_load(rt, val, time),
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// LBU - Load byte unsigned.
    fn op_lbu(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        dbg.data_load(addr);

        match self.load::<u8>(addr) {
            Ok((val, time)) => self.pipeline_bus_load(rt, val.into(), time),
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// LHU - Load half word unsigned.
    fn op_lhu(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        dbg.data_load(addr);

        match self.load::<u16>(addr) {
            Ok((val, time)) => self.pipeline_bus_load(rt, val.into(), time),
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// # LWR - Load word right
    ///
    /// See 'op_lwl'.
    fn op_lwr(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        let aligned = addr & !0x3;
        dbg.data_load(aligned);

        let val = if self.load_delay.reg == rt {
            debug_assert_ne!(self.load_delay.reg, RegIdx::ZERO);
            self.load_delay.val
        } else {
            self.read_reg(rt)
        };

        match self.load::<u32>(aligned) {
            Ok((word, time)) => {
                let val = match addr & 0x3 {
                    0 => word,
                    1 => (val & 0xff00_0000) | (word >> 8),
                    2 => (val & 0xffff_0000) | (word >> 16),
                    3 => (val & 0xffff_ff00) | (word >> 24),
                    _ => unreachable!(),
                };

                self.pipeline_bus_load(rt, val, time);
            }
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// SB - Store byte.
    fn op_sb(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        dbg.data_store(addr);

        let val = self.read_reg(rt);
        self.fetch_load_slot();

        if let Err(exp) = self.store::<u8>(addr, val as u8) {
            self.throw_exception(exp);
        }
    }

    /// SH - Store half word.
    fn op_sh(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        dbg.data_store(addr);

        let val = self.read_reg(rt);
        self.fetch_load_slot();

        if let Err(exp) = self.store::<u16>(addr, val as u16) {
            self.throw_exception(exp);
        }
    }

    /// # SWL - Store world left.
    ///
    /// This is used to store words to addresses which aren't 32 bit aligned. It's the 
    /// same idea as 'op_lwl' and 'op_lwr'.
    fn op_swl(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        let val = self.read_reg(rt);

        // Get address of whole word containing unaligned address.
        let aligned = addr & !3;

        dbg.data_store(aligned);
        dbg.data_load(aligned);

        match self.load::<u32>(aligned) {
            Ok((word, time)) => {
                let val = match addr & 3 {
                    0 => (word & 0xffff_ff00) | (val >> 24),
                    1 => (word & 0xffff_0000) | (val >> 16),
                    2 => (word & 0xff00_0000) | (val >> 8),
                    3 => val,
                    _ => unreachable!(),
                };

                self.bus.schedule.advance(time);
                self.fetch_load_slot();

                if let Err(exp) = self.store::<u32>(aligned, val) {
                    self.throw_exception(exp);
                }
            }
            Err(exp) => self.throw_exception(exp),
        }
    }

    /// # SW - Store word
    ///
    /// Store word from target register at address from source register + signed immediate value.
    fn op_sw(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        dbg.data_store(addr);

        let val = self.read_reg(rt);
        self.fetch_load_slot();

        if let Err(exp) = self.store::<u32>(addr, val) {
            self.throw_exception(exp);
        }
    }

    /// # SWR - Store world right
    ///
    /// See 'op_swl'.
    fn op_swr(&mut self, dbg: &mut impl Debugger, op: Opcode) {
        let rs = self.access_reg(op.rs());
        let rt = self.access_reg(op.rt());

        let addr = self.read_reg(rs).wrapping_add(op.signed_imm());

        let val = self.read_reg(rt);

        let aligned = addr & !3;

        dbg.data_store(aligned);
        dbg.data_load(aligned);

        match self.load::<u32>(aligned) {
            Ok((word, time)) => {
                let val = match addr & 3 {
                    0 => val,
                    1 => (word & 0x0000_00ff) | (val << 8),
                    2 => (word & 0x0000_ffff) | (val << 16),
                    3 => (word & 0x00ff_ffff) | (val << 24),
                    _ => unreachable!(),
                };

                self.bus.schedule.advance(time);
                self.fetch_load_slot();

                if let Err(ex) = self.store::<u32>(aligned, val) {
                    self.throw_exception(ex);
                }
            }
            Err(ex) => self.throw_exception(ex),
        }
    }

    /// LWC0 - Load word in Coprocessor0
    fn op_lwc0(&mut self) {
        // This doesn't work on the COP0.
        self.throw_exception(Exception::CopUnusable);
    }

    /// LWC1 - Load word in Coprocessor1.
    fn op_lwc1(&mut self) {
        self.throw_exception(Exception::CopUnusable);
    }

    /// LWC2 - Load word in Coprocessor2.
    fn op_lwc2(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());

        let addr = self
            .read_reg(rs)
            .wrapping_add(op.signed_imm());

        self.fetch_load_slot();

        match self.load::<u32>(addr) {
            Ok((val, time)) => {
                self.bus.schedule.advance(time);
                self.gte.data_store(op.rt().0 as u32, val);
            }
            Err(ex) => {
                self.throw_exception(ex);
            }
        };
    }

    /// LWC3 - Load word in Coprocessor3.
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
    fn op_swc2(&mut self, op: Opcode) {
        let rs = self.access_reg(op.rs());

        let val = self.gte.data_load(op.rt().0.into());

        let addr = self
            .read_reg(rs)
            .wrapping_add(op.signed_imm());

        self.fetch_load_slot();

        if let Err(ex) = self.store::<u32>(addr, val) {
            self.throw_exception(ex);
        }
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

#[derive(Clone, Copy)]
struct ICacheLine {
    tag: u32,
    data: [u32; 4],
}

impl ICacheLine {
    fn valid() -> Self {
        Self {
            tag: 0x0,
            data: [0xdeadbeef; 4],
        }
    }

    fn tag(&self) -> u32 {
        self.tag.bit_range(12, 30)
    }

    fn valid_word_idx(&self) -> usize {
        self.tag.bit_range(2, 4) as usize
    }

    fn set_tag(&mut self, pc: u32) {
        self.tag = pc & 0x7fff_f00c;
    }

    fn invalidate(&mut self) {
        self.tag |= 0x10;
    }
}

pub const REGISTER_NAMES: [&str; 32] = [
    "zero", "at", "v0", "v1", "a0", "a1", "a2", "a3", "t0", "t1", "t2", "t3", "t4", "t5", "t6",
    "t7", "s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "t8", "t9", "k0", "k1", "gp", "sp", "fp",
    "ra",
];
