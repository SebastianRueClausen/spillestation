//! Represent the memory BUS of the playstation 1.
//!
//! # TODO
//!
//! - Add a debug peek funktion to read from devices without side effects. For now reading data
//!   through the debugger could potentially have side effects.
//!
//! - Make ['MemUnit'] a trait implemented on u8, u16 and u32 and make devices return and take the
//!   actual primtive being loaded or stored. This also allows the compiler to infer the type.

pub mod bios;
pub mod dma;
pub mod ram;
pub mod scratchpad;
mod raw;

use splst_util::Bit;
use crate::{VideoOutput, AudioOutput, SysTime};
use crate::schedule::{Event, Schedule};
use crate::gpu::Gpu;
use crate::cdrom::{CdRom, Disc};
use crate::cpu::IrqState;
use crate::timer::Timers;
use crate::spu::Spu;
use crate::io_port::{IoPort, pad};
use bios::Bios;
use dma::Dma;
use ram::Ram;
use scratchpad::ScratchPad;

use std::cell::RefCell;
use std::rc::Rc;
use std::fmt;

pub struct Bus {
    pub cache_ctrl: CacheCtrl,
    pub scratchpad: ScratchPad,
    pub(super) irq_state: IrqState,
    pub(super) bios: Bios,
    pub(super) schedule: Schedule,
    ram: Ram,
    dma: Dma,
    pub(super) gpu: Gpu,
    pub(super) cdrom: CdRom,
    pub(super) timers: Timers,
    pub(super) spu: Spu,
    mem_ctrl: MemCtrl,
    ram_size: RamSize,
    pub(super) io_port: IoPort,
}

impl Bus {
    pub fn new(
        bios: Bios,
        video_output: Rc<RefCell<dyn VideoOutput>>,
        audio_output: Rc<RefCell<dyn AudioOutput>>,
        disc: Rc<RefCell<Disc>>,
        controllers: Rc<RefCell<pad::Controllers>>,
    ) -> Self {
        let mut schedule = Schedule::new();

        let gpu = Gpu::new(&mut schedule, video_output);
        let cdrom = CdRom::new(&mut schedule, disc);
        let spu = Spu::new(&mut schedule, audio_output);

        Self {
            bios,
            schedule,
            gpu,
            cdrom,
            irq_state: IrqState::new(),
            scratchpad: ScratchPad::new(),
            ram: Ram::new(),
            dma: Dma::new(),
            timers: Timers::new(),
            spu,
            io_port: IoPort::new(controllers),
            mem_ctrl: MemCtrl::new(),
            cache_ctrl: CacheCtrl(0),
            ram_size: RamSize(0),
        }
    }

    /// Read from memory address on the bus without side effects.
    pub fn peek<T: AddrUnit>(&self, addr: u32) -> Option<T> {
        let addr = regioned_addr(addr);
        let val: T = match addr {
            Ram::BUS_BEGIN..=Ram::BUS_END => {
                self.ram.load(addr)
            }
            Bios::BUS_BEGIN..=Bios::BUS_END => {
                self.bios.load(addr - Bios::BUS_BEGIN)
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                let val = self.mem_ctrl.load(addr - MemCtrl::BUS_BEGIN);
                T::from_u32_aligned(val, addr)
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                T::from_u32_aligned(self.ram_size.0, addr)
            }
            CacheCtrl::BUS_BEGIN..=CacheCtrl::BUS_END => {
                T::from_u32_aligned(self.cache_ctrl.0, addr)
            }
            EXP1_BEGIN..=EXP1_END => T::from_u32(0xff),
            EXP2_BEGIN..=EXP2_END => T::from_u32(0xff),
            Gpu::BUS_BEGIN..=Gpu::BUS_END => {
                self.gpu.peek(addr - Gpu::BUS_BEGIN)
            }
            IrqState::BUS_BEGIN..=IrqState::BUS_END => {
                self.irq_state.load(addr - IrqState::BUS_BEGIN)
            }
            Dma::BUS_BEGIN..=Dma::BUS_END => {
                self.dma.load(addr - Dma::BUS_BEGIN)
            }
            CdRom::BUS_BEGIN..=CdRom::BUS_END => {
                self.cdrom.peek(addr - CdRom::BUS_BEGIN)
            }
            Spu::BUS_BEGIN..=Spu::BUS_END => {
                self.spu.load(addr - Spu::BUS_BEGIN)
            }
            Timers::BUS_BEGIN..=Timers::BUS_END => {
                self.timers.peek(addr - Timers::BUS_BEGIN)
            }
            IoPort::BUS_BEGIN..=IoPort::BUS_END => {
                self.io_port.peek(addr - IoPort::BUS_BEGIN)
            }
            _ => return None,
        };
        Some(val)
    }

