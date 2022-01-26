//! Represent the memory of the playstation 1.

pub mod bios;
pub mod dma;
pub mod ram;

use splst_util::Bit;
use crate::gpu::Gpu;
use crate::cdrom::{CdRom, CdRomCmd};
use crate::cpu::{Irq, IrqState, cop0::Exception};
use crate::timer::{Timers, TimerId};
use crate::spu::Spu;
use crate::io_port::IoPort;
use crate::Cycle;

use std::collections::BinaryHeap;
use std::collections::binary_heap::Iter as BinaryHeapIter;
use std::cmp::Ordering;
use std::fmt;

use bios::Bios;
use dma::{Dma, Port};
use ram::Ram;

pub use dma::{DmaChan, ChanStat, ChanDir};

pub struct Bus {
    pub cache_ctrl: CacheCtrl,
    pub irq_state: IrqState,
    pub schedule: Schedule,
    bios: Bios,
    ram: Ram,
    dma: Dma,
    gpu: Gpu,
    cdrom: CdRom,
    timers: Timers,
    spu: Spu,
    mem_ctrl: MemCtrl,
    ram_size: RamSize,
    io_port: IoPort,
}

impl Bus {
    pub fn new(bios: Bios) -> Self {
        let mut schedule = Schedule::new();
        schedule.schedule_in(5_000, Event::RunGpu);
        schedule.schedule_in(7_000, Event::RunCdRom);
        Self {
            bios,
            schedule,
            irq_state: IrqState::new(),
            ram: Ram::new(),
            dma: Dma::new(),
            gpu: Gpu::new(),
            cdrom: CdRom::new(),
            timers: Timers::new(),
            spu: Spu::new(),
            io_port: IoPort::new(),
            mem_ctrl: MemCtrl::new(),
            cache_ctrl: CacheCtrl(0),
            ram_size: RamSize(0),
        }
    }

    pub fn load<T: AddrUnit>(&mut self, addr: u32) -> Result<u32, Exception> {
        if !T::is_aligned(addr) {
            return Err(Exception::AddressLoadError);
        }
        let addr = to_region(addr);
        match addr {
            Ram::BUS_BEGIN..=Ram::BUS_END => {
                Ok(self.ram.load::<T>(addr))
            }
            Bios::BUS_BEGIN..=Bios::BUS_END => {
                Ok(self.bios.load::<T>(addr - Bios::BUS_BEGIN))
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                Ok(self.mem_ctrl.load(addr - MemCtrl::BUS_BEGIN))
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                Ok(self.ram_size.0)
            }
            CacheCtrl::BUS_BEGIN..=CacheCtrl::BUS_END => {
                Ok(self.cache_ctrl.0)
            }
            EXP1_BEGIN..=EXP1_END => {
                Ok(0xff)
            },
            EXP2_BEGIN..=EXP2_END => {
                Ok(0xff)
            }
            IrqState::BUS_BEGIN..=IrqState::BUS_END => {
                Ok(self.irq_state.load(addr - IrqState::BUS_BEGIN))
            }
            Dma::BUS_BEGIN..=Dma::BUS_END => {
                self.run_dma();
                Ok(self.dma.load(addr - Dma::BUS_BEGIN))
            }
            CdRom::BUS_BEGIN..=CdRom::BUS_END => {
                Ok(self.cdrom.load::<T>(addr - CdRom::BUS_BEGIN))
            }
            Spu::BUS_BEGIN..=Spu::BUS_END => {
                Ok(self.spu.load(addr - Spu::BUS_BEGIN))
            }
            Timers::BUS_BEGIN..=Timers::BUS_END => {
                Ok(self.timers.load(
                    &mut self.schedule,
                    addr - Timers::BUS_BEGIN,
                ))
            }
            Gpu::BUS_BEGIN..=Gpu::BUS_END => {
                Ok(self.gpu.load::<T>(addr - Gpu::BUS_BEGIN))
            }
            IoPort::BUS_BEGIN..=IoPort::BUS_BEGIN => {
                Ok(self.io_port.load(addr - IoPort::BUS_BEGIN))
            }
            _ => {
                warn!("BUS data error when loading");
                Err(Exception::BusDataError)
            }
        }
    }

