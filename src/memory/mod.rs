//! Represent the memory of the playstation 1.

pub mod bios;
pub mod dma;
pub mod ram;

use crate::{
    gpu::{Gpu, Vram},
    util::bits::BitExtract, cdrom::CdRom,
    cpu::IrqState,
    timer::Timers,
};
use bios::Bios;
use dma::{BlockTransfer, ChannelPort, Direction, Dma, LinkedTransfer, Transfers};
use ram::Ram;

pub trait AddrUnit {
    fn width() -> usize;
    fn is_aligned(address: u32) -> bool;
}

/// 8 bit.
pub struct Byte;

impl AddrUnit for Byte {
    fn width() -> usize {
        1
    }

    fn is_aligned(_: u32) -> bool {
        true
    }
}

/// 16 bit.
pub struct HalfWord;

impl AddrUnit for HalfWord {
    fn width() -> usize {
        2
    }

    fn is_aligned(address: u32) -> bool {
        (address & 0x1) == 0
    }
}

/// 32 bit.
pub struct Word;

impl AddrUnit for Word {
    fn width() -> usize {
        4
    }

    fn is_aligned(address: u32) -> bool {
        (address & 0x3) == 0
    }
}

mod map {
    const REGION_MAP: [u32; 8] = [
        0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0x7fffffff, 0x1fffffff, 0xffffffff,
        0xffffffff,
    ];

    pub fn to_region(address: u32) -> u32 {
        address & REGION_MAP[(address >> 29) as usize]
    }

    /// RAM - 2 megabytes.
    pub const RAM_START: u32 = 0x00000000;
    pub const RAM_END: u32 = RAM_START + 2 * 1024 * 1024 - 1;

    /// Memory Control - 36 bytes.
    pub const MEMCTRL_START: u32 = 0x1f801000;
    pub const MEMCTRL_END: u32 = MEMCTRL_START + 36 - 1;

    /// BIOS - 512 kilobytes.
    pub const BIOS_START: u32 = 0x1fc00000;
    pub const BIOS_END: u32 = BIOS_START + 512 * 1024 - 1;

    /// Ram Size - 4 bytes.
    pub const RAM_SIZE_START: u32 = 0x1f801060;
    pub const RAM_SIZE_END: u32 = RAM_SIZE_START + 4 - 1;

    /// Cache Control - 4 bytes.
    pub const CACHE_CONTROL_START: u32 = 0xfffe0130;
    pub const CACHE_CONTROL_END: u32 = CACHE_CONTROL_START + 4 - 1;

    /// SPU - 640 bytes.
    pub const SPU_START: u32 = 0x1f801c00;
    pub const SPU_END: u32 = SPU_START + 640 - 1;

    /// EXP1/EXPANSION REGION 1 - 8 kilobytes.
    pub const EXP1_START: u32 = 0x1f000000;
    pub const EXP1_END: u32 = EXP1_START + 512 * 1024 - 1;

    /// EXP2/EXPANSION REGION 2 - 66 bytes.
    pub const EXP2_START: u32 = 0x1f802000;
    pub const EXP2_END: u32 = EXP2_START + 66 - 1;

    /// IRQ Control - 8 bytes.
    pub const IRQ_CONTROL_START: u32 = 0x1f801070;
    pub const IRQ_CONTROL_END: u32 = IRQ_CONTROL_START + 8 - 1;

    /// Timer Control - 48 bytes.
    pub const TIMER_CONTROL_START: u32 = 0x1f801100;
    pub const TIMER_CONTROL_END: u32 = TIMER_CONTROL_START + 48 - 1;

    /// Direct Memory Access - 128 bytes.
    pub const DMA_START: u32 = 0x1f801080;
    pub const DMA_END: u32 = DMA_START + 128 - 1;

    /// CDROM - 4 bytes.
    pub const CDROM_START: u32 = 0x1f801800;
    pub const CDROM_END: u32 = CDROM_START + 4 - 1;

    /// GPU Control - 8 bytes.
    pub const GPU_START: u32 = 0x1f801810;
    pub const GPU_END: u32 = GPU_START + 8 - 1;
}

/// The BUS of the Playstation 1. This is what (mostly) all devices are connected with, and how to
/// the CPU interacts with the rest of the machine.
pub struct Bus {
    /// The amount of CPU cycles since boot.
    pub cycle_count: u64,
    bios: Bios,
    pub irq_state: IrqState,
    ram: Ram,
    dma: Dma,
    transfers: Transfers,
    gpu: Gpu,
    cdrom: CdRom,
    timers: Timers,
}

