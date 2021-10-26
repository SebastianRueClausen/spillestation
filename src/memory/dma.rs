//! Emulating Direct Memory Access chip. Used to transfer data between devices. The CPU halts when
//! this is running, but the CPU can be allowed to run in intervals called chopping.

use super::Bus;
use crate::bits::BitExtract;

#[derive(Clone, Copy)]
enum ChannelId {
    MdecIn = 0,
    MdecOut = 1,
    Gpu = 2,
    CdRom = 3,
    Spu = 4,
    Pio = 5,
    Otc = 6,
}

impl ChannelId {
    fn from_value(value: u32) -> Self {
        match value {
            0 => ChannelId::MdecIn,
            1 => ChannelId::MdecOut,
            2 => ChannelId::Gpu,
            3 => ChannelId::CdRom,
            4 => ChannelId::Spu,
            5 => ChannelId::Pio,
            6 => ChannelId::Otc,
            _ => unreachable!("Invalid MDA Port"),
        }
    }
}

/// An ongoing transfer.
#[derive(Clone, Copy)]
pub struct Transfer {
    /// The number of words left.
    pub remaining_words: u16,
    /// The current address of the transfer.
    pub address: u32,
    pub cycles: u32,
}

/// Keeps track of size information for channels.
#[derive(Clone, Copy)]
struct BlockControl(u32);

impl BlockControl {
    fn new(value: u32) -> Self {
        Self {
            0: value
        }
    }

    /// Block size - Bits 0..15.
    /// This is only used in request mode.
    fn block_size(self) -> u32 {
        self.0.extract_bits(0, 15)
    }

    /// Block count - Bites 16..31.
    /// In request mode, this is used to determine the amount the amount of blocks. In Manual mode
    /// this is number of words to transfer. Not used for linked list mode.
    fn block_count(self) -> u32 {
        self.0.extract_bits(16, 31)
    }
}

/// DMA can transfer either from or to CPU.
#[derive(Clone, Copy)]
enum Direction {
    ToCpu,
    ToDma,
}

impl Direction {
    fn from_value(value: u32) -> Self {
        match value {
            0 => Direction::ToCpu,
            1 => Direction::ToDma,
            _ => unreachable!("Invalid direction"),
        }
    }
}

/// How to sync the transfer with the rest of the CPU.
#[derive(PartialEq, Eq)]
enum SyncMode {
    /// Start immediately ad transfer all at once.
    Manual,
    /// Transfer blocks at intervals.
    Request,
    /// Linked list, only used by GPU commands.
    LinkedList,
}

impl SyncMode {
    fn from_value(value: u32) -> Self {
        match value {
            0 => SyncMode::Manual,
            1 => SyncMode::Request,
            2 => SyncMode::LinkedList,
            _ => panic!("Invalid sync mode"),
        }
    }
}

/// Where to move from the base value.
enum Step {
    Forward,
    Backward,
}

impl Step {
    fn from_value(value: u32) -> Self {
        match value {
            0 => Step::Forward,
            1 => Step::Backward,
            _ => panic!("Invalid step"),
        }
    }
}

/// Keeps track of size information for channels.
#[derive(Clone, Copy)]
struct ChannelControl(u32);

impl ChannelControl {
    fn new(value: u32) -> Self {
        Self {
            0: value
        }
    }

    /// Direction - Bit 0.
    /// Check if Channel is either from or to CPU.
    fn direction(self) -> Direction {
        Direction::from_value(self.0.extract_bit(0))
    }

    /// Step - Bit 1.
    fn step(self) -> Step {
        Step::from_value(self.0.extract_bit(1))
    }

    fn chopping_enabled(self) -> bool {
        self.0.extract_bit(8) == 1
    }

    /// Sync Mode - Bits 9..10.
    fn sync_mode(self) -> SyncMode {
        SyncMode::from_value(self.0.extract_bits(9, 10))
    }

    /// Start busy - bit 24.
    fn transfer_busy(self) -> bool {
        self.0.extract_bit(24) == 1
    }

    /// DMA Chopping Window - Bits 16..18.
    /// How long the CPU is allowed to run when chopping.
    fn dma_chopping_window(self) -> u32 {
        self.0.extract_bits(16, 18) << 1
    }

