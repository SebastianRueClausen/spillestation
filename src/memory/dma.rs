//! Emulating Direct Memory Access chip. Used to transfer data between devices. The CPU halts when
//! this is running, but the CPU can be allowed to run in intervals called chopping.

#![allow(dead_code)]

use crate::util::bits::BitExtract;
use crate::cpu::{IrqState, Irq};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelPort {
    MdecIn = 0,
    MdecOut = 1,
    Gpu = 2,
    CdRom = 3,
    Spu = 4,
    Pio = 5,
    Otc = 6,
}

impl ChannelPort {
    fn from_value(value: u32) -> Self {
        match value {
            0 => ChannelPort::MdecIn,
            1 => ChannelPort::MdecOut,
            2 => ChannelPort::Gpu,
            3 => ChannelPort::CdRom,
            4 => ChannelPort::Spu,
            5 => ChannelPort::Pio,
            6 => ChannelPort::Otc,
            _ => unreachable!("Invalid MDA Port"),
        }
    }
}

/// Keeps track of size information for channels.
#[derive(Clone, Copy)]
struct BlockControl(u32);

impl BlockControl {
    fn new(value: u32) -> Self {
        Self { 0: value }
    }

    /// Block size - Bits 0..15.
    /// This is only used in request mode.
    fn block_size(self) -> u32 {
        self.0.extract_bits(0, 15)
    }

    /// Block count - Bits 16..31.
    /// In request mode, this is used to determine the amount the amount of blocks. In Manual mode
    /// this is number of words to transfer. Not used for linked list mode.
    fn block_count(self) -> u32 {
        self.0.extract_bits(16, 31)
    }
}

/// DMA can transfer either from or to CPU.
#[derive(Copy, Clone)]
pub enum Direction {
    ToRam,
    ToPort,
}

impl Direction {
    fn from_value(value: u32) -> Self {
        match value {
            0 => Direction::ToRam,
            1 => Direction::ToPort,
            _ => unreachable!("Invalid direction."),
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
            _ => unreachable!("Invalid sync mode."),
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
            _ => unreachable!("Invalid step."),
        }
    }
}

/// Keeps track of size information for channels.
/// - 0 - Transfer direction - to/from RAM.
/// - 1 - Address increment/decrement.
/// - 2 - Chopping mode. Allowing the CPU the run at times.
/// - 9..10 - Sync mode - Manual/Request/Linked List.
/// - 16..18 - Chopping DMA window. How long the CPU are allowed to run when chopping.
/// - 20..22 - Chopping CPU window. How often the CPU is allowed to run.
/// - 24 - Enable flag.
/// - 28 - Manual trigger.
/// - 29..30 - Uknown. Maybe for pausing tranfers.
#[derive(Clone, Copy)]
struct ChannelControl(u32);

impl ChannelControl {
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
        self.0 &= !(1 << 24);
        self.0 &= !(1 << 28);
    }
}

#[derive(Clone, Copy)]
struct Channel {
    /// The base address. Address of the first words the be read/written.
    base: u32,
    block_control: BlockControl,
    control: ChannelControl,
}

impl Channel {
    fn new() -> Self {
        Self {
            base: 0x0,
            block_control: BlockControl::new(0x0),
            control: ChannelControl::new(0x0),
        }
    }

    /// Load a channel register.
    fn load(&self, offset: u32) -> u32 {
        match offset {
            0 => self.base,
            4 => self.block_control.0,
            8 => self.control.0,
            _ => unreachable!("Invalid load in channel at offset {:08x}", offset),
        }
    }

    /// Store value in channel register.
    fn store(&mut self, offset: u32, value: u32) {
        match offset {
            0 => self.base = value.extract_bits(0, 23),
            4 => self.block_control = BlockControl::new(value),
            8 => self.control = ChannelControl::new(value),
            _ => unreachable!("Invalid store at in channel at offset {:08x}", offset),
        }
    }
}

#[derive(Copy, Clone)]
struct Control(u32);

impl Control {
    fn new(value: u32) -> Self {
        Self { 0: value }
    }

    pub fn channel_priority(self, channel: ChannelPort) -> u32 {
        let base = (channel as u32) << 2;
        self.0.extract_bits(base, base + 2)
    }

    pub fn channel_enabled(self, channel: ChannelPort) -> bool {
        let base = (channel as u32) << 2;
        self.0.extract_bit(base + 3) == 1
    }
}

/// DMA Interrupt register.
#[derive(Copy, Clone)]
pub struct Interrupt(u32);

impl Interrupt {
    fn new(value: u32) -> Self {
        Self { 0: value }
    }

    /// If this is set, a interrupt will always be triggered when a channel is done or this
    /// register is written to.
    fn force_irq(self) -> bool {
        self.0.extract_bit(15) == 1
    }

