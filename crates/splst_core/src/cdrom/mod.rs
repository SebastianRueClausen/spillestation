//! TODO:
//! * The CDROM runs at fixed intervals which doesn't seem neccessary but running after every store
//!   doesn't seem to work, likely because the CDROM executes the command immediately, which the
//!   BIOS isn't expecting. 

mod fifo;
pub mod disc;

use splst_util::Bit;
use splst_util::{Bcd, Msf};
use splst_cdimg::Sector;

use crate::cpu::Irq;
use crate::bus::{AddrUnit, BusMap};
use crate::bus::{DmaChan, ChanDir};
use crate::schedule::{Schedule, Event};
use crate::Cycle;

pub use disc::Disc;
use fifo::Fifo;

use std::fmt;

pub struct CdRom {
    disc: Disc,
    state: DriveState,
    /// The index register. This decides what happens when the CPU writes to and
    /// loads from the CDROM.
    index: u8,
    /// Which ['Interrupt']s are enabled.
    irq_mask: u8,
    /// Which ['Interrupt']s are active.
    irq_flags: u8,
    /// The CDROM may or may not have a command waiting to be executed.
    cmd: Option<u8>,
    /// Responses from commands.
    response_fifo: Fifo,
    /// Arguments to commands.
    arg_fifo: Fifo,
    position: Msf,
    pending_seek: Option<Msf>,
    mode: ModeReg,
    sector: Sector,
    data_buffer: DataBuffer,
}

impl CdRom {
    pub fn new(disc: Disc) -> Self {

        // TODO: Check startup value.
        let mode = ModeReg(0x0);
        let position = Msf::ZERO;

        Self {
            disc,
            state: DriveState::Idle,
            index: 0x0,
            irq_mask: 0x0,
            irq_flags: 0x0,
            cmd: None,
            response_fifo: Fifo::new(),
            arg_fifo: Fifo::new(),
            pending_seek: None,
            sector: Sector::start_value(),
            data_buffer: DataBuffer::new(),
            position,
            mode,
        }
    }

    pub fn store<T: AddrUnit>(&mut self, schedule: &mut Schedule, addr: u32, val: u32) {
        match addr {
            0 => self.index = val.bit_range(0, 1) as u8,
            1 => match self.index {
                0 => {
                    if self.cmd.is_some() {
                        warn!("CDROM beginning command while command is pending");
                    }
                    self.cmd = Some(val as u8);
                }
                _ => todo!(),
            }
            2 => match self.index {
                0 => self.arg_fifo.push(val as u8),
                1 => {
                    let was_active = self.irq_active();
                    self.irq_mask = val.bit_range(0, 4) as u8;

                    if !was_active && self.irq_active() {
                        schedule.schedule_now(Event::IrqTrigger(Irq::CdRom));
                    }
                }
                2 => todo!(),
                _ => unreachable!(),
            }
            3 => match self.index {
                0 => {
                    let was_active = self.data_buffer.active;
                    self.data_buffer.active = val.bit(7);

                    if self.data_buffer.active {
                        if !was_active {
                            self.data_buffer.fill_from_sector(&self.sector);
                        }
                    } else {
                        self.data_buffer.advance();
                    }
                }
                1 => {
                    self.irq_flags &= !(val.bit_range(0, 4) as u8);

                    if val.bit(6) {
                        self.arg_fifo.clear();
                    }
                }
                2 => todo!(),
                _ => unreachable!(),
            }
            _ => unreachable!(),
        }
    }

    pub fn load<T: AddrUnit>(&mut self, addr: u32) -> u32 {
        match addr {
            // Status register.
            0 => {
                let register = self.index
                    | (self.arg_fifo.is_empty() as u8) << 3
                    | (!self.arg_fifo.is_full() as u8) << 4
                    | (!self.response_fifo.is_empty() as u8) << 5
                    | (!self.data_buffer.data_ready() as u8) << 6
                    | (self.cmd.is_some() as u8) << 7;
                register.into()
            }
            1 => {
                self.response_fifo.try_pop().unwrap_or(0x0).into()
            }
            2 => {
                self.data_buffer.read_byte().into()
            }
            3 => match self.index {
                0 => self.irq_mask as u32 | !0x1f,
                1 => self.irq_flags as u32 | !0x1f,
                2 => todo!(),
                _ => unreachable!(),
            }
            _ => unreachable!(),
        }
    }