use map::*;

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
        }
    }

    pub fn try_load<T: AddrUnit>(&mut self, address: u32) -> Option<u32> {
        debug_assert!(T::is_aligned(address));
        let address = to_region(address);
        match address {
            RAM_START..=RAM_END => {
                Some(self.ram.load::<T>(address))
            }
            BIOS_START..=BIOS_END => {
                Some(self.bios.load::<T>(address - BIOS_START))
            }
            MEMCTRL_START..=MEMCTRL_END => None,
            RAM_SIZE_START..=RAM_SIZE_END => None,
            CACHE_CONTROL_START..=CACHE_CONTROL_END => None,
            EXP1_START..=EXP1_END => {
                Some(0xff)
            },
            IRQ_CONTROL_START..=IRQ_CONTROL_END => {
                Some(self.irq_state.load(address - IRQ_CONTROL_START))
            }
            DMA_START..=DMA_END => {
                Some(self.dma.load(address - DMA_START))
            }
            CDROM_START..=CDROM_END => {
                Some(self.cdrom.load(address - CDROM_START))
            }
            SPU_START..=SPU_END => {
                Some(0x0)
            }
            TIMER_CONTROL_START..=TIMER_CONTROL_END => {
                Some(self.timers.load(
                    &mut self.irq_state,
                    self.cycle_count,
                    address - TIMER_CONTROL_START,
                ))
            }
            GPU_START..=GPU_END => {
                Some(self.gpu.load(address - GPU_START))
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
            RAM_START..=RAM_END => {
                self.ram.store::<T>(address, value);
            }
            MEMCTRL_START..=MEMCTRL_END => {
                // TODO: Memory Control.
            }
            RAM_SIZE_START..=RAM_SIZE_END => {
                // TODO: Ram size.
            }
            CACHE_CONTROL_START..=CACHE_CONTROL_END => {
                // TODO: Cache Control.
            }
            SPU_START..=SPU_END => {
                // TODO: Sound.
            }
            EXP1_START..=EXP1_END => {
                // TODO.
            }
            EXP2_START..=EXP2_END => {
                // TODO.
            }
            IRQ_CONTROL_START..=IRQ_CONTROL_END => {
                self.irq_state.store(address - IRQ_CONTROL_START, value);
            }
            TIMER_CONTROL_START..=TIMER_CONTROL_END => {
                self.timers.store(
                    &mut self.irq_state,
                    self.cycle_count,
                    address - TIMER_CONTROL_START,
                    value
                ); 
            }
            DMA_START..=DMA_END => {
                self.dma.store(
                    &mut self.transfers,
                    &mut self.irq_state,
                    address - DMA_START,
                    value,
                );
                self.exec_transfers();
            }
            CDROM_START..=CDROM_END => {
                self.cdrom.store(address - CDROM_START, value);
            }
            GPU_START..=GPU_END => {
                self.gpu.store(address - GPU_START, value);
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
            self.dma.channel_done(dma::ChannelPort::Gpu, &mut self.irq_state);
        }
    }

    /// Execute transfers to a port.
    fn trans_block_to_port(&mut self, transfer: &BlockTransfer) {
        let mut address = transfer.start;
        for _ in 0..transfer.size {
            let value = self.ram.load::<Word>(address & 0x1ffffc);
            // Hopefully the compiler can optimize this to be outside the loop.
            match transfer.port {
                ChannelPort::Gpu => self.gpu.dma_store(value),
                _ => todo!(),
            }
            address = address.wrapping_add(transfer.increment);
        }
    }
   
    /// Execute transfers to RAM from a port.
    fn trans_block_to_ram(&mut self, transfer: &BlockTransfer) {
        let mut address = transfer.start;
        for remain in (0..transfer.size).rev() {
            let value = match transfer.port {
                ChannelPort::Otc => match remain {
                    0 => 0xffffff,
                    _ => address.wrapping_sub(4).extract_bits(0, 21),
                },
                ChannelPort::Gpu => self.gpu.dma_load(),
                _ => todo!(),
            };
            self.ram.store::<Word>(address & 0x1ffffc, value);
            address = address.wrapping_add(transfer.increment);
        }
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
