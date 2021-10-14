//! Coprocessor 0/System coprocessor. Takes care of exceptions.
//! Also takes care virtual memory, but that isn't used by the playstation 1.
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

// TODO: Add better support here
pub struct Cop0 {
    /// Status register.
    status: u32,
    /// Exceptions cause register.
    cause: u32,
    /// Pc where the exceptions occured.
    epc: u32,
}

impl Cop0 {
    pub fn new() -> Self {
        Self {
            status: 0,
            cause: 0,
            epc: 0,
        }
    }

    pub fn cache_is_isolated(&self) -> bool {
        self.status & 0x10000 != 0
    }

    /// Set the value of a COP0 register.
    pub fn set_reg(&mut self, reg: u32, value: u32) {
        match reg {
            3 | 5 | 6 | 7 | 9 | 11 => {
                if value != 0 {
                    panic!("Unsupported write to COP0 register {} with value {:08x}", reg, value);
                }
            }
            12 => self.status = value,
            13 => { },
            _ => panic!("Invalid COP0 register {}", reg),
        }
    }

    /// Read the value of a COP0 register.
    pub fn read_reg(&self, reg: u32) -> u32 {
        match reg {
            12 => self.status,
            13 => self.cause,
            14 => self.epc,
            _ => {
                panic!("Unsupported read from COP0 register {}", reg);
            }
        }
    }

}
