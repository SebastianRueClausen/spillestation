//! Represent the memory of the playstation 1.
//!
//! TODO:
//! * Add a debug peek funktion to read from devices without side effects. For now reading data
//!   through the debugger could potentially have side effects.
//!
//! * Make ['AddrUnit'] a trait implemented on u8, u16 and u32 and make devices return and take the
//!   actual primtive being loaded or stored. This also allows the compiler to infer the type.

pub mod bios;
pub mod dma;
pub mod ram;
pub mod scratchpad;
mod raw;

use splst_util::Bit;
use splst_cdimg::CdImage;
use crate::Cycle;
use crate::schedule::{Event, Schedule};
use crate::gpu::Gpu;
use crate::cdrom::CdRom;
use crate::cpu::IrqState;
use crate::timer::Timers;
use crate::spu::Spu;
use crate::io_port::IoPort;
use bios::Bios;
use dma::Dma;
use ram::Ram;
use scratchpad::ScratchPad;

pub use dma::{DmaChan, ChanStat, ChanDir};

pub struct Bus {
    pub cache_ctrl: CacheCtrl,
    pub irq_state: IrqState,
    pub schedule: Schedule,
    pub scratchpad: ScratchPad,
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
    pub fn new(bios: Bios, cd: Option<CdImage>) -> Self {
        let mut schedule = Schedule::new();

        schedule.schedule_in(5_000, Event::RunGpu);
        schedule.schedule_in(7_000, Event::RunCdRom);

        Self {
            bios,
            schedule,
            irq_state: IrqState::new(),
            scratchpad: ScratchPad::new(),
            ram: Ram::new(),
            dma: Dma::new(),
            gpu: Gpu::new(),
            cdrom: CdRom::new(cd),
            timers: Timers::new(),
            spu: Spu::new(),
            io_port: IoPort::new(),
            mem_ctrl: MemCtrl::new(),
            cache_ctrl: CacheCtrl(0),
            ram_size: RamSize(0),
        }
    }

    pub fn load<T: AddrUnit>(&mut self, addr: u32) -> Option<u32> {
        let val = match addr {
            Ram::BUS_BEGIN..=Ram::BUS_END => {
                self.schedule.tick(3);
                self.ram.load::<T>(addr)
            }
            Bios::BUS_BEGIN..=Bios::BUS_END => {
                self.schedule.tick(6 * T::WIDTH as Cycle);
                self.bios.load::<T>(addr - Bios::BUS_BEGIN)
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                self.schedule.tick(3);
                self.mem_ctrl.load(addr - MemCtrl::BUS_BEGIN)
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                self.schedule.tick(3);
                self.ram_size.0
            }
            CacheCtrl::BUS_BEGIN..=CacheCtrl::BUS_END => {
                self.schedule.tick(2);
                self.cache_ctrl.0
            }
            EXP1_BEGIN..=EXP1_END => {
                self.schedule.tick(7 * T::WIDTH as Cycle);
                0xff
            }
            EXP2_BEGIN..=EXP2_END => {
                self.schedule.tick(10 * T::WIDTH as Cycle);
                0xff
            }
            IrqState::BUS_BEGIN..=IrqState::BUS_END => {
                self.schedule.tick(3);
                self.irq_state.load(addr - IrqState::BUS_BEGIN)
            }
            Dma::BUS_BEGIN..=Dma::BUS_END => {
                self.schedule.tick(3);
                self.run_dma();
                self.dma.load(addr - Dma::BUS_BEGIN)
            }
            CdRom::BUS_BEGIN..=CdRom::BUS_END => {
                self.schedule.tick(6);
                self.cdrom.load::<T>(addr - CdRom::BUS_BEGIN)
            }
            Spu::BUS_BEGIN..=Spu::BUS_END => {
                let time = match T::WIDTH {
                    4 => 39,
                    _ => 18, 
                };
                self.schedule.tick(time);
                self.spu.load(addr - Spu::BUS_BEGIN)
            }
            Timers::BUS_BEGIN..=Timers::BUS_END => {
                self.schedule.tick(3);
                self.timers.load(
                    &mut self.schedule,
                    addr - Timers::BUS_BEGIN,
                )
            }
            Gpu::BUS_BEGIN..=Gpu::BUS_END => {
                self.schedule.tick(3);
                self.gpu.load::<T>(addr - Gpu::BUS_BEGIN)
            }
            IoPort::BUS_BEGIN..=IoPort::BUS_BEGIN => {
                self.schedule.tick(3);
                self.io_port.load(addr - IoPort::BUS_BEGIN)
            }
            _ => {
                warn!("BUS data error when loading");
                return None;
            }
        };
        Some(val)
    }

    pub fn store<T: AddrUnit>(&mut self, addr: u32, val: u32) -> Option<()> {
        match addr {
            Ram::BUS_BEGIN..=Ram::BUS_END => {
                self.ram.store::<T>(addr, val)
            }
            ScratchPad::BUS_BEGIN..=ScratchPad::BUS_END => {
                self.scratchpad.store::<T>(addr - ScratchPad::BUS_BEGIN, val)
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                self.ram_size.0 = val
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                self.mem_ctrl.store(addr - MemCtrl::BUS_BEGIN, val)
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
                self.irq_state.store(
                    &mut self.schedule,
                    addr - IrqState::BUS_BEGIN,
                    val
                )
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
                return None;
            }
        }
        Some(()) 
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
            Event::IrqTrigger(..) | Event::IrqCheck => unreachable!(),
        }
    }
}

pub trait BusMap {
    /// The first address in the range.
    const BUS_BEGIN: u32;

    /// The last address included in the range.
    const BUS_END: u32;

    /// Get the offset into the mapped range from absolute address (Which has been masked to the
    /// region). Returns 'None' if the address isn't in the mapped range.
    fn offset(addr: u32) -> Option<u32> {
        if (Self::BUS_BEGIN..=Self::BUS_END).contains(&addr) {
            Some(addr - Self::BUS_BEGIN)
        } else {
            None
        }
    }
}

struct RamSize(u32);

impl BusMap for RamSize {
    const BUS_BEGIN: u32 = 0x1f80_1060;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}

#[derive(Clone, Copy)]
pub struct CacheCtrl(u32);

impl CacheCtrl {
    pub fn icache_enabled(self) -> bool {
        self.0.bit(11)
    }

    pub fn tag_test_enabled(self) -> bool {
        self.0.bit(2)
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

const EXP1_BEGIN: u32 = 0x1f00_0000;
const EXP1_END: u32 = EXP1_BEGIN + 512 * 1024 - 1;

const EXP2_BEGIN: u32 = 0x1f80_2000;
const EXP2_END: u32 = EXP2_BEGIN + 66 - 1;