    pub fn store<T: AddrUnit>(&mut self, addr: u32, val: u32) -> Result<(), Exception>{
        if !T::is_aligned(addr) {
            return Err(Exception::AddressStoreError);
        }
        let addr = to_region(addr);
        match addr {
            Ram::BUS_BEGIN..=Ram::BUS_END => {
                self.ram.store::<T>(addr, val)
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                self.mem_ctrl.store(addr - MemCtrl::BUS_BEGIN, val)
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                self.ram_size.0 = val
            }
            CacheCtrl::BUS_BEGIN..=CacheCtrl::BUS_END => {
                self.cache_ctrl.0 = val
            }
            Spu::BUS_BEGIN..=Spu::BUS_END => {
                self.spu.store(addr - Spu::BUS_BEGIN, val)
            }
            EXP1_BEGIN..=EXP1_END => {}
            EXP2_BEGIN..=EXP2_END => {}
            IrqState::BUS_BEGIN..=IrqState::BUS_END => {
                self.irq_state.store(addr - IrqState::BUS_BEGIN, val)
            }
            Timers::BUS_BEGIN..=Timers::BUS_END => {
                self.timers.store(
                    &mut self.schedule,
                    addr - Timers::BUS_BEGIN,
                    val
                );
            }
            Dma::BUS_BEGIN..=Dma::BUS_END => {
                self.dma.store(
                    &mut self.schedule,
                    addr - Dma::BUS_BEGIN,
                    val,
                );
                self.run_dma();
            }
            CdRom::BUS_BEGIN..=CdRom::BUS_END => {
                self.cdrom.store::<T>(
                    &mut self.schedule,
                    addr - CdRom::BUS_BEGIN,
                    val,
                );
            }
            Gpu::BUS_BEGIN..=Gpu::BUS_END => {
                self.gpu.store::<T>(&mut self.schedule, addr - Gpu::BUS_BEGIN, val);
            }
            IoPort::BUS_BEGIN..=IoPort::BUS_END => {
                self.io_port.store(&mut self.irq_state, addr - IoPort::BUS_BEGIN, val);
            }
            _ => {
                warn!("BUS data error when storing at address {:?}", addr);
                return Err(Exception::BusDataError);
            }
        }
        Ok(()) 
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    pub fn timers(&self) -> &Timers {
        &self.timers
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::RunCdRom => self.cdrom.run(&mut self.schedule),
            Event::CdRomResponse(cmd) => self.cdrom.reponse(cmd),
            Event::RunGpu => self.gpu.run(&mut self.schedule, &mut self.timers),
            Event::GpuCmdDone => self.gpu.cmd_done(),
            Event::RunDmaChan(port) => self.run_dma_chan(port),
            Event::TimerIrqEnable(id) => self.timers.enable_irq_master_flag(id),
            Event::RunTimer(id) => self.timers.run_timer(&mut self.schedule, id),
            Event::IrqTrigger(..) => unreachable!(),
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum Event {
    RunCdRom,
    RunGpu,
    GpuCmdDone,
    RunDmaChan(Port),
    CdRomResponse(CdRomCmd),
    TimerIrqEnable(TimerId),
    RunTimer(TimerId),
    IrqTrigger(Irq),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Event::RunCdRom => write!(f, "Run CDROM"),
            Event::RunGpu => write!(f, "Run GPU"),
            Event::GpuCmdDone => write!(f, "GPU command done"),
            Event::RunDmaChan(port) => write!(f, "Run DMA Channel: {:?}", port),
            Event::CdRomResponse(cmd) => write!(f, "CDROM reponse for command: {}", cmd),
            Event::TimerIrqEnable(id) => write!(f, "Enable IRQ for timer: {}", id),
            Event::RunTimer(id) => write!(f, "Run timer: {}", id),
            Event::IrqTrigger(irq) => write!(f, "Trigger IRQ of type: {}", irq),
        }
    }
}

pub struct EventEntry(pub Cycle, pub Event);

impl PartialEq for EventEntry {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for EventEntry {}

impl PartialOrd for EventEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))  
    }
}

impl Ord for EventEntry {
    /// Sort smallest to largest cycle.
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.cmp(&self.0)
    }
}

/// This is reponsible to handling events and timing of the system in general.
pub struct Schedule {
    /// The absolute cycle number, which is the amount of cycles the system has run since startup.
    /// It's used for timing event and allow the devices on ['Bus'] to pick an absolute
    /// cycle to run an event.
    cycle: Cycle,
    /// Event queue. This allows for a fast way to check if any events should run at any given cycle.
    /// Events are sorted in the binary queue such that the next event to run is the root item.
    events: BinaryHeap<EventEntry>,
}

impl Schedule {
    fn new() -> Self {
        Self {
            cycle: 0,
            events: BinaryHeap::with_capacity(16),
        }
    }