    fn finish_cmd(&mut self, schedule: &mut Schedule, irq: Interrupt) {
        self.response_fifo.push(self.drive_stat());
        self.set_interrupt(schedule, irq);
    }

    pub fn run(&mut self, schedule: &mut Schedule) {
        self.exec_cmd(schedule);
        schedule.schedule_in(2_000, Event::RunCdRom);
    }

    fn start_seek(&mut self, schedule: &mut Schedule, ty: SeekType, after: AfterSeek) -> Cycle {
        let target = match self.pending_seek.take() {
            Some(msf) => msf,
            None => {
                warn!("Seeking without setting a location");
                Msf::ZERO
            }
        };

        // TODO: Do some kind of seek time heuristic.
        let cycles = 225_000;

        schedule.schedule_in(cycles, Event::CdRomSectorDone);
        self.state = DriveState::Seeking(target, ty, after);

        cycles
    }

    fn start_read(&mut self, schedule: &mut Schedule) {
        let cycles = 225_000;

        schedule.schedule_in(cycles, Event::CdRomSectorDone);
        self.state = DriveState::Reading;
    }

    pub fn sector_done(&mut self, schedule: &mut Schedule) {
        let data_ready = match self.state {
            DriveState::Seeking(target, _, after) => {
                self.position = target;
                match after {
                    AfterSeek::Read => self.start_read(schedule),
                    AfterSeek::Pause => self.state = DriveState::Paused,
                    AfterSeek::Play => todo!(),
                };
                false
            }
            DriveState::Reading => {
                match *self.disc.cd_mut() {
                    None => unreachable!(),
                    Some(ref mut cd) => {
                        self.position = self.position
                            .next_sector()
                            .unwrap();

                        // TODO: Heuristics.
                        let cycles = 225_000;

                        schedule.schedule_in(cycles, Event::CdRomSectorDone);

                        // Maybe unshedule any other 'CdRomSectorDone' events, since it's possible
                        // it could have started reading, then paused, but started reading again
                        // before the first sector was read.

                        self.sector = cd.load_sector(self.position).unwrap();
                    }
                }
                true
            }
            _ => false,
        };

        if data_ready {
            self.finish_cmd(schedule, Interrupt::DataReady);
        }
    }

    pub fn reponse(&mut self, schedule: &mut Schedule, cmd: CdRomCmd) {
        match cmd.0 {
            // init
            0x0a => {
                self.finish_cmd(schedule, Interrupt::Complete);
            }
            // pause
            0x09 => {
                self.state = DriveState::Paused;
                self.finish_cmd(schedule, Interrupt::Complete);
            }
            // read_toc
            0x1e => {
                self.state = DriveState::Paused;
                self.finish_cmd(schedule, Interrupt::Complete);
            }
            // seek_l
            0x15 => {
                self.finish_cmd(schedule, Interrupt::Complete);
            }
            // get_id
            0x1a => {
                
                // TODO: Handle error where there is no disc here. It can't be reached at the
                // moment, but in the future if removing the disc during execution is supported,
                // this should return an error instead.

                // TODO: Change the response to represent the disc region.

                let response = [
                    self.drive_stat(),
                    0x0,
                    0x20,
                    0x00,
                    b'S',
                    b'C',
                    b'E',
                    b'A',
                ];

                self.state = DriveState::Idle;

                self.response_fifo.push_slice(&response);
                self.set_interrupt(schedule, Interrupt::Complete);
            }
            _ => todo!(),
        }
    }

