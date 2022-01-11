//! Represent the memory of the playstation 1.

pub mod bios;
pub mod dma;
pub mod ram;

use crate::gpu::{Gpu, Vram};
use crate::util::BitExtract;
use crate::cdrom::CdRom;
use crate::cpu::{IrqState, cop0::Exception};
use crate::timer::Timers;
use crate::spu::Spu;
use crate::io_port::IoPort;

use bios::Bios;
use dma::{BlockTransfer, Port, Direction, Dma, LinkedTransfer, Transfers};
use ram::Ram;

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
    io_port: IoPort,
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
                    &mut self.irq_state,
                    self.cycle_count,
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
                Ok(self.ram.store::<T>(addr, val))
            }
            MemCtrl::BUS_BEGIN..=MemCtrl::BUS_END => {
                Ok(self.mem_ctrl.store(addr - MemCtrl::BUS_BEGIN, val))
            }
            RamSize::BUS_BEGIN..=RamSize::BUS_END => {
                Ok(self.ram_size.0 = val)
            }
            CacheCtrl::BUS_BEGIN..=CacheCtrl::BUS_END => {
                Ok(self.cache_ctrl.0 = val)
            }
            Spu::BUS_BEGIN..=Spu::BUS_END => {
                Ok(self.spu.store(addr - Spu::BUS_BEGIN, val))
            }
            EXP1_BEGIN..=EXP1_END => {
                Ok(())
            }
            EXP2_BEGIN..=EXP2_END => {
                Ok(())
            }
            IrqState::BUS_BEGIN..=IrqState::BUS_END => {
                Ok(self.irq_state.store(addr - IrqState::BUS_BEGIN, val))
            }
            Timers::BUS_BEGIN..=Timers::BUS_END => {
                Ok(self.timers.store(
                    &mut self.irq_state,
                    self.cycle_count,
                    addr - Timers::BUS_BEGIN,
                    val
                ))
            }
            Dma::BUS_BEGIN..=Dma::BUS_END => {
                self.dma.store(
                    &mut self.transfers,
                    &mut self.irq_state,
                    addr - Dma::BUS_BEGIN,
                    val,
                );
                Ok(self.exec_transfers())
            }
            CdRom::BUS_BEGIN..=CdRom::BUS_END => {
                Ok(self.cdrom.store::<T>(
                    &mut self.irq_state,
                    addr - CdRom::BUS_BEGIN,
                    val,
                ))
            }
            Gpu::BUS_BEGIN..=Gpu::BUS_END => {
                Ok(self.gpu.store::<T>(addr - Gpu::BUS_BEGIN, val))
            }
            IoPort::BUS_BEGIN..=IoPort::BUS_END => {
                Ok(self.io_port.store(addr - IoPort::BUS_BEGIN, val))
            }
            _ => {
                warn!("BUS data error when storing");
                Err(Exception::BusDataError)
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

    pub fn run_gpu(&mut self) {
        self.gpu.run(&mut self.irq_state, &mut self.timers, self.cycle_count);
    }

    /// This executes waiting DAM transfers.
    fn exec_transfers(&mut self) {
        // Execute block transfers.
        while let Some(transfer) = self.transfers.block.pop() {
            trace!("DMA block transfer: {:?}", transfer);
            match transfer.direction {
                Direction::ToPort => {
                    self.trans_block_to_port(&transfer);
                }
                Direction::ToRam => {
                    self.trans_block_to_ram(&transfer);
                }
            }
            self.dma.channel_done(transfer.port, &mut self.irq_state);
        }
        // Execute linked list transfers.
        while let Some(transfer) = self.transfers.linked.pop() {
            trace!("DMA linked transfer: {:?}", transfer);
            self.trans_linked_to_port(&transfer);
            self.dma.channel_done(Port::Gpu, &mut self.irq_state);
        }
    }

    /// Execute transfers to a port.
    fn trans_block_to_port(&mut self, transfer: &BlockTransfer) {
        (0..transfer.size).fold(transfer.start, |address, _| {
            let value = self.ram.load::<Word>(address & 0x1f_fffc);
            match transfer.port {
                Port::Gpu => self.gpu.dma_store(value),
                _ => todo!(),
            }
            address.wrapping_add(transfer.increment)
        });
    }
   
    /// Execute transfers to RAM from a port.
    fn trans_block_to_ram(&mut self, transfer: &BlockTransfer) {
        (0..transfer.size).rev().fold(transfer.start, |addr, remain| {
            self.ram.store::<Word>(addr & 0x1_ffffc, match transfer.port {
                Port::Otc => match remain {
                    0 => 0xff_ffff,
                    _ => addr.wrapping_sub(4).extract_bits(0, 21),
                }
                Port::Gpu => self.gpu.dma_load(),
                _ => todo!(),
            });
            addr.wrapping_add(transfer.increment)
        });
    }

    fn trans_linked_to_port(&mut self, transfer: &LinkedTransfer) {
        let mut addr = transfer.start;
        loop {
            let header = self.ram.load::<Word>(addr & 0x1f_fffc);
            // Bit 24..31 in the header represents the size of the node, which get's transfered to
            // the port.
            for _ in 0..header.extract_bits(24, 31) {
                addr = addr.wrapping_add(4).extract_bits(0, 23);
                self.gpu.dma_store(self.ram.load::<Word>(addr));
            }
            addr = header.extract_bits(0, 23);
            if addr == 0xff_ffff {
                break;
            }
        }
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
    const BUS_BEGIN: u32 = 0x1f801060;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}

pub struct CacheCtrl(u32);

impl CacheCtrl {
    pub fn icache_enabled(&self) -> bool {
        self.0.extract_bit(11) == 1
    }
}

impl BusMap for CacheCtrl {
    const BUS_BEGIN: u32 = 0xfffe0130;
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
            0 if val != 0x1f000000 => {
                todo!("Expansion 1 base address"); 
            }
            4 if val != 0x1f802000 => {
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
    const BUS_BEGIN: u32 = 0x1f801000;
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
        0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0x7fffffff, 0x1fffffff, 0xffffffff,
        0xffffffff,
    ];
    address & REGION_MAP[(address >> 29) as usize]
}

const EXP1_BEGIN: u32 = 0x1f000000;
const EXP1_END: u32 = EXP1_BEGIN + 512 * 1024 - 1;

const EXP2_BEGIN: u32 = 0x1f802000;
const EXP2_END: u32 = EXP2_BEGIN + 66 - 1;
