//! Represent the memory of the playstation 1.

pub mod bios;
pub mod ram;
pub mod dma;

use crate::util::bits::BitExtract;
use crate::gpu::Gpu;

use bios::Bios;
use dma::{
    Dma,
    Direction,
    BlockTransfer,
    LinkedTransfer,
    Transfers,
    ChannelPort,
};
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
        0xffffffff, 
        0xffffffff, 
        0xffffffff, 
        0xffffffff, 
        0x7fffffff, 
        0x1fffffff, 
        0xffffffff, 
        0xffffffff, 
    ];

    pub fn to_region(address: u32) -> u32 {
        address & REGION_MAP[(address >> 29) as usize]
    }

    /// [RAM] - 2 megabytes.
    pub const RAM_START: u32 = 0x00000000;
    pub const RAM_END: u32 = RAM_START + 2 * 1024 * 1024 - 1;

    /// [Memory Control] - 36 bytes.
    pub const MEMCTRL_START: u32 = 0x1f801000;
    pub const MEMCTRL_END: u32 = MEMCTRL_START + 36 - 1;

    /// [BIOS] - 512 kilobytes.
    pub const BIOS_START: u32 = 0x1fc00000;
    pub const BIOS_END: u32 = BIOS_START + 512 * 1024 - 1;

    /// [Ram Size] - 4 bytes.
    pub const RAM_SIZE_START: u32 = 0x1f801060;
    pub const RAM_SIZE_END: u32 = RAM_SIZE_START + 4 - 1;

    /// [Cache Control] - 4 bytes.
    pub const CACHE_CONTROL_START: u32 = 0xfffe0130;
    pub const CACHE_CONTROL_END: u32 = CACHE_CONTROL_START + 4 - 1;

    /// [SPU] - 640 bytes.
    pub const SPU_START: u32 = 0x1f801c00;
    pub const SPU_END: u32 = SPU_START + 640 - 1;

    /// [EXP1/EXPANSION REGION 1] - 8 kilobytes.
    pub const EXP1_START: u32 = 0x1f000000;
    pub const EXP1_END: u32 = EXP1_START + 512 * 1024 - 1;

    /// [EXP2/EXPANSION REGION 2] - 66 bytes.
    pub const EXP2_START: u32 = 0x1f802000;
    pub const EXP2_END: u32 = EXP2_START + 66 - 1;

    /// [IRQ Control] - 8 bytes.
    pub const IRQ_CONTROL_START: u32 = 0x1f801070;
    pub const IRQ_CONTROL_END: u32 = IRQ_CONTROL_START + 8 - 1;

    /// [Timer Control] - 48 bytes.
    pub const TIMER_CONTROL_START: u32 = 0x1f801100;
    pub const TIMER_CONTROL_END: u32 = TIMER_CONTROL_START + 48 - 1;

    /// [Direct Memory Access] - 128 bytes.
    pub const DMA_START: u32 = 0x1f801080;
    pub const DMA_END: u32 = DMA_START + 128 - 1;

    /// [GPU Control] - 8 bytes.
    pub const GPU_START: u32 = 0x1f801810;
    pub const GPU_END: u32 = GPU_START + 8 - 1;
}

pub struct Bus {
    bios: Bios,
    ram: Ram,
    dma: Dma,
    transfers: Transfers,
    gpu: Gpu,
}

use map::*;

impl Bus {
    pub fn new(bios: Bios, ram: Ram, dma: Dma, gpu: Gpu) -> Self {
        Self {
            bios,
            ram,
            dma,
            transfers: Transfers::new(),
            gpu,
        }
    }