    fn exec_cmd(&mut self, schedule: &mut Schedule) {
        if self.irq_flags != 0 {
            return;
        }

        if let Some(cmd) = self.cmd.take() {
            self.response_fifo.clear();

            match cmd {
                // status
                0x01 => {
                    self.finish_cmd(schedule, Interrupt::Ack);
                }
                // set_loc
                0x02 => {
                    // TODO: Handle invalid bcd's and invalid argument count.
                    self.state = DriveState::ReadingToc;

                    let m = Bcd::from_bcd(self.arg_fifo.pop()).unwrap(); 
                    let s = Bcd::from_bcd(self.arg_fifo.pop()).unwrap(); 
                    let f = Bcd::from_bcd(self.arg_fifo.pop()).unwrap();

                    self.pending_seek = Some(Msf::from_bcd(m, s, f));
                    self.finish_cmd(schedule, Interrupt::Ack); 
                }
                // read_n
                0x06 => {
                    self.finish_cmd(schedule, Interrupt::Ack);

                    // Ignore the command if we are seeking and should read after the seek unless
                    // there there is a new seek target set by set_loc that doesn't match the current
                    // seek location.
                    if let DriveState::Seeking(target, _, AfterSeek::Read) = self.state {
                        if self.pending_seek.is_none() || self.pending_seek.contains(&target) {
                            return;
                        }
                    }

                    // Ignore command if we are reading or paused and there isn't a new seek target.
                    if let DriveState::Reading | DriveState::Paused = self.state {
                        if self.pending_seek.is_none() {
                            self.start_read(schedule);
                            return;
                        }
                    }
                    
                    // At this point we should start seeking. If we are already seeking then we
                    // should read after seeking.
                    match self.pending_seek {
                        Some(_) => {
                            self.start_seek(schedule, SeekType::Data, AfterSeek::Read);
                        }
                        None => {
                            if let DriveState::Seeking(_, _, ref mut after) = self.state {
                                *after = AfterSeek::Read;
                            } else {
                                self.start_read(schedule);
                            }
                        }
                    }
                }
                // pause
                0x09 => {
                    self.finish_cmd(schedule, Interrupt::Ack);

                    let cycles = match self.state {
                        DriveState::Paused | DriveState::Idle => 9_000,
                        _ => 1_000_000,
                    };

                    self.state = DriveState::Paused;
                    schedule.schedule_in(cycles, Event::CdRomResponse(CdRomCmd(0x9)));
                }
                // init
                0x0a => {
                    self.finish_cmd(schedule, Interrupt::Ack);

                    // Should this be idle?
                    self.state = DriveState::Paused;

                    self.position = Msf::ZERO;
                    self.pending_seek = None;

                    schedule.schedule_in(900_000, Event::CdRomResponse(CdRomCmd(0x0a)));
                }
                // set_mode: Sets the value of the mode register.
                0x0e => {
                    self.mode = ModeReg(self.arg_fifo.pop());
                    self.finish_cmd(schedule, Interrupt::Ack); 
                }
                // seekl: Seek location set by (02) setloc in data mode. 
                0x15 => {
                    let cycles = self.start_seek(schedule, SeekType::Data, AfterSeek::Pause);

                    self.finish_cmd(schedule, Interrupt::Ack);
                    schedule.schedule_in(cycles, Event::CdRomResponse(CdRomCmd(0x15)));
                }
                // test: It's behavior depent on the first argument.
                0x19 => match self.arg_fifo.pop() {
                    0x20 => {
                        // These represent year, month, day and version respectively.
                        self.response_fifo.push_slice(&[0x98, 0x06, 0x10, 0xc3]);
                        self.set_interrupt(schedule, Interrupt::Ack);
                    }
                    _ => todo!(),
                }
                // get_id
                0x1a => {
                    if !self.disc.is_loaded() {
                        self.response_fifo.push_slice(&[0x11, 0x80]);
                        self.set_interrupt(schedule, Interrupt::Error);
                    } else {
                        self.finish_cmd(schedule, Interrupt::Ack);
                        schedule.schedule_in(33868, Event::CdRomResponse(CdRomCmd(0x1a)));
                    }

                }
                // read_toc
                0x1e => {
                    self.state = DriveState::Reading;
                    self.finish_cmd(schedule, Interrupt::Ack);
                    schedule.schedule_in(30_000_000, Event::CdRomResponse(CdRomCmd(0x1e)));
                }
                _ => todo!("CDROM Command: {:08x}", cmd),
            }

            self.arg_fifo.clear();
        }
    }

    fn set_interrupt(&mut self, schedule: &mut Schedule, int: Interrupt) {
        self.irq_flags = int as u8;

        if self.irq_active() {
            schedule.schedule_now(Event::IrqTrigger(Irq::CdRom));
        }
    }

    fn irq_active(&self) -> bool {
        (self.irq_flags & self.irq_mask) != 0
    }

