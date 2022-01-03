//! Represent the memory of the playstation 1.

pub mod bios;
pub mod dma;
pub mod ram;

mod mem_ctrl;
mod cache_ctrl;
mod ram_size;

use crate::{
    gpu::{Gpu, Vram},
    util::bits::BitExtract, cdrom::CdRom,
    cpu::IrqState,
    timer::Timers,
    spu::Spu,
};
use bios::Bios;
use dma::{BlockTransfer, Port, Direction, Dma, LinkedTransfer, Transfers};
use ram::Ram;
use mem_ctrl::MemCtrl;
use cache_ctrl::CacheCtrl;
use ram_size::RamSize;

/// The BUS of the Playstation 1. This is what (mostly) all devices are connected with, and how to
/// the CPU interacts with the rest of the machine.
pub struct Bus {
    /// The amount of CPU cycles since boot.
    pub cycle_count: u64,
    pub cache_ctrl: CacheCtrl,
    pub irq_state: IrqState,
    bios: Bios,
    ram: Ram,
    dma: Dma,
    transfers: Transfers,
    gpu: Gpu,
    cdrom: CdRom,
    timers: Timers,
    spu: Spu,
    mem_ctrl: MemCtrl,
    ram_size: RamSize,
}

impl Bus {
    pub fn new(bios: Bios) -> Self {
        Self {
            cycle_count: 0,
            bios,
            irq_state: IrqState::new(),
            ram: Ram::new(),
            dma: Dma::new(),
            transfers: Transfers::new(),
            gpu: Gpu::new(),
            cdrom: CdRom::new(),
            timers: Timers::new(),
            spu: Spu::new(),
            mem_ctrl: MemCtrl::new(),
            cache_ctrl: CacheCtrl::new(),
            ram_size: RamSize::new(),
        }
    }

    pub fn try_load<T: AddrUnit>(&mut self, address: u32) -> Option<u32> {
        debug_assert!(T::is_aligned(address));
        let address = to_region(address);
        match address {
            Ram::BUS_BEGIN..=Ram::BUS_END => {
                Some(self.ram.load::<T>(address))
            }
            Bios::BUS_BEGIN..=Bios::BUS_END => {
                Some(self.bios.load::<T>(address - Bios::BUS_BEGIN))
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                Some(self.mem_ctrl.load(address - MemCtrl::BUS_BEGIN))
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                Some(self.ram_size.load())
            }
            CacheCtrl::BUS_BEGIN..=CacheCtrl::BUS_END => {
                Some(self.cache_ctrl.load())
            }
            EXP1_BEGIN..=EXP1_END => {
                Some(0xff)
            },
            IrqState::BUS_BEGIN..=IrqState::BUS_END => {
                Some(self.irq_state.load(address - IrqState::BUS_BEGIN))
            }
            Dma::BUS_BEGIN..=Dma::BUS_END => {
                Some(self.dma.load(address - Dma::BUS_BEGIN))
            }
            CdRom::BUS_BEGIN..=CdRom::BUS_END => {
                Some(self.cdrom.load::<T>(address - CdRom::BUS_BEGIN))
            }
            Spu::BUS_BEGIN..=Spu::BUS_END => {
                Some(self.spu.load(address - Spu::BUS_BEGIN))
            }
            Timers::BUS_BEGIN..=Timers::BUS_END => {
                Some(self.timers.load(
                    &mut self.irq_state,
                    self.cycle_count,
                    address - Timers::BUS_BEGIN,
                ))
            }
            Gpu::BUS_BEGIN..=Gpu::BUS_END => {
                Some(self.gpu.load::<T>(address - Gpu::BUS_BEGIN))
            }
            _ => None,
        }
    }

    pub fn load<T: AddrUnit>(&mut self, address: u32) -> u32 {
        match self.try_load::<T>(address) {
            Some(value) => value,
            None => panic!("Trying to load invalid address to bus at {:08x}", address),
        }
    }

    pub fn store<T: AddrUnit>(&mut self, address: u32, value: u32) {
        debug_assert!(T::is_aligned(address));
        let address = to_region(address);
        match address {
            Ram::BUS_BEGIN..=Ram::BUS_END => {
                self.ram.store::<T>(address, value);
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                self.mem_ctrl.store(address - MemCtrl::BUS_BEGIN, value);
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                self.ram_size.store(value)
            }
            CacheCtrl::BUS_BEGIN..=CacheCtrl::BUS_END => {
                self.cache_ctrl.store(value)
            }
            Spu::BUS_BEGIN..=Spu::BUS_END => {
                self.spu.store(address - Spu::BUS_BEGIN, value);
            }
            EXP1_BEGIN..=EXP1_END => {
                // TODO.
            }
            EXP2_BEGIN..=EXP2_END => {
                // TODO.
            }
            IrqState::BUS_BEGIN..=IrqState::BUS_END => {
                self.irq_state.store(address - IrqState::BUS_BEGIN, value);
            }
            Timers::BUS_BEGIN..=Timers::BUS_END => {
                self.timers.store(
                    &mut self.irq_state,
                    self.cycle_count,
                    address - Timers::BUS_BEGIN,
                    value
                ); 
            }
            Dma::BUS_BEGIN..=Dma::BUS_END => {
                self.dma.store(
                    &mut self.transfers,
                    &mut self.irq_state,
                    address - Dma::BUS_BEGIN,
                    value,
                );
                self.exec_transfers();
            }
            CdRom::BUS_BEGIN..=CdRom::BUS_END => {
                self.cdrom.store::<T>(address - CdRom::BUS_BEGIN, value);
            }
            Gpu::BUS_BEGIN..=Gpu::BUS_END => {
                self.gpu.store::<T>(address - Gpu::BUS_BEGIN, value);
            }
            _ => {
                panic!("Trying to store invalid address to bus at {:08x}", address)
            }
        }
    }

