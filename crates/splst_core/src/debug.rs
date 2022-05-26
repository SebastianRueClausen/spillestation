use crate::cpu::{Irq, Cpu, Opcode};
use crate::bus::AddrUnit;

pub trait Debugger {
    /// Called before executing each instruction.
    fn instruction(&mut self, _cpu: &Cpu, _addr: u32, _op: Opcode) {}

    /// Called during each load instruction, but only if the load succeedes.
    fn load<T: AddrUnit>(&mut self, _cpu: &Cpu, _addr: u32, _val: T) {}

    /// Called during each store instruction, before the store.
    fn store<T: AddrUnit>(&mut self, _cpu: &Cpu, _addr: u32, _val: T) {}

    /// Called just before interrupt of type `irq` is handeled.
    fn irq(&mut self, _cpu: &Cpu, _irq: Irq) {}

    /// Called after every instruction, if it returns `true`, the system will stop further
    /// execution.
    fn should_break(&mut self) -> bool;
}

impl Debugger for () {
    fn should_break(&mut self) -> bool {
        false
    }
}
