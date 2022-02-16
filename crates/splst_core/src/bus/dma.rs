//! Emulating Direct Memory Access chip. Used to transfer data between devices. The CPU halts when
//! this is running, but the CPU can be allowed to run in intervals called chopping.
//!
//! TODO:
//! * Add timings for transfers.

use splst_util::{Bit, BitSet};

use crate::cpu::Irq;
use crate::Cycle;
use crate::bus::{Ram, Word, Bus, BusMap, Schedule, Event};

use std::ops::{Index, IndexMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Port {
    MdecIn = 0,
    MdecOut = 1,
    Gpu = 2,
    CdRom = 3,
    Spu = 4,
    Pio = 5,
    /// Depth ordering table. It's only used to initialize/reset it.
    Otc = 6,
}

/// Register holding the size information for manual and request transfers.
#[derive(Clone, Copy)]
struct BlockCtrl {
    size: u16,
    count: u16,
}

impl BlockCtrl {
    fn new(val: u32) -> Self {
        Self {
            size: val.bit_range(0, 15) as u16,
            count: val.bit_range(16, 31) as u16,
        }
    }

    fn load(self) -> u32 {
        self.size as u32 | (self.count as u32) << 16
    }
}

/// DMA can transfer either from or to RAM.
#[derive(Debug, Copy, Clone)]
pub enum ChanDir {
    ToRam,
    ToPort,
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum SyncMode {
    /// Start immediately and transfer all at once. Used to send textures to the VRAM and
    /// initializing the ordering table.
    Manual = 0,
    /// Transfer blocks when the signaled by the devices.
    Request = 1,
    /// A linked list of (generally) smaller blocks. It's only used to send commands to GP0.
    LinkedList = 2,
}

/// Which way to step from the base address. Either increment or decrement one word.
#[derive(Copy, Clone)]
pub enum Step {
    Inc,
    Dec,
}

impl Step {
    /// The step amount. This is the amount to add to the base address each word and uses
    /// wrap around to avoid branching each word transfered.
    fn step_amount(self) -> u32 {
        match self {
            Step::Inc => 4,
            Step::Dec => (-4_i32) as u32,
        }
    }
}

#[derive(Clone, Copy)]
struct ChanCtrl(u32);

impl ChanCtrl {
    /// Check if Channel is either from or to CPU.
    fn direction(self) -> ChanDir {
        match self.0.bit(0) {
            false => ChanDir::ToRam,
            true => ChanDir::ToPort,
        }
    }

    fn step(self) -> Step {
        match self.0.bit(1) {
            false => Step::Inc,
            true => Step::Dec,
        }
    }

    /// Chopping means that the CPU get's to run at intervals while transfering.
    fn chopping_enabled(self) -> bool {
        self.0.bit(8)
    }

    fn sync_mode(self) -> SyncMode {
        match self.0.bit_range(9, 10)  {
            0 => SyncMode::Manual,
            1 => SyncMode::Request,
            2 => SyncMode::LinkedList,
            _ => unreachable!("Invalid sync mode"),
        }
    }

    /// If the channel itself is enabled. If it's not, then the channel doesn't run.
    fn enabled(self) -> bool {
        self.0.bit(24)
    }

    /// How many cycles to run in the interval between CPU chop.
    fn dma_chop_size(self) -> u32 {
        self.0.bit_range(16, 18) << 1
    }

    /// How many cycles the CPU get's to run when chopping.
    fn cpu_chop_size(self) -> Cycle {
        (self.0.bit_range(20, 22) << 1) as Cycle
    }

    /// This is only used when in manual sync mode. It must be set for the transfer to start.
    fn start(self) -> bool {
        self.0.bit(28)
    }

    fn mark_as_finished(&mut self) {
        // Clear both enabled and start flags.
        self.0 = self.0
            .set_bit(24, false)
            .set_bit(28, false);
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

#[derive(Debug)]
struct Transfer {
    cursor: u32,
    size: u32,
    /// The increment each step. This is required since the start address may be both the highest
    /// or lowest address.
    inc: u32,
}

/// The registers and info about a DMA channel.
pub struct ChanStat {
    port: Port,
    base: u32,
    block_ctrl: BlockCtrl,
    ctrl: ChanCtrl,
    transfer: Option<Transfer>,
}

impl ChanStat {
    fn new(port: Port) -> Self {
        Self {
            port,
            base: 0x0,
            block_ctrl: BlockCtrl::new(0x0),
            ctrl: ChanCtrl(0x0),
            transfer: None,
        }
    }

    /// Load a channel register.
    fn load(&self, offset: u32) -> u32 {
        match offset {
            0 => self.base,
            4 => self.block_ctrl.load(),
            8 => self.ctrl.0,
            _ => unreachable!("Invalid load in channel at offset {offset}"),
        }
    }

    /// Store value in channel register.
    fn store(&mut self, offset: u32, val: u32) {
        match offset {
            0 => self.base = val.bit_range(0, 23),
            4 => self.block_ctrl = BlockCtrl::new(val),
            8 => self.ctrl.store(self.port, val),
            _ => unreachable!("Invalid store at in channel at offset {:08x}", offset),
        }
    }
}

// TODO: Add support for this.
#[derive(Copy, Clone)]
struct CtrlReg(u32);

impl CtrlReg {
    #[allow(dead_code)]
    pub fn channel_priority(self, channel: Port) -> u32 {
        let base = (channel as usize) << 2;
        self.0.bit_range(base, base + 2)
    }

    #[allow(dead_code)]
    pub fn channel_enabled(self, channel: Port) -> bool {
        let base = (channel as usize) << 2;
        self.0.bit(base + 3)
    }
}

/// DMA Interrupt register.
#[derive(Copy, Clone)]
pub struct IrqReg(u32);

impl IrqReg {
    /// If this is set, a interrupt will always be triggered when a channel is done or this
    /// register is written to.
    fn force_irq(self) -> bool {
        self.0.bit(15)
    }

    /// If interrupts are enabled for each channel.
    fn channel_irq_enabled(self, channel: Port) -> bool {
        self.0.bit(channel as usize + 16)
    }

    /// Master flag to enabled or disabled interrupts. ['force_irq'] has higher precedence.
    fn master_irq_enabled(self) -> bool {
        self.0.bit(23)
    }

    /// This is set when a channel is done with a transfer, if interrupts are enabled for the
    /// channed.
    #[allow(dead_code)]
    fn channel_irq_flag(self, channel: Port) -> bool {
        self.0.bit(channel as usize + 24)
    }

    fn set_channel_irq_flag(&mut self, channel: Port) {
        self.0 = self.0.set_bit(24 + channel as usize, true);
    }

    /// This is a readonly and is updated whenever ['Interrupt'] is changed in any way.
    fn master_irq_flag(self) -> bool {
        self.0.bit(31)
    }

    /// If this ever get's set, an interrupt is triggered.
    fn update_master_irq_flag(&mut self, schedule: &mut Schedule) {
        // If this is true, then the DMA should trigger an interrupt if 'master_irq_flag' isn't
        // already on. If the force_irq flag is set, then it will always trigger an interrupt.
        // Otherwise it will trigger if any of the flags are set and 'master_irq_enabled' is on.
        let result = self.force_irq()
            || self.master_irq_enabled()
            && self.0.bit_range(24, 30) != 0;

        if result {
            if !self.master_irq_flag() {
                self.0 = self.0.set_bit(31, true);
                schedule.schedule_now(Event::IrqTrigger(Irq::Dma));
            }
        } else {
            self.0 = self.0.set_bit(31, false);
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

/// The DMA is a chip used by the Playstation to transfer data between RAM and some BUS mapped devices.
/// It can be a lot faster than CPU loads, even though the CPU is stopped during transfers.
///
/// # Chopping
///
/// Because the CPU doesn't get to run during transfers, the DMA allows for chopping. Chopping
/// is a feature allowing the CPU to run for a given amount of cycles at a given interval while
/// transfering. It's likely to allow games to handle input and rendering and such while handling
/// a large and slow transfer from something like the CDROM.
pub struct Dma {
    /// Control register. 
    ctrl: CtrlReg,
    /// Interrupt register.
    pub irq: IrqReg,
    channels: [ChanStat; 7],
}

impl Dma {
    pub fn new() -> Self {
        Self {
            ctrl: CtrlReg(0x7654321),
            irq: IrqReg(0),
            channels: [
                ChanStat::new(Port::MdecIn),
                ChanStat::new(Port::MdecOut),
                ChanStat::new(Port::Gpu),
                ChanStat::new(Port::CdRom),
                ChanStat::new(Port::Spu),
                ChanStat::new(Port::Pio),
                ChanStat::new(Port::Otc),
            ],
        }
    }

    pub fn load(&self, offset: u32) -> u32 {
        let chan = offset.bit_range(4, 6);
        let reg = offset.bit_range(0, 3);

        match chan {
            0..=6 => self.channels[chan as usize].load(reg),
            7 => match reg {
                0 => self.ctrl.0,
                4 => self.irq.0,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    pub fn store(&mut self, schedule: &mut Schedule, offset: u32, value: u32) {
        let chan = offset.bit_range(4, 6);
        let reg = offset.bit_range(0, 3);

        match chan {
            0..=6 => self.channels[chan as usize].store(reg, value),
            7 => match reg {
                0 => self.ctrl.0 = value,
                4 => self.irq.store(schedule, value),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    /// Mark channel as done.
    fn channel_done(&mut self, port: Port, schedule: &mut Schedule) {
        self.channels[port as usize].ctrl.mark_as_finished();

        if self.irq.channel_irq_enabled(port) {
            self.irq.set_channel_irq_flag(port);
        }

        self.irq.update_master_irq_flag(schedule);
    }

    /// Run DMA channel. This performs any transfers ready to be executed for a channel.
    /// If chopping is disabled for the channel, then the whole transfer get's executed
    /// at once. If chopping is enabled, then the Transfer runs for a given number of cycles,
    /// and if the transfer isn't done in time, the rest get's transfered after the CPU is allowed
    /// to run for a given number of cycles.
    fn run_chan<T: DmaChan>(
        &mut self,
        port: Port,
        chan: &mut T,
        schedule: &mut Schedule,
        ram: &mut Ram
    ) {
        let ctrl = self[port].ctrl;

        let done = if ctrl.chopping_enabled() {
            ctrl.dma_chop_size() as Cycle + schedule.cycle()
        } else {
            Cycle::MAX
        };

        let mut manual_done = false;

        while schedule.cycle() < done
            && self[port].ctrl.enabled()
            && chan.dma_ready(self[port].ctrl.direction())
        {
            let stat = &mut self[port];

            let mut tran = match stat.transfer.take() {
                Some(tran) => tran,
                None => match stat.ctrl.sync_mode() {
                    SyncMode::Manual => {
                        if manual_done {
                            self.channel_done(port, schedule);
                            return;
                        }

                        // For manual transfers the start flag must be set as opposed to the other
                        // sync modes.
                        if !stat.ctrl.start() {
                            return;
                        }

                        manual_done = true;

                        Transfer {
                            inc: stat.ctrl.step().step_amount(),
                            size: stat.block_ctrl.size as u32,
                            cursor: stat.base
                        }
                    }
                    SyncMode::Request => {
                        if let Some(blocks) = stat.block_ctrl.count.checked_sub(1) {
                            stat.block_ctrl.count = blocks;

                            Transfer {
                                inc: stat.ctrl.step().step_amount(),
                                size: stat.block_ctrl.size as u32,
                                cursor: stat.base,
                            }
                        } else {
                            self.channel_done(port, schedule);
                            return;
                        }
                    },
                    SyncMode::LinkedList => {
                        if stat.base != 0x00ff_ffff {
                            let header = ram.load::<Word>(stat.base & 0x001f_fffc);

                            let tran = Transfer {
                                inc: stat.ctrl.step().step_amount(),
                                size: header.bit_range(24, 31),
                                cursor: (stat.base + 4).bit_range(0, 23),
                            };

                            stat.base = header.bit_range(0, 23);

                            if tran.size == 0 {
                                continue;
                            }

                            tran
                        } else {
                            self.channel_done(port, schedule);
                            return;
                        }
                    }
                }
            };

            // Transfer a single block. If it's in in manual mode, this does the whole transfer, if
            // it's in request mode, then it transfer a single block and if it's in linked mode,
            // this does transfers a single node. It will stop in the middle of a transfer if 
            // chopping is enabled and it runs out of cycles.
            self[port].transfer = match stat.ctrl.direction() {
                ChanDir::ToRam => {
                    loop {
                        if schedule.cycle() > done {
                            let stat = &mut self[port];
                           
                            // If the channel is in manual sync mode, then the base address will
                            // only get updated during if chopping is enabled. Should it hold for
                            // request mode as well?
                            if let SyncMode::Manual = stat.ctrl.sync_mode() {
                                stat.base = tran.cursor;
                            }

                            schedule.schedule_in(
                                stat.ctrl.cpu_chop_size(),
                                Event::RunDmaChan(port)
                            );

                            break Some(tran); 
                        }

                        if let Some(size) = tran.size.checked_sub(1) {
                            let addr = tran.cursor & 0x001f_fffc;
                            let val = chan.dma_load(schedule, (tran.size as u16, tran.cursor));

                            ram.store::<Word>(addr, val);

                            tran.cursor = tran.cursor.wrapping_add(tran.inc) & 0x00ff_ffff;
                            tran.size = size;
                        } else {
                            let stat = &mut self[port];

                            // Request mode and manual mode with chopping enabled will point at the
                            // end of the transfer when the transfer is done.
                            let update_base = stat.ctrl.chopping_enabled()
                                && stat.ctrl.sync_mode() == SyncMode::Manual
                                || stat.ctrl.sync_mode() == SyncMode::Request;

                            if update_base {
                                stat.base = tran.cursor; 
                            }

                            break None;
                        }

                        schedule.tick(1);
                    }
                }
                ChanDir::ToPort => {
                    loop {
                        if schedule.cycle() > done {
                            let stat = &mut self[port];
                           
                            if let SyncMode::Manual = stat.ctrl.sync_mode() {
                                stat.base = tran.cursor;
                            }

                            schedule.schedule_in(
                                stat.ctrl.cpu_chop_size(),
                                Event::RunDmaChan(port)
                            );

                            break Some(tran); 
                        }

                        if let Some(size) = tran.size.checked_sub(1) {
                            let addr = tran.cursor & 0x001f_fffc;
                            let val = ram.load::<Word>(addr);

                            chan.dma_store(schedule, val);

                            tran.cursor = tran.cursor.wrapping_add(tran.inc) & 0x00ff_ffff;
                            tran.size = size;
                        } else {
                            let stat = &mut self[port];

                            let update_base = stat.ctrl.chopping_enabled()
                                && stat.ctrl.sync_mode() == SyncMode::Manual
                                || stat.ctrl.sync_mode() == SyncMode::Request;

                            if update_base {
                                stat.base = tran.cursor; 
                            }

                            break None;
                        }

                        schedule.tick(1);
                    }
                }
            };
        }
    }
}

impl Bus {
    pub fn run_dma(&mut self) {
        self.dma.run_chan(
            Port::Gpu,
            &mut self.gpu,
            &mut self.schedule,
            &mut self.ram
        );
        self.dma.run_chan(
            Port::CdRom,
            &mut self.cdrom,
            &mut self.schedule,
            &mut self.ram
        );
        self.dma.run_chan(
            Port::Otc,
            &mut OrderingTable,
            &mut self.schedule,
            &mut self.ram
        );
    }

    pub fn run_dma_chan(&mut self, port: Port) {
        match port {
            Port::Gpu => {
               self.dma.run_chan(Port::Gpu, &mut self.gpu, &mut self.schedule, &mut self.ram);
            }
            Port::Otc => {
                self.dma.run_chan(Port::Otc, &mut OrderingTable, &mut self.schedule, &mut self.ram);
            }
            _ => todo!(),
        }
    }
}


/// The ordering table. This is used by the Playstation to order draw calls. The playstation stores
/// all draw calls in a scene in a compact buffer somewhere in RAM. The playstation however,
/// doesn't have a Z-buffer or anything like that, so it can't just execute the drawcalls in an
/// arbitrary order. It must send them to the GPU in a correct order to render scenes correctly.
/// This has to be done every frame, since objects and the camera could have moved. To do this
/// effeciently, the Playstation uses a depth ordering table. Each element in the table is 32 bit
/// wide and contains two elements; An 8 bit offset into the drawcall buffer, and a 24 bit pointer
/// to the next element.
///
/// The DMA is used to create an empty table of a given size. Here each element starts out by with
/// an empty offset and the pointer pointing to the previous element in the table, and the last one
/// with the value ffffffh.
///
/// When the Playstation wants to draw a line or polygon it calculates its distance to the camera
/// and uses that value to determine a slot in the ordering table. It then inserts the drawcall at
/// that slot. Since the Playstation doesn't have a lot of RAM to work with, there is often a lot
/// more objects to draw than slots in the ordering table, so many drawcalls share the same cell in
/// the ordering table. When that happens the Playstation draws the elements from newest to oldest.
/// This is likely random and causes visual glitches.
struct OrderingTable;

impl DmaChan for OrderingTable {
    fn dma_load(&mut self, _: &mut Schedule, stats: (u16, u32)) -> u32 {
        let (words_left, addr) = stats;
        if words_left == 1 {
            0x00ff_ffff
        } else {
            addr.wrapping_sub(4).bit_range(0, 21)
        }
    }

    fn dma_store(&mut self, _: &mut Schedule, _: u32) {
        warn!("Ordering table DMA store");
    }

    fn dma_ready(&self, _: ChanDir) -> bool {
        true
    }
}

impl Index<Port> for Dma {
    type Output = ChanStat;

    fn index(&self, port: Port) -> &Self::Output {
        &self.channels[port as usize]
    }
}

impl IndexMut<Port> for Dma {
    fn index_mut(&mut self, port: Port) -> &mut Self::Output {
        &mut self.channels[port as usize]
    }
}

pub trait DmaChan {
    fn dma_load(&mut self, schedule: &mut Schedule, stats: (u16, u32)) -> u32;
    fn dma_store(&mut self, schedule: &mut Schedule, val: u32);
    fn dma_ready(&self, dir: ChanDir) -> bool;
}

impl BusMap for Dma {
    const BUS_BEGIN: u32 = 0x1f801080;
    const BUS_END: u32 = Self::BUS_BEGIN + 128 - 1;
}
