//! Emulating Direct Memory Access chip. Used to transfer data between devices. The CPU halts when
//! this is running, but the CPU can be allowed to run in intervals called chopping.

#![allow(dead_code)]

use crate::util::{BitExtract, BitSet};
use crate::cpu::Irq;
use crate::bus::{BusMap, Schedule, Event};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Port {
    MdecIn = 0,
    MdecOut = 1,
    Gpu = 2,
    CdRom = 3,
    Spu = 4,
    Pio = 5,
    Otc = 6,
}

impl Port {
    fn from_value(value: u32) -> Self {
        match value {
            0 => Port::MdecIn,
            1 => Port::MdecOut,
            2 => Port::Gpu,
            3 => Port::CdRom,
            4 => Port::Spu,
            5 => Port::Pio,
            6 => Port::Otc,
            _ => unreachable!("Invalid MDA Port"),
        }
    }
}

/// Keeps track of size information for channels.
#[derive(Clone, Copy)]
struct BlockCtrl(u32);

impl BlockCtrl {
    fn new(value: u32) -> Self {
        Self { 0: value }
    }

    /// This is only used in request mode.
    fn block_size(self) -> u32 {
        self.0.extract_bits(0, 15)
    }

    /// In request mode, this is used to determine the amount the amount of blocks. In Manual mode
    /// this is number of words to transfer. Not used for linked list mode.
    fn block_count(self) -> u32 {
        self.0.extract_bits(16, 31)
    }
}

/// DMA can transfer either from or to CPU.
#[derive(Debug, Copy, Clone)]
pub enum Direction {
    ToRam,
    ToPort,
}

impl Direction {
    fn from_value(value: u32) -> Self {
        match value {
            0 => Direction::ToRam,
            1 => Direction::ToPort,
            _ => unreachable!("Invalid direction"),
        }
    }
}

/// How to sync the transfer with the rest of the CPU.
#[derive(PartialEq, Eq, Copy, Clone)]
pub enum SyncMode {
    /// Start immediately ad transfer all at once.
    Manual = 0,
    /// Transfer blocks at intervals.
    Request = 1,
    /// Linked list, only used by GPU commands.
    LinkedList = 2,
}

impl SyncMode {
    fn from_value(value: u32) -> Self {
        match value {
            0 => SyncMode::Manual,
            1 => SyncMode::Request,
            2 => SyncMode::LinkedList,
            _ => unreachable!("Invalid sync mode"),
        }
    }
}

/// Where to move from the base value.
#[derive(Copy, Clone)]
pub enum Step {
    Increment,
    Decrement,
}

impl Step {
    fn from_value(value: u32) -> Self {
        match value {
            0 => Step::Increment,
            1 => Step::Decrement,
            _ => unreachable!("Invalid step"),
        }
    }
}

#[derive(Clone, Copy)]
struct ChannelCtrl(u32);

impl ChannelCtrl {
    fn new(value: u32) -> Self {
        Self { 0: value }
    }

    /// Check if Channel is either from or to CPU.
    fn direction(self) -> Direction {
        Direction::from_value(self.0.extract_bit(0))
    }

    fn step(self) -> Step {
        Step::from_value(self.0.extract_bit(1))
    }

    fn chopping_enabled(self) -> bool {
        self.0.extract_bit(8) == 1
    }

    fn sync_mode(self) -> SyncMode {
        SyncMode::from_value(self.0.extract_bits(9, 10))
    }

    fn enabled(self) -> bool {
        self.0.extract_bit(24) == 1
    }

    /// DMA Chopping Window. How long the CPU is allowed to run when chopping.
    fn dma_chopping_window(self) -> u32 {
        self.0.extract_bits(16, 18) << 1
    }

    /// CPU Chopping Window. How often the CPU is allowed to run.
    fn cpu_chopping_window(self) -> u32 {
        self.0.extract_bits(20, 22) << 1
    }

    fn start(self) -> bool {
        self.0.extract_bit(28) == 1
    }

    fn mark_as_finished(&mut self) {
        // Clear both enabled and start flags.
        self.0.set_bit(24, false);
        self.0.set_bit(28, false);
    }

    fn store(&mut self, port: Port, val: u32) {
        if port == Port::Otc {
            self.0 &= 0x5100_0000;
            self.0 |= 2;
        }
        self.0 = val;
        if self.chopping_enabled() {
            warn!("Chopping enabled for port {:?}", port);
        }
    }
}