    pub fn load<T: AddrUnit>(&self, address: u32) -> u32 {
        assert!(T::is_aligned(address));
        let address = to_region(address);
        match address {
            RAM_START..=RAM_END => {
                self.ram.load::<T>(address)
            },
            BIOS_START..=BIOS_END => {
                self.bios.load::<T>(address - BIOS_START)
            },
            // Some of these io devices might need to be read from, so we just crash to find out.
            MEMCTRL_START..=MEMCTRL_END => {
                panic!("Loading from memory control")
            }
            RAM_SIZE_START..=RAM_SIZE_END => {
                panic!("Loading from ram size io port")
            },
            CACHE_CONTROL_START..=CACHE_CONTROL_END => {
                panic!("Loading from cache control")
            },
            EXP1_START..=EXP1_END => {
                // TODO.
                0xff
            },
            IRQ_CONTROL_START..=IRQ_CONTROL_END => {
                // TODO.
                0x0
            },
            DMA_START..=DMA_END => {
                self.dma.load(address - DMA_START)   
            },
            SPU_START..=SPU_END => {
                // TODO.
                0x0
            },
            TIMER_CONTROL_START..=TIMER_CONTROL_END => {
                0x0
            },
            GPU_START..=GPU_END => {
                if address - GPU_START == 4 {
                    0x1c000000
                } else {
                    0x0
                }
            },
            _ => {
                panic!("Trying to load invalid address to bus at {:08x}", address)
            }
        }
    }

    pub fn store<T: AddrUnit>(&mut self, address: u32, value: u32) {
        assert!(T::is_aligned(address));
        let address = to_region(address);
        match address {
            RAM_START..=RAM_END => {
                self.ram.store::<T>(address, value);
            }
            // Ignore stores to memory controller and ram size controller.
            MEMCTRL_START..=MEMCTRL_END => {
                // TODO: Memory Control.
            },
            RAM_SIZE_START..=RAM_SIZE_END => {
                // TODO: Ram size.
            },
            CACHE_CONTROL_START..=CACHE_CONTROL_END => {
                // TODO: Cache Control.
            },
            SPU_START..=SPU_END => {
                // TODO: Sound.
            },
            EXP1_START..=EXP1_END => {
                // TODO.
            },
            EXP2_START..=EXP2_END => {
                // Ignore.
            },
            IRQ_CONTROL_START..=IRQ_CONTROL_END => {
                // TODO.
            }
            TIMER_CONTROL_START..=TIMER_CONTROL_END => {
                // TODO.
            },
            DMA_START..=DMA_END => {
                self.dma.store(&mut self.transfers, address - DMA_START, value);
                self.execute_transfers();
            },
            GPU_START..=GPU_END => {
                self.gpu.store(address - GPU_START, value);
            },
             _ => {
                 panic!("Trying to store invalid address to bus at {:08x}", address)
            },
        }
    }

    fn execute_transfers(&mut self) {
        while let Some(transfer) = self.transfers.block.pop() {
            match transfer.direction {
                Direction::ToPort => self.to_port_block_transfer(&transfer),
                Direction::ToRam => self.to_ram_block_transfer(&transfer),
            }
            self.dma.mark_channel_as_finished(transfer.port);
        }
        while let Some(transfer) = self.transfers.linked.pop() {
            self.to_port_linked_transfer(&transfer);
            self.dma.mark_channel_as_finished(dma::ChannelPort::Gpu);
        }
    }

    fn to_port_block_transfer(&self, transfer: &BlockTransfer) {
        let mut address = transfer.start;
        for _ in 0..transfer.size {
            let _ = self.ram.load::<Word>(address & 0x1ffffc);
            match transfer.port {
                _ => {
                    // Write to port/device.
                },
            }
            address = address.wrapping_add(transfer.increment);
        }
    }

    fn to_ram_block_transfer(&mut self, transfer: &BlockTransfer) {
        let mut address = transfer.start;
        for remain in (0..transfer.size).rev() {
            let value = match transfer.port {
                ChannelPort::Otc => match remain {
                    1 => 0xffffff,
                    _ => address.wrapping_sub(4) & 0x1fffff,
                }
                _ => {
                    0 as u32
                },
            };
            self.ram.store::<Word>(address & 0x1ffffc, value);
            address = address.wrapping_add(transfer.increment);
        }
    }

    /// Linked list DMA transfer.
    fn to_port_linked_transfer(&mut self, transfer: &LinkedTransfer) {
        let mut address = transfer.start & 0x1ffffc;
        loop {
            let header = self.ram.load::<Word>(address);
            for _ in 0..header.extract_bits(24, 31) {
                address = address.wrapping_add(4) & 0x1ffffc;
                self.gpu.gp0_store(self.ram.load::<Word>(address));
            }
            if header.extract_bit(23) == 1 {
                break;
            }
            address = header & 0x1ffffc;
        }
    }
}