    pub fn vram(&self) -> &Vram {
        self.gpu.vram()
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    pub fn timers(&self) -> &Timers {
        &self.timers
    }

    pub fn run_cdrom(&mut self) {
        self.cdrom.exec_cmd(&mut self.irq_state);
    }

    pub fn run_timers(&mut self) {
        self.timers.run(&mut self.irq_state, self.cycle_count);
    }

    /// This executes waiting DAM transfers.
    fn exec_transfers(&mut self) {
        // Execute block transfers.
        while let Some(transfer) = self.transfers.block.pop() {
            match transfer.direction {
                Direction::ToPort => self.trans_block_to_port(&transfer),
                Direction::ToRam => self.trans_block_to_ram(&transfer),
            }
            self.dma.channel_done(transfer.port, &mut self.irq_state);
        }
        // Execute linked list transfers.
        while let Some(transfer) = self.transfers.linked.pop() {
            self.trans_linked_to_port(&transfer);
            self.dma.channel_done(Port::Gpu, &mut self.irq_state);
        }
    }

    /// Execute transfers to a port.
    fn trans_block_to_port(&mut self, transfer: &BlockTransfer) {
        (0..transfer.size).fold(transfer.start, |address, _| {
            let value = self.ram.load::<Word>(address & 0x1ffffc);
            match transfer.port {
                Port::Gpu => self.gpu.dma_store(value),
                _ => todo!(),
            }
            address.wrapping_add(transfer.increment)
        });
    }
   
    /// Execute transfers to RAM from a port.
    fn trans_block_to_ram(&mut self, transfer: &BlockTransfer) {
        (0..transfer.size).rev().fold(transfer.start, |address, remain| {
            self.ram.store::<Word>(address & 0x1ffffc, match transfer.port {
                Port::Otc => match remain {
                    0 => 0xffffff,
                    _ => address.wrapping_sub(4).extract_bits(0, 21),
                }
                Port::Gpu => self.gpu.dma_load(),
                _ => todo!(),
            });
            address.wrapping_add(transfer.increment)
        });
    }

    fn trans_linked_to_port(&mut self, transfer: &LinkedTransfer) {
        let mut address = transfer.start & 0x1ffffc;
        loop {
            let header = self.ram.load::<Word>(address);
            for _ in 0..header.extract_bits(24, 31) {
                address = address.wrapping_add(4) & 0x1ffffc;
                self.gpu.gp0_store(self.ram.load::<Word>(address));
            }
            // It's done when the 23th bit is set in the header.
            if header.extract_bit(23) == 1 {
                break;
            }
            address = header & 0x1ffffc;
        }
    }
}

/// Represents the address range of a device mapped to the BUS.
pub trait BusMap {
    /// The first address in the range.
    const BUS_BEGIN: u32;
    /// The last address included in the range.
    const BUS_END: u32;
}

pub trait AddrUnit {
    const WIDTH: usize;

    fn is_aligned(address: u32) -> bool;
}

/// 8 bit.
pub struct Byte;

impl AddrUnit for Byte {
    const WIDTH: usize = 1;

    fn is_aligned(_: u32) -> bool {
        true
    }
}

/// 16 bit.
pub struct HalfWord;

impl AddrUnit for HalfWord {
    const WIDTH: usize = 2;

    fn is_aligned(address: u32) -> bool {
        (address & 0x1) == 0
    }
}

/// 32 bit.
pub struct Word;

impl AddrUnit for Word {
    const WIDTH: usize = 4;

    fn is_aligned(address: u32) -> bool {
        (address & 0x3) == 0
    }
}

pub fn to_region(address: u32) -> u32 {
    const REGION_MAP: [u32; 8] = [
        0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0x7fffffff, 0x1fffffff, 0xffffffff,
        0xffffffff,
    ];
    address & REGION_MAP[(address >> 29) as usize]
}

const EXP1_BEGIN: u32 = 0x1f000000;
const EXP1_END: u32 = EXP1_BEGIN + 512 * 1024 - 1;

const EXP2_BEGIN: u32 = 0x1f802000;
const EXP2_END: u32 = EXP2_BEGIN + 66 - 1;