#[derive(Clone, Copy)]
struct Channel {
    port: Port,
    /// The base address. Address of the first words the be read/written.
    base: u32,
    block_ctrl: BlockCtrl,
    ctrl: ChannelCtrl,
}

impl Channel {
    fn new(port: Port) -> Self {
        Self {
            port,
            base: 0x0,
            block_ctrl: BlockCtrl::new(0x0),
            ctrl: ChannelCtrl::new(0x0),
        }
    }

    /// Load a channel register.
    fn load(&self, offset: u32) -> u32 {
        match offset {
            0 => self.base,
            4 => self.block_ctrl.0,
            8 => self.ctrl.0,
            _ => unreachable!("Invalid load in channel at offset {:08x}", offset),
        }
    }

    /// Store value in channel register.
    fn store(&mut self, offset: u32, value: u32) {
        match offset {
            0 => self.base = value.extract_bits(0, 23),
            4 => self.block_ctrl = BlockCtrl::new(value),
            8 => {
                self.ctrl.store(self.port, value);
                if self.ctrl.chopping_enabled() {
                    warn!("DMA chopping enabled");
                }
            },
            _ => unreachable!("Invalid store at in channel at offset {:08x}", offset),
        }
    }
}

#[derive(Copy, Clone)]
struct CtrlReg(u32);

impl CtrlReg {
    fn new(value: u32) -> Self {
        Self { 0: value }
    }

    pub fn channel_priority(self, channel: Port) -> u32 {
        let base = (channel as u32) << 2;
        self.0.extract_bits(base, base + 2)
    }

    pub fn channel_enabled(self, channel: Port) -> bool {
        let base = (channel as u32) << 2;
        self.0.extract_bit(base + 3) == 1
    }
}

/// DMA Interrupt register.
#[derive(Copy, Clone)]
pub struct IrqReg(u32);

impl IrqReg {
    fn new(value: u32) -> Self {
        Self { 0: value }
    }

    /// If this is set, a interrupt will always be triggered when a channel is done or this
    /// register is written to.
    fn force_irq(self) -> bool {
        self.0.extract_bit(15) == 1
    }

    /// If interrupts are enabled for each channel.
    fn channel_irq_enabled(self, channel: Port) -> bool {
        self.0.extract_bit((channel as u32) + 16) == 1
    }

    /// Master flag to enabled or disabled interrupts. ['force_irq'] has higher precedence.
    fn master_irq_enabled(self) -> bool {
        self.0.extract_bit(23) == 1
    }

    /// This is set when a channel is done with a transfer, if interrupts are enabled for the
    /// channel.
    fn channel_irq_flag(self, channel: Port) -> bool {
        self.0.extract_bit((channel as u32) + 24) == 1
    }

    fn set_channel_irq_flag(&mut self, channel: Port) {
        self.0.set_bit(24 + channel as usize, true);
    }

    /// This is a readonly and is updated whenever ['Interrupt'] is changed in any way.
    fn master_irq_flag(self) -> bool {
        self.0.extract_bit(31) == 1
    }

    /// If this ever get's set, an interrupt is triggered.
    fn update_master_irq_flag(&mut self, schedule: &mut Schedule) {
        // If this is true, then the DMA should trigger an interrupt if 'master_irq_flag' isn't
        // already on. If the force_irq flag is set, then it will always trigger an interrupt.
        // Otherwise it will trigger if any of the flags are set and 'master_irq_enabled' is on.
        let result = self.force_irq()
            || self.master_irq_enabled()
            && self.0.extract_bits(24, 30) != 0;
        if result {
            if !self.master_irq_flag() {
                self.0.set_bit(31, true);
                schedule.schedule_now(Event::IrqTrigger(Irq::Dma));
            }
        } else {
            self.0.set_bit(31, false);
        }
    }

    fn store(&mut self, schedule: &mut Schedule, val: u32) {
        let mask = 0x00ff_803f;
        self.0 &= !mask;
        self.0 |= val & mask;
        self.0 &= !(val & 0x7f00_0000);
        self.update_master_irq_flag(schedule);
    }
}

/// Block transfers are large blocks of memory transferred between RAM and BUS mapped devices.
/// These tranfers could technically be done via CPU load operations, but these transfers are much
/// faster, and therefore widely used, especially when large blocks of data has to be
/// transferred to or from something like VRAM or CDROM.
#[derive(Debug, Copy, Clone)]
pub struct BlockTransfer {
    pub port: Port,
    pub direction: Direction,
    /// The starting address of the transfer.
    pub start: u32,
    /// The size of the transfer.
    pub size: u32,
    /// The increment each step. This is required since the start address may be both the highest
    /// or lowest address.
    pub increment: u32,
}

