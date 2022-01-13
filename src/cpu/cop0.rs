//! # Coprocessor 0.
//!
//! Used to handle CPU exceptions in the Playstation 1. It can also handle virtual memory,
//! but that isn't used by the playstation 1.

use crate::util::{BitExtract, BitSet};

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
    /// Thrown via 'super::Gpu::op_syscall'.
    Syscall = 0x8,
    /// Thrown via 'super::Gpu::op_break'.
    Breakpoint = 0x9,
    /// Thrown when calling a illegal instruction.
    ReservedInstruction = 0xa,
    /// Thrown when calling an instruction for an unavailable coprocessor.
    CopUnusable = 0xb,
    /// Throwm by some instruction if an overload has occured.
    ArithmeticOverflow = 0xc,
}

pub(super) struct Cop0 {
    /// # COP0 registers
    ///
    /// Most aren't used by any game, so it would maybe be a better idea just to
    /// only implement the required onces.  
    ///
    /// - 0..2 - NA.
    /// - 3 - BPC - Breakpoint on execution.
    /// - 5 - BDA - Breakpoint on data access.
    /// - 6 - JUMPDEST - Memorized jump address.
    /// - 7 - DCIC - Breakpoint control.
    /// - 8 - BadVaddr - Bad virtual address.
    /// - 9 - BDAM - Data access breakpoint mask.
    /// - 11 - BPCM- Execute breakpoint mask.
    /// - 12 - SR - Status register.
    /// - 13 - Cause - Exception type.
    /// - 14 - EPC - Return address tom trap.
    /// - 15 - PRID - Processor ID.
    registers: [u32; 16],
}

impl Cop0 {
    pub(super) fn new() -> Self {
        Self {
            registers: REGISTER_VALUES,
        }
    }

    /// Describes if the scrachpad is enabled.
    pub(super) fn cache_isolated(&self) -> bool {
        self.registers[12].extract_bit(16) == 1
    }

    /// Describes if boot exception vectors is in RAM.
    fn bev_in_ram(&self) -> bool {
        self.registers[12].extract_bit(22) == 1
    }

    pub(super) fn irq_enabled(&self) -> bool {
        self.registers[12].extract_bit(0) == 1
    }

    pub(super) fn set_reg(&mut self, reg: u32, value: u32) {
        self.registers[reg as usize] = value;
    }

    pub(super) fn read_reg(&self, reg: u32) -> u32 {
        if reg == 8 {
            trace!("Bad virtual address register read");
        }
        self.registers[reg as usize]
    }

    pub(super) fn enter_exception(&mut self, last_pc: u32, in_delay: bool, ex: Exception) -> u32 {
        // Remember bits 0..5 of the status register, which keep track of interrupt and
        // kernel/user mode flags. Bits 0..1 keep track of the current flags, bits 2..3 keeps
        // track of the last flags, and bits 4..5 the ones before that.
        let flags = self.registers[12].extract_bits(0, 5);
        // When entering and exception, two 0 are appended to these bits, which disables interrupts
        // and sets the CPU to kernel mode.
        self.registers[12].set_bit_range(0, 5, flags << 2);
        // Set CAUSE register to the exception type.
        self.registers[13].set_bit_range(2, 6, ex as u32);
        // If the CPU is in a branch delay slot, EPC is set to one instruction behind the last pc.
        // Bit 31 of CAUSE is also set.
        let (bit31, addr) = if in_delay {
            (true, last_pc.wrapping_sub(4))
        } else {
            (false, last_pc)
        };
        self.registers[13].set_bit(31, bit31);
        self.registers[14] = addr;
        // Set PC to the exception handler. The exception handler address depend on BEV flag in
        // COP0 status register.
        if self.bev_in_ram() {
            0xbfc00180
        } else {
            0x80000080
        }
    }

    pub(super) fn exit_exception(&mut self) {
        let flags = self.registers[12].extract_bits(0, 5);
        self.registers[12] &= !0xf;
        self.registers[12] |= flags >> 2;
    }
}

/// Register restart values. Just sets the register proccessor id for now.
const REGISTER_VALUES: [u32; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00000002, 0];