    /// Returns iter of all event entries in the event heap in arbitary order.
    pub fn iter<'a>(&'a self) -> BinaryHeapIter<'a, EventEntry> {
        self.events.iter()
    }

    /// Schedule an ['Event'] at a given absolute cycle.
    pub fn schedule_at(&mut self, cycle: Cycle, event: Event) {
        self.events.push(EventEntry(cycle, event));
    }

    /// Schedule an ['Event'] in a given number of cycles.
    pub fn schedule_in(&mut self, cycles: Cycle, event: Event) {
        self.schedule_at(self.cycle + cycles, event);
    }

    /// Schedule an ['Event'] to be executed as soon as possible.
    pub fn schedule_now(&mut self, event: Event) {
        self.schedule_at(0, event);
    }

    /// Returns an event if any is ready.
    pub fn pop_event(&mut self) -> Option<Event> {
        match self.events.peek() {
            Some(entry) if entry.0 <= self.cycle => {
                Some(self.events.pop().unwrap().1)
            }
            _ => None,
        }
    }

    pub fn unschedule(&mut self, event: Event) {
        self.events.retain(|entry| {
            entry.1 != event 
        });
    }

    pub fn cycle(&self) -> Cycle {
        self.cycle
    }

    /// Move a given amount of cycles forward.
    pub fn tick(&mut self, cycles: Cycle) {
        self.cycle += cycles;
    }

    /// Skip to a cycle. It can only skip forward, so if a cycle given is less than the current cycle,
    /// nothing happens.
    pub fn skip_to(&mut self, cycle: Cycle) {
        self.cycle = self.cycle.max(cycle);
    }
}

pub trait BusMap {
    /// The first address in the range.
    const BUS_BEGIN: u32;
    /// The last address included in the range.
    const BUS_END: u32;
}

struct RamSize(u32);

impl BusMap for RamSize {
    const BUS_BEGIN: u32 = 0x1f80_1060;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}

pub struct CacheCtrl(u32);

impl CacheCtrl {
    pub fn icache_enabled(&self) -> bool {
        self.0.bit(11)
    }
}

impl BusMap for CacheCtrl {
    const BUS_BEGIN: u32 = 0xfffe_0130;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}

pub struct MemCtrl {
    regs: [u32; 9], 
}

impl MemCtrl {
    pub fn new() -> Self {
        Self { regs: [0x0; 9] }
    }

    pub fn store(&mut self, addr: u32, val: u32) {
        match addr {
            0 if val != 0x1f00_0000 => {
                todo!("Expansion 1 base address"); 
            }
            4 if val != 0x1f80_2000 => {
                todo!("Expansion 2 base address"); 
            }
            _ => {},
        }
        self.regs[(addr >> 2) as usize] = val;
    }

    pub fn load(&self, addr: u32) -> u32 {
        self.regs[(addr >> 2) as usize]
    }
}

impl BusMap for MemCtrl {
    const BUS_BEGIN: u32 = 0x1f80_1000;
    const BUS_END: u32 = Self::BUS_BEGIN + 36 - 1;
}

pub trait AddrUnit {
    const WIDTH: usize;

    fn is_aligned(address: u32) -> bool;
}

pub struct Byte;

impl AddrUnit for Byte {
    const WIDTH: usize = 1;

    fn is_aligned(_: u32) -> bool {
        true
    }
}

pub struct HalfWord;

impl AddrUnit for HalfWord {
    const WIDTH: usize = 2;

    fn is_aligned(address: u32) -> bool {
        (address & 0x1) == 0
    }
}

pub struct Word;

impl AddrUnit for Word {
    const WIDTH: usize = 4;

    fn is_aligned(address: u32) -> bool {
        (address & 0x3) == 0
    }
}

pub fn to_region(address: u32) -> u32 {
    const REGION_MAP: [u32; 8] = [
        0xffff_ffff, 0xffff_ffff, 0xffff_ffff, 0xffff_ffff, 0x7fff_ffff, 0x1fff_ffff, 0xffff_ffff,
        0xffff_ffff,
    ];
    address & REGION_MAP[(address >> 29) as usize]
}

const EXP1_BEGIN: u32 = 0x1f00_0000;
const EXP1_END: u32 = EXP1_BEGIN + 512 * 1024 - 1;

const EXP2_BEGIN: u32 = 0x1f80_2000;
const EXP2_END: u32 = EXP2_BEGIN + 66 - 1;