    /// CPU Chopping Window - Bits 20..22.
    /// How often the CPU is allowed to run.
    fn cpu_chopping_window(self) -> u32 {
        self.0.extract_bits(20, 22) << 1
    }

   fn start(self) -> bool {
        self.0.extract_bit(28) == 1
    }
}


#[derive(Clone, Copy)]
struct Channel {
    /// The base address. Address of the first words the be read/written.
    base: u32,
    block_control: BlockControl,
    /// Channel control:
    ///  - [0] - Transfer direction - to/from RAM.
    ///  - [1] - Address increment/decrement.
    ///  - [2] - Chopping mode. Allowing the CPU the run at times.
    ///  - [9..10] - Sync mode - Manual/Request/Linked List.
    ///  - [16 - 18] - Chopping DMA window. How long the CPU are allowed to run when chopping.
    ///  - [20..22] - Chopping CPU window. How often the CPU is allowed to run.
    ///  - [24] - Enable flag.
    ///  - [28] - Manual trigger.
    ///  - [29..30] - Uknown. Maybe for pausing tranfers. TODO: Experiment.
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
            _ => panic!("Invalid load at offset {:08x}", offset),
        }
    }

    /// Store value in channel register.
    fn store(&mut self, offset: u32, value: u32) {
        match offset {
            0 => self.base = value,
            4 => self.block_control = BlockControl::new(value),
            8 => self.control = ChannelControl::new(value),
            _ => panic!("Invalid store at offset {:08x}", offset),
        }
    }
}

#[derive(Copy, Clone)]
struct Control(u32);

impl Control {
    fn new(value: u32) -> Self {
        Self {
            0: value,
        }
    }

    pub fn channel_priority(self, channel: ChannelId) -> u32 {
        let base = (channel as u32) << 2;
        self.0.extract_bits(base, base + 2)
    }

    pub fn channel_enabled(self, channel: ChannelId) -> bool {
        let base = (channel as u32) << 2;
        self.0.extract_bit(base + 3) == 1
    }
}

#[derive(Copy, Clone)]
struct Interrupt(u32);

impl Interrupt {
    fn new(value: u32) -> Self {
        Self {
            0: value,
        }
    }

    pub fn force_irq(self) -> bool {
        self.0.extract_bit(15) == 1
    }

    pub fn channel_irq_enabled(self, channel: ChannelId) -> bool {
        self.0.extract_bit((channel as u32) + 16) == 1
    }

    pub fn master_irq_enabled(self) -> bool {
        self.0.extract_bit(23) == 1
    }

    pub fn channel_irq_flag(self, channel: ChannelId) -> bool {
        self.0.extract_bit((channel as u32) + 24) == 1
    }

    pub fn master_irq_flag(self) -> bool {
        self.0.extract_bit(31) == 1
    }

    pub fn update_master_irq_flag(&mut self) {
        let enabled = self.0.extract_bits(16, 22);
        let flags = self.0.extract_bits(24, 30);
        let result = self.force_irq() || (self.master_irq_enabled() && (enabled & flags) > 0);
        self.0 |= (result as u32) << 31;
    }
}


pub struct Dma {
    control: Control,
    interrupt: Interrupt,
    pub channels: [Channel; 7],
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
        let channel = (offset & 0x70) >> 4;
        let offset = offset & 0xc;
        match channel {
            0..=6 => {
                self.channels[channel as usize].load(offset)
            },
            7 => match offset {
                0 => self.control.0,
                4 => self.interrupt.0,
                _ => panic!("Invalid DMA register {:08x}", offset),
            }
            _ => unreachable!(),
        }
    }

    /// Store value in DMA register.
    pub fn store(&mut self, offset: u32, value: u32) {
        let channel = (offset & 0x70) >> 4;
        let register = offset & 0xc;
        match channel {
            0..=6 => {
                self.channels[channel as usize].store(register, value);
            },
            7 => {
                match register {
                    0 => self.control.0 = value,
                    4 => {
                        self.interrupt.0 = value;
                        self.interrupt.update_master_irq_flag();
                    }
                    _ => panic!("Invalid register {:08x}", offset),
                }
            }
            _ => unreachable!(),
        }
    }
}
