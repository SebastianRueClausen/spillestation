//! Coprocessor 0/System coprocessor. Takes care of exceptions.
//! Also takes care virtual memory, but that isn't used by the playstation 1.

use crate::util::BitExtract;

#[allow(dead_code)]
pub enum Exception {
    Interrupt = 0x0,
    AddressLoadError = 0x4,
    AddressStoreError = 0x5,
    BusInstructionError = 0x6,
    BusDataError = 0x7,
    Syscall = 0x8,
    Breakpoint = 0x9,
    ReservedInstruction = 0xa,
    CopUnusable = 0xb,
    ArithmeticOverflow = 0xc,
}

pub struct Cop0 {
    /// COP0 registers - http://problemkaputt.de/psx-spx.htm#cop0exceptionhandling
    /// - 0..2 - NA.
    /// - 3 - BPC- Breakpoint on execution.
    /// - 4 - NA.
    /// - 5 - BDA - Breakpoint on data access.
    /// - 6 - JUMPDEST - Memorized jump address.
    /// - 7 - DCIC - Breakpoint control.
    /// - 8 - BadVaddr - Bad virtual address.
    /// - 9 - BDAM - Data access breakpoint mask.
    /// - 10 - NA.
    /// - 11 - BPCM- Execute breakpoint mask.
    /// - 12 - SR - Status register.
    /// - 13 - Cause - Exception type.
    /// - 14 - EPC - Return address tom trap.
    /// - 15 - PRID - Processor ID.
    registers: [u32; 16],
}

impl Cop0 {
    pub fn new() -> Self {
        Self {
            registers: REGISTER_VALUES,
        }
    }

    /// ISC - Cache isolated.
    pub fn cache_isolated(&self) -> bool {
        self.registers[12].extract_bit(16) == 1
    }

    /// BEV - Boot exception vectors in RAM/ROM.
    /// - true: KSEG1.
    /// - false: KSEG0.
    fn bev_in_ram(&self) -> bool {
        self.registers[12].extract_bit(22) == 1
    }

    /// IRQ - Interrupt enabled.
    pub fn irq_enabled(&self) -> bool {
        self.registers[12].extract_bit(0) == 1
    }

    /// Set the value of a COP0 register.
    pub fn set_reg(&mut self, reg: u32, value: u32) {
        self.registers[reg as usize] = value;
    }

    /// Read the value of a COP0 register.
    pub fn read_reg(&self, reg: u32) -> u32 {
        self.registers[reg as usize]
    }

    /// Prepares the COP0 for an exception.
    pub fn enter_exception(&mut self, last_pc: u32, in_delay: bool, ex: Exception) -> u32 {
        // Remember bits [0..5] of the status register, which keep track of interrupt and
        // kernel/user mode flags. Bits [0..1] keep track of the current flags, bits [2..3]keeps
        // track of the last flags, and bits [4..5] the ones before that.
        let flags = self.registers[12].extract_bits(0, 5);
        // When entering and exception, two 0 are appended to these bits, which disables interrupts
        // and sets the CPU to kernel mode.
        self.registers[12] &= !0x3f;
        self.registers[12] |= (flags << 2) & 0x3f;
        // Set CAUSE register to the exception type.
        self.registers[13] &= !0x7c;
        self.registers[13] |= (ex as u32) << 2;
        // If the CPU is in a branch delay slot, EPC is set to one instruction behind the last pc.
        // Bit 31 of CAUSE is also set.
        if in_delay {
            self.registers[13] |= 1 << 31;
            self.registers[14] = last_pc.wrapping_sub(4);
        } else {
            self.registers[13] &= !(1 << 31);
            self.registers[14] = last_pc;
        }
        // Set PC to the to exception handler. The exception handler address depend on BEV flag in
        // COP0 status register.
        if self.bev_in_ram() {
            0xbfc00180
        } else {
            0x80000080
        }
    }

    /// Called just before returning from an exception.
    pub fn exit_exception(&mut self) {
        let flags = self.registers[12].extract_bits(0, 5);
        self.registers[12] &= !0xf;
        self.registers[12] |= flags >> 2;
    }
}

/// Register restart values. Just sets the register proccessor id for now.
const REGISTER_VALUES: [u32; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00000002, 0];
