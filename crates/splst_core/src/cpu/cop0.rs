//! # Coprocessor 0.
//!
//! Used to handle CPU exceptions in the Playstation 1. It can also handle virtual memory,
//! but that isn't used by the playstation 1.

use splst_util::{Bit, BitSet};
use crate::schedule::{Schedule, Event};

#[derive(Debug, PartialEq, Eq)]
pub enum Exception {
    /// An interrupt has occured.
    Interrupt = 0x0,
    /// Loading data at an unaligned address.
    AddressLoadError = 0x4,
    /// Storing data at an unaligned address.
    AddressStoreError = 0x5,
    /// Trying to load an instruction from an invalid address.
    BusInstructionError = 0x6,
    /// Trying to load or store data at an invalid address.
    BusDataError = 0x7,
    /// Thrown via [`super::Cpu::op_syscall`].
    Syscall = 0x8,
    /// Thrown via [`super::Cpu::op_break`].
    Breakpoint = 0x9,
    /// Thrown when calling a illegal instruction.
    ReservedInstruction = 0xa,
    /// Thrown when calling an instruction for an unavailable coprocessor.
    CopUnusable = 0xb,
    /// Throwm by some instruction if an overload has occured.
    ArithmeticOverflow = 0xc,
}

pub struct Cop0 {
    /// # COP0 registers
    ///
    /// Most aren't used by any game, so it would maybe be a better idea just to
    /// only implement the required onces.  
    ///
    /// | Number | Name     | Usage                       |
    /// |--------|----------|-----------------------------|
    /// | 0..2   | Na       | -                           |
    /// | 3      | bpc      | Breakpoint on execution     |
    /// | 5      | bda      | Breakpoint on data access   |
    /// | 6      | jumpdest | Memorized jump address      |
    /// | 7      | dcic     | Breakpoint control          |
    /// | 8      | badvaddr | Bad virtual address.        |
    /// | 9      | bdam     | Data access breakpoint mask |
    /// | 10     | Na       | -                           |
    /// | 11     | bpcm     | Execute breakpoint mask     |
    /// | 12     | sr       | Status register             |
    /// | 13     | cause    | Exception type              |
    /// | 14     | epc      | Return address from trap    |
    /// | 15     | prid     | Processor ID                |
    /// 
    regs: [u32; 16],
}

impl Default for Cop0 {
    fn default() -> Self {
        Self { regs: REGISTER_VALUES }
    }
}

impl Cop0 {
    /// If the scratchpad is enabled.
    #[inline]
    pub fn cache_isolated(&self) -> bool {
        self.regs[12].bit(16)
    }

    /// Describes if boot exception vectors is in RAM.
    #[inline]
    fn bev_in_ram(&self) -> bool {
        self.regs[12].bit(22)
    }

    #[inline]
    pub fn irq_enabled(&self) -> bool {
        self.regs[12].bit(0)
    }

    pub fn set_reg(&mut self, reg: u32, value: u32) {
        self.regs[reg as usize] = value;
    }

    pub fn read_reg(&self, reg: u32) -> u32 {
        if reg == 8 {
            trace!("bad virtual address register read");
        }
        self.regs[reg as usize]
    }

    /// Start handling an exception. It updates the status register to disable interrupt and sets
    /// the processor to kernel mode. It then updates the CAUSE register to store the type of
    /// exception and the EPC register to store the address of the instruction currently being
    /// executed (or the one before that if the CPU is in a branch delay slot).
    ///
    /// # Returns
    ///
    /// The address of the code to handle the exception, which may depend on if the boot exception
    /// vectors have been transfered to RAM.
    pub fn enter_exception(
        &mut self,
        schedule: &mut Schedule,
        last_pc: u32,
        in_delay: bool,
        ex: Exception,
    ) -> u32 {
        // Remember bits 0..5 of the status register, which keep track of interrupt and
        // kernel/user mode flags. Bits 0..1 keep track of the current flags, bits 2..3 keeps
        // track of the last flags, and bits 4..5 the ones before that.
        let flags = self.regs[12].bit_range(0, 5);

        // When entering and exception, two 0 are appended to these bits, which disables interrupts
        // and sets the CPU to kernel mode.
        self.regs[12] = self.regs[12].set_bit_range(0, 5, flags << 2);

        // Set CAUSE register to the exception type.
        self.regs[13] = self.regs[13].set_bit_range(2, 6, ex as u32);

        // If the CPU is in a branch delay slot, EPC is set to one instruction behind the last pc.
        // Bit 31 of CAUSE is also set.
        let addr = if in_delay { last_pc.wrapping_sub(4) } else { last_pc };

        self.regs[13] = self.regs[13].set_bit(31, in_delay);
        self.regs[14] = addr;

        // IRQ state might have changed.
        schedule.trigger(Event::IrqCheck);

        // Set PC to the exception handler. The exception handler address depend on BEV flag in
        // COP0 status register.
        if self.bev_in_ram() {
            0xbfc00180
        } else {
            0x80000080
        }
    }

    pub(super) fn exit_exception(&mut self, schedule: &mut Schedule) {
        let flags = self.regs[12].bit_range(0, 5);
        self.regs[12] = self.regs[12].set_bit_range(0, 3, flags >> 2);

        schedule.trigger(Event::IrqCheck);
    }
}

/// Register restart values. Just sets the register proccessor id for now.
const REGISTER_VALUES: [u32; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00000002, 0];