    pub fn load<T: AddrUnit>(&mut self, addr: u32) -> Option<(T, SysTime)> {
        let (val, time) = match addr {
            Ram::BUS_BEGIN..=Ram::BUS_END => {
                (self.ram.load(addr), SysTime::new(3))
            }
            Bios::BUS_BEGIN..=Bios::BUS_END => {
                let time = SysTime::new(6 * T::WIDTH as u64);
                (self.bios.load(addr - Bios::BUS_BEGIN), time)
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                let val = self.mem_ctrl.load(addr - MemCtrl::BUS_BEGIN);
                (T::from_u32_aligned(val, addr), SysTime::new(3))
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                (T::from_u32(self.ram_size.0), SysTime::new(3))
            }
            CacheCtrl::BUS_BEGIN..=CacheCtrl::BUS_END => {
                (T::from_u32(self.cache_ctrl.0), SysTime::new(2))
            }
            EXP1_BEGIN..=EXP1_END => {
                let time = SysTime::new(7 * T::WIDTH as u64);
                (T::from_u32(0xff), time)
            }
            EXP2_BEGIN..=EXP2_END => {
                let time = SysTime::new(10 * T::WIDTH as u64);
                (T::from_u32(0xff), time)
            }
            IrqState::BUS_BEGIN..=IrqState::BUS_END => {
                (self.irq_state.load(addr - IrqState::BUS_BEGIN), SysTime::new(3))
            }
            Dma::BUS_BEGIN..=Dma::BUS_END => {
                self.run_dma();
                (self.dma.load(addr - Dma::BUS_BEGIN), SysTime::new(3))
            }
            CdRom::BUS_BEGIN..=CdRom::BUS_END => {
                (self.cdrom.load(addr - CdRom::BUS_BEGIN), SysTime::new(6))
            }
            Spu::BUS_BEGIN..=Spu::BUS_END => {
                let time = match T::WIDTH {
                    AddrUnitWidth::Word => SysTime::new(39),
                    _ => SysTime::new(18), 
                };
                (self.spu.load(addr - Spu::BUS_BEGIN), time)
            }
            Timers::BUS_BEGIN..=Timers::BUS_END => {
                let val: T = self.timers.load(
                    &mut self.schedule,
                    addr - Timers::BUS_BEGIN,
                );
                (val, SysTime::new(3))
            }
            Gpu::BUS_BEGIN..=Gpu::BUS_END => {
                let val: T = self.gpu.load::<T>(addr - Gpu::BUS_BEGIN);
                (val, SysTime::new(3))
            }
            IoPort::BUS_BEGIN..=IoPort::BUS_END => {
                let val: T = self.io_port.load(
                    &mut self.schedule,
                    addr - IoPort::BUS_BEGIN,
                );
                (val, SysTime::new(3))
            }
            _ => {
                warn!("BUS data error when loading at address {addr:08x}");
                return None;
            }
        };
        Some((val, time))
    }

    pub fn store<T: AddrUnit>(&mut self, addr: u32, val: T) -> Option<()> {
        match addr {
            Ram::BUS_BEGIN..=Ram::BUS_END => {
                self.ram.store(addr, val)
            }
            ScratchPad::BUS_BEGIN..=ScratchPad::BUS_END => {
                self.scratchpad.store(addr - ScratchPad::BUS_BEGIN, val)
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                self.ram_size.0 = val.as_u32()
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                self.mem_ctrl.store(addr - MemCtrl::BUS_BEGIN, val.as_u32())
            }
            CacheCtrl::BUS_BEGIN..=CacheCtrl::BUS_END => {
                self.cache_ctrl.0 = val.as_u32()
            }
            Spu::BUS_BEGIN..=Spu::BUS_END => {
                self.spu.store(&mut self.schedule, addr - Spu::BUS_BEGIN, val)
            }
            EXP1_BEGIN..=EXP1_END => {}
            EXP2_BEGIN..=EXP2_END => {}
            IrqState::BUS_BEGIN..=IrqState::BUS_END => {
                self.irq_state.store(
                    &mut self.schedule,
                    addr - IrqState::BUS_BEGIN,
                    val,
                )
            }
            Timers::BUS_BEGIN..=Timers::BUS_END => {
                self.timers.store(
                    &mut self.schedule,
                    addr - Timers::BUS_BEGIN,
                    val,
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
                self.cdrom.store(
                    &mut self.schedule,
                    addr - CdRom::BUS_BEGIN,
                    val,
                );
            }
            Gpu::BUS_BEGIN..=Gpu::BUS_END => {
                self.gpu.store(&mut self.schedule, addr - Gpu::BUS_BEGIN, val);
            }
            IoPort::BUS_BEGIN..=IoPort::BUS_END => {
                self.io_port.store(&mut self.schedule, addr - IoPort::BUS_BEGIN, val);
            }
            _ => {
                warn!("BUS data error when storing at address {:?}", addr);
                return None;
            }
        }
        Some(()) 
    }
}

/// Instructions in KUSEG and KUSEG0 are cached in the instruction cache.
pub fn addr_cached(addr: u32) -> bool {
    (addr >> 29) <= 4
}

#[inline]
pub fn regioned_addr(addr: u32) -> u32 {
    const REGION_MAP: [u32; 8] = [
        0xffff_ffff,
        0xffff_ffff,
        0xffff_ffff,
        0xffff_ffff,
        0x7fff_ffff,
        0x1fff_ffff,
        0xffff_ffff,
        0xffff_ffff,
    ];
    addr & REGION_MAP[(addr >> 29) as usize]
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
            _ => (),
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

/// The width of an addressable unit. The value represents the amount of bytes in the unit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AddrUnitWidth {
   Byte = 1,
   HalfWord = 2,
   Word = 4,
}

impl AddrUnitWidth {
    pub fn is_byte(self) -> bool {
        matches!(self, AddrUnitWidth::Byte)
    }