    /// If interrupts are enabled for each channel.
    fn channel_irq_enabled(self, channel: ChannelPort) -> bool {
        self.0.extract_bit((channel as u32) + 16) == 1
    }

    /// Master flag to enabled or disabled interrupts. ['force_irq'] has higher precedence.
    fn master_irq_enabled(self) -> bool {
        self.0.extract_bit(23) == 1
    }

    /// This is set when a channel is done with a transfer, if interrupts are enabled for the
    /// channel.
    fn channel_irq_flag(self, channel: ChannelPort) -> bool {
        self.0.extract_bit((channel as u32) + 24) == 1
    }

    fn set_channel_irq_flag(&mut self, channel: ChannelPort) {
        self.0 |= 1 << 24 + channel as u32;
    }

    /// This is a readonly and is updated whenever ['Interrupt'] is changed in any way.
    fn master_irq_flag(self) -> bool {
        self.0.extract_bit(31) == 1
    }

    /// If this ever get's set, an interrupt is triggered.
    fn update_master_irq_flag(&mut self, irq: &mut IrqState) {
        let enabled = self.0.extract_bits(16, 22);
        let flags = self.0.extract_bits(24, 30);
        let result = self.force_irq() || (self.master_irq_enabled() && (enabled & flags) > 0);
        self.0 |= (result as u32) << 31;
        if result {
            irq.trigger(Irq::Dma); 
        }
    }
}

/// Block transfers are large blocks of memory transferred between RAM and BUS mapped devices.
/// These tranfers could technically be done via CPU load operations, but these transfers are much
/// faster, and therefore widely used, especially when large blocks of data has to be
/// transferred to or from something like VRAM or CDROM.
#[derive(Copy, Clone)]
pub struct BlockTransfer {
    pub port: ChannelPort,
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
#[derive(Copy, Clone)]
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
    control: Control,
    /// Interrupt register.
    pub interrupt: Interrupt,
    channels: [Channel; 7],
}

impl Dma {
    pub fn new() -> Self {
        Self {
            control: Control::new(0x0),
            interrupt: Interrupt::new(0x0),
            channels: [Channel::new(); 7],
        }
    }

    /// Read a register from DMA.
    pub fn load(&self, offset: u32) -> u32 {
        let channel = offset.extract_bits(4, 6);
        let offset = offset.extract_bits(0, 3);
        match channel {
            0..=6 => self.channels[channel as usize].load(offset),
            7 => match offset {
                0 => self.control.0,
                4 => self.interrupt.0,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    /// Store value in DMA register.
    pub fn store(
        &mut self,
        transfers: &mut Transfers,
        irq: &mut IrqState,
        offset: u32,
        value: u32,
    ) {
        let channel = offset.extract_bits(4, 6);
        let offset = offset.extract_bits(0, 3);
        match channel {
            0..=6 => {
                self.channels[channel as usize].store(offset, value);
            }
            7 => match offset {
                0 => self.control.0 = value,
                4 => {
                    self.interrupt.0 = value;
                    self.interrupt.update_master_irq_flag(irq);
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
        self.build_transfers(transfers);
    }

    pub fn channel_done(
        &mut self,
        port: ChannelPort,
        irq: &mut IrqState,
    ) {
        self.channels[port as usize].control.mark_as_finished();
        if self.interrupt.channel_irq_enabled(port) {
            self.interrupt.set_channel_irq_flag(port);
            self.interrupt.update_master_irq_flag(irq);
            if self.interrupt.master_irq_flag() {
                irq.trigger(Irq::Dma);
            }
        }
    }

    fn build_transfers(&mut self, transfers: &mut Transfers) {
        fn increment(step: Step) -> u32 {
            match step {
                Step::Increment => 4,
                Step::Decrement => (-4_i32) as u32,
            }
        }
        for (i, channel) in self.channels.iter().enumerate() {
            match channel.control.sync_mode() {
                SyncMode::Manual if channel.control.start() && channel.control.enabled() => {
                    transfers.block.push(BlockTransfer {
                        port: ChannelPort::from_value(i as u32),
                        direction: channel.control.direction(),
                        size: channel.block_control.block_size(),
                        increment: increment(channel.control.step()),
                        start: channel.base,
                    });
                }
                SyncMode::Request if channel.control.enabled() => {
                    transfers.block.push(BlockTransfer {
                        port: ChannelPort::from_value(i as u32),
                        direction: channel.control.direction(),
                        size: channel.block_control.block_size()
                            * channel.block_control.block_count(),
                        increment: increment(channel.control.step()),
                        start: channel.base,
                    });
                }
                SyncMode::LinkedList if channel.control.enabled() => {
                    assert!(ChannelPort::from_value(i as u32) == ChannelPort::Gpu);
                    transfers.linked.push(LinkedTransfer {
                        start: channel.base,
                    });
                }
                _ => {}
            }
        }
    }
}