/// Linked list transfers are only used for GPU commands. It basically continues to transfer data
/// until the end of a linked list is reached.
#[derive(Debug, Copy, Clone)]
pub struct LinkedTransfer {
    pub start: u32,
}

pub struct Transfers {
    pub block: Vec<BlockTransfer>,
    pub linked: Vec<LinkedTransfer>,
}

impl Transfers {
    pub fn new() -> Self {
        Self {
            block: vec![],
            linked: vec![],
        }
    }
}

/// The DMA is a chip used by the Playstation to transfer data between BUS mapped devices. It can
/// be a lot faster than CPU loads, even though the CPU is stopped during transfers. It also allows
/// for breaking up large transfers, to give the CPU time during transfers.
pub struct Dma {
    /// Control register. 
    ctrl: CtrlReg,
    /// Interrupt register.
    pub irq: IrqReg,
    channels: [Channel; 7],
}

impl Dma {
    pub fn new() -> Self {
        Self {
            ctrl: CtrlReg::new(0x0),
            irq: IrqReg::new(0x0),
            channels: [
                Channel::new(Port::MdecIn),
                Channel::new(Port::MdecOut),
                Channel::new(Port::Gpu),
                Channel::new(Port::CdRom),
                Channel::new(Port::Spu),
                Channel::new(Port::Pio),
                Channel::new(Port::Otc),
            ],
        }
    }

    /// Read a register from DMA.
    pub fn load(&self, offset: u32) -> u32 {
        let channel = offset.extract_bits(4, 6);
        let reg = offset.extract_bits(0, 3);
        match channel {
            0..=6 => self.channels[channel as usize].load(reg),
            7 => match reg {
                0 => self.ctrl.0,
                4 => self.irq.0,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    /// Store value in DMA register.
    pub fn store(
        &mut self,
        transfers: &mut Transfers,
        schedule: &mut Schedule,
        offset: u32,
        value: u32,
    ) {
        let channel = offset.extract_bits(4, 6);
        let reg = offset.extract_bits(0, 3);
        match channel {
            0..=6 => self.channels[channel as usize].store(reg, value),
            7 => match reg {
                0 => self.ctrl.0 = value,
                4 => self.irq.store(schedule, value),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
        self.build_transfers(transfers);
    }

    /// Mark channel as done.
    pub fn channel_done(&mut self, port: Port, schedule: &mut Schedule) {
        self.channels[port as usize].ctrl.mark_as_finished();
        if self.irq.channel_irq_enabled(port) {
            self.irq.set_channel_irq_flag(port);
        }
        self.irq.update_master_irq_flag(schedule);
    }

    /// Build transfer command for the BUS to execute.
    fn build_transfers(&mut self, trans: &mut Transfers) {
        fn increment(step: Step) -> u32 {
            match step {
                Step::Increment => 4,
                Step::Decrement => (-4_i32) as u32,
            }
        }
        for (i, ch) in self.channels.iter().enumerate() {
            match ch.ctrl.sync_mode() {
                SyncMode::Manual if ch.ctrl.start() && ch.ctrl.enabled() => {
                    let size = ch.block_ctrl.block_size();
                    trans.block.push(BlockTransfer {
                        port: ch.port,
                        size,
                        direction: ch.ctrl.direction(),
                        increment: increment(ch.ctrl.step()),
                        start: ch.base,
                    });
                }
                SyncMode::Request if ch.ctrl.enabled() => {
                    let size = ch.block_ctrl.block_size() * ch.block_ctrl.block_count();
                    trans.block.push(BlockTransfer {
                        port: ch.port,
                        size,
                        direction: ch.ctrl.direction(),
                        increment: increment(ch.ctrl.step()),
                        start: ch.base,
                    });
                }
                SyncMode::LinkedList if ch.ctrl.enabled() => {
                    debug_assert!(Port::from_value(i as u32) == Port::Gpu);
                    trans.linked.push(LinkedTransfer {
                        start: ch.base,
                    });
                }
                _ => {}
            }
        }
    }
}

impl BusMap for Dma {
    const BUS_BEGIN: u32 = 0x1f801080;
    const BUS_END: u32 = Self::BUS_BEGIN + 128 - 1;
}