    pub fn is_half_word(self) -> bool {
        matches!(self, AddrUnitWidth::HalfWord)
    }

    pub fn is_word(self) -> bool {
        matches!(self, AddrUnitWidth::Word)
    }
}

impl fmt::Display for AddrUnitWidth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            AddrUnitWidth::Byte => "byte",
            AddrUnitWidth::HalfWord => "half word",
            AddrUnitWidth::Word => "word",
        })
    }
}

/// Addressable unit.
pub trait AddrUnit: Into<u32> + From<u8> {
    /// The width of the addressable unit.
    const WIDTH: AddrUnitWidth;

    /// Create from 'u32' where it gets the value from the bytes of 'val', depending on the
    /// value of 'addr'. 'addr' should be aligned to get a correct value.
    ///
    /// # Example
    ///
    /// '''
    /// let val: u8 = u8::from_u32(0xff00, 1);
    /// assert_eq!(val, 0xff);
    /// '''
    fn from_u32_aligned(val: u32, addr: u32) -> Self;

    /// Get from 'u32' which may be lossy.
    fn from_u32(val: u32) -> Self;

    /// Get from 'u8'.
    fn from_u8(val: u8) -> Self {
        Self::from(val)
    }

    /// Get as 'u32' where the value depend on the alignment of 'addr'. 'addr' should be correctly
    /// aligned.
    ///
    /// # Example
    ///
    /// '''
    /// let val: u16 = 0xff00;
    /// assert_eq!(val.as_u32(2), 0xff00_0000);
    /// '''
    fn as_u32_aligned(self, addr: u32) -> u32 {
        let val: u32 = self.into();
        let align = addr & 3;
        val << (8 * align)
    }

    /// Cast to u32 like 'self as u32'.
    fn as_u32(self) -> u32 {
        self.into()
    }

    /// Cast to u16 like 'self as u16'.
    fn as_u16(self) -> u16 {
        self.as_u32() as u16
    }

    /// Cast to u8 like 'self as u8'.
    fn as_u8(self) -> u8 {
        self.as_u32() as u8
    }
}

/// Align 'addr' to the an address with alignment of ['AddrUnit'] 'T'. It will always round down,
///
/// # Example
///
/// '''
/// assert_eq!(align_as::<u16>(3), 2);
/// assert_eq!(align_as::<u32>(3), 0);
/// '''
pub fn align_as<T: AddrUnit>(addr: u32) -> u32 {
    addr & !(T::WIDTH as u32 - 1)
}

/// Check if an address is aligned to the width of an ['AddrUnit'].
///
/// # Example
///
/// '''
/// assert_eq!(is_aligned_as::<u16>(3), false);
/// assert_eq!(is_aligned_as::<u32>(4), true);
/// '''
pub fn is_aligned_to<T: AddrUnit>(addr: u32) -> bool {
    (addr % T::WIDTH as u32) == 0
}

impl AddrUnit for u32 {
    const WIDTH: AddrUnitWidth = AddrUnitWidth::Word;

    fn from_u32(val: u32) -> Self {
        val
    }

    fn from_u32_aligned(val: u32, _: u32) -> Self {
        val
    }
}

impl AddrUnit for u16 {
    const WIDTH: AddrUnitWidth = AddrUnitWidth::HalfWord;

    fn from_u32(val: u32) -> Self {
        val as u16
    }

    fn from_u32_aligned(val: u32, addr: u32) -> Self {
        (val >> (8 * (addr % 4))) as u16
    }
}

impl AddrUnit for u8 {
    const WIDTH: AddrUnitWidth = AddrUnitWidth::Byte;

    fn from_u32(val: u32) -> Self {
        val as u8
    }

    fn from_u32_aligned(val: u32, addr: u32) -> Self {
        (val >> (8 * (addr % 4))) as u8
    }
}

const EXP1_BEGIN: u32 = 0x1f00_0000;
const EXP1_END: u32 = EXP1_BEGIN + 512 * 1024 - 1;

const EXP2_BEGIN: u32 = 0x1f80_2000;
const EXP2_END: u32 = EXP2_BEGIN + 66 - 1;