    fn drive_stat(&self) -> u8 {
        if !self.disc.is_loaded() {
            // This means that the drive cover is open.
            0x10
        } else {
            match self.state {
                DriveState::Idle => 0,
                DriveState::Paused | DriveState::ReadingToc => (1 << 1),
                DriveState::Seeking { .. } => (1 << 1) | (1 << 6),
                DriveState::Reading => (1 << 1) | (1 << 5),
            }
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct CdRomCmd(u8);

impl fmt::Display for CdRomCmd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            0x01 => write!(f, "status"),
            0x02 => write!(f, "set_loc"),
            0x06 => write!(f, "read_n"),
            0x09 => write!(f, "pause"),
            0x0a => write!(f, "init"),
            0x0e => write!(f, "set_mode"),
            0x15 => write!(f, "seek_l"),
            0x19 => write!(f, "test"),
            0x1a => write!(f, "get_id"),
            0x1e => write!(f, "readtoc"),
            cmd => todo!("CDROM Command {:08x}", cmd),
        }
    }
}
/// Interrupt types used internally by the CDROM.
#[derive(Clone, Copy)]
enum Interrupt {
    DataReady = 0x1,
    Complete = 0x2,
    Ack = 0x3,
    Error = 0x5,
}

#[derive(Clone, Copy)]
enum SeekType {
    Data,
    #[allow(dead_code)]
    Audio,
}

#[derive(Clone, Copy)]
enum AfterSeek {
    Pause,
    Read,
    #[allow(dead_code)]
    Play,
}

#[derive(Clone, Copy)]
enum DriveState {
    Idle,
    Seeking(Msf, SeekType, AfterSeek),
    Paused,
    Reading,
    ReadingToc,
}

#[derive(Clone, Copy)]
struct ModeReg(u8);

impl ModeReg {
    #[allow(dead_code)]
    fn cdda_mode(self) -> bool {
        self.0.bit(0)
    }

    #[allow(dead_code)]
    fn auto_pause(self) -> bool {
        self.0.bit(1)
    }

    #[allow(dead_code)]
    fn report_interrupt(self) -> bool {
        self.0.bit(2)
    }

    #[allow(dead_code)]
    fn filter_enabled(self) -> bool {
        self.0.bit(3)
    }

    #[allow(dead_code)]
    fn sector_size_override(self) -> bool {
        self.0.bit(4)
    }

    #[allow(dead_code)]
    fn read_whole_sector(self) -> bool {
        self.0.bit(5)
    }

    #[allow(dead_code)]
    fn xa_adpcm_to_spu(self) -> bool {
        self.0.bit(6)
    }

    #[allow(dead_code)]
    fn double_speed(self) -> bool {
        self.0.bit(7)
    }
}

struct DataBuffer {
    data: Box<[u8; 2352]>,
    len: u16,
    index: u16,
    active: bool,
}

impl DataBuffer {
    fn new() -> Self {
        Self {
            data: Box::new([0x0; 2352]),
            len: 0,
            index: 0,
            active: false,
        }
    }

    fn fill_from_sector(&mut self, sector: &Sector) {
        let data = sector.data(); 

        for (i, byte) in data.iter().enumerate() {
            self.data[i] = *byte;
        }

        self.len = data.len() as u16;
        self.index = 0;
    }

    fn data_ready(&self) -> bool {
        // Maybe it should also be active.
        self.index < self.len
    }

    fn advance(&mut self) {
        let idx = self.index;
        let adj = (idx & 4) << 1;
        self.index = (idx & !7) + adj;
    }

    fn read_byte(&mut self) -> u8 {
        let byte = self.data[self.index as usize];
        
        if self.active {
            self.index += 1;
            if self.index == self.len {
                self.active = false;
            }
        } else {
            warn!("CDROM Read inactive data buffer");
        }

        byte
    }
}


impl DmaChan for CdRom {
    fn dma_load(&mut self, _: &mut Schedule, _: (u16, u32)) -> u32 {
        let v1 = self.data_buffer.read_byte() as u32;
        let v2 = self.data_buffer.read_byte() as u32;
        let v3 = self.data_buffer.read_byte() as u32;
        let v4 = self.data_buffer.read_byte() as u32;

        v1 | (v2 << 8) | (v3 << 8) | (v4 << 8)
    }

    fn dma_store(&mut self, _: &mut Schedule, _: u32) {
        unreachable!("DMA store to CDROM");
    }

    fn dma_ready(&self, _: ChanDir) -> bool {
        // This is probably wrong.
        true
    }
}

impl BusMap for CdRom {
    const BUS_BEGIN: u32 = 0x1f801800;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}
