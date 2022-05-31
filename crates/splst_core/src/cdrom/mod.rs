//! # TODO
//!
//! - The CDROM runs at fixed intervals which doesn't seem neccessary but running after every store
//!   doesn't seem to work, likely because the CDROM executes the command immediately, which the
//!   BIOS isn't expecting.

mod xa_buffer;

use splst_cdimg::{CdImage, Sector};
use splst_util::Bit;
use splst_util::{Bcd, Msf};
use crate::bus::{dma, BusMap, AddrUnit};
use crate::cpu::Irq;
use crate::schedule::{Event, Schedule};
use crate::{dump, dump::Dumper, SysTime};
use crate::fifo::Fifo;

use xa_buffer::XaBuffer;

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::time::Duration;

pub struct CdRom {
    pub(super) disc: Rc<RefCell<Disc>>,
    state: DriveState,
    /// The index register. This decides what happens when the CPU writes to and
    /// loads from the CDROM.
    index: u8,
    /// Which [`Interrupt`]s are enabled.
    irq_mask: u8,
    /// Which [`Interrupt`]s are active.
    irq_flags: u8,
    /// The CDROM may or may not have a command waiting to be executed.
    cmd: Option<u8>,
    /// Responses from commands.
    response_fifo: Fifo<u8, 16>,
    /// Arguments to commands.
    arg_fifo: Fifo<u8, 16>,
    position: Msf,
    pending_seek: Option<Msf>,
    mode: ModeReg,
    sector: Sector,
    data_buffer: DataBuffer,

    audio_buffer: Vec<(i16, i16)>,
    audio_freq: AudioFreq,
    audio_index: u32,
    audio_phase: u8,

    volume_matrix: VolMatrix,
    next_volume_matrix: VolMatrix,

    xa_buffer: XaBuffer,
}

impl CdRom {
    pub fn new(schedule: &mut Schedule, disc: Rc<RefCell<Disc>>) -> Self {
        schedule.schedule_repeat(SysTime::new(7_000), Event::CdRom(CdRom::run));

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
            audio_buffer: Vec::with_capacity(4096),
            audio_freq: AudioFreq::Da1x,
            audio_index: 0,
            audio_phase: 0,

            volume_matrix: VolMatrix::default(),
            next_volume_matrix: VolMatrix::default(),

            xa_buffer: XaBuffer::default(),
        }
    }

    pub fn store<T: AddrUnit>(&mut self, schedule: &mut Schedule, addr: u32, val: T) {
        if !T::WIDTH.is_byte() {
            warn!("{} store to CD-ROM", T::WIDTH);
        }

        let val = val.as_u8();

        match addr {
            0 => self.index = val.bit_range(0, 1) as u8,
            1 => match self.index {
                0 => {
                    if self.cmd.is_some() {
                        warn!("cd-rom beginning command while command is pending");
                    }
                    self.cmd = Some(val as u8);
                }
                _ => todo!(),
            },
            2 => match self.index {
                0 => self.arg_fifo.push(val as u8),
                1 => {
                    let was_active = self.irq_active();
                    self.irq_mask = val.bit_range(0, 4) as u8;

                    if !was_active && self.irq_active() {
                        schedule.trigger(Event::Irq(Irq::CdRom));
                    }
                }
                2 => todo!(),
                _ => unreachable!(),
            },
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
            },
            _ => unreachable!(),
        }
    }

    pub fn load<T: AddrUnit>(&mut self, addr: u32) -> T {
        let val = match addr {
            0 => self.status_reg(),
            1 => self.response_fifo.try_pop().unwrap_or(0x0).into(),
            2 => self.data_buffer.read_byte().into(),
            3 => match self.index {
                0 => self.irq_mask as u32 | !0x1f,
                1 => self.irq_flags as u32 | !0x1f,
                2 => todo!(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        T::from_u32(val)
    }

    /// `load` without side effects.
    pub fn peek<T: AddrUnit>(&self, addr: u32) -> T {
        let val = match addr {
            0 => self.status_reg(),
            1 => self.response_fifo.peek().unwrap_or(0x0).into(),
            2 => self.data_buffer.peek_byte().into(),
            3 => match self.index {
                0 => self.irq_mask as u32 | !0x1f,
                1 => self.irq_flags as u32 | !0x1f,
                2 => todo!(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        T::from_u32(val)
    }

    fn status_reg(&self) -> u32 {
        let register = self.index
            | (self.arg_fifo.is_empty() as u8) << 3
            | (!self.arg_fifo.is_full() as u8) << 4
            | (!self.response_fifo.is_empty() as u8) << 5
            | (!self.data_buffer.data_ready() as u8) << 6
            | (self.cmd.is_some() as u8) << 7;
        register.into()
    }

    fn finish_cmd(&mut self, schedule: &mut Schedule, irq: Interrupt) {
        self.response_fifo.push(self.drive_stat());
        self.set_interrupt(schedule, irq);
    }

    pub fn run_audio_cycle(&mut self, resample: bool) -> (i16, i16) {
        let idx = self.audio_index as usize;

        let Some((raw_left, raw_right)) = self.audio_buffer.get(idx) else {
            return (0, 0);
        };

        let mut left = *raw_left;
        let mut right = *raw_right;
        
        match self.audio_freq {
            AudioFreq::Da1x => self.audio_index += 1,
            AudioFreq::Da2x => self.audio_index += 2,
            AudioFreq::Xa37k8 | AudioFreq::Xa18k9 => {
                if resample {
                    let (ls, rs) = self.xa_buffer.resample(self.audio_phase);
                    left += ls;
                    right += rs;
                }

                let step = self.audio_freq as u8;
                self.audio_phase += step;

                if let Some(phase) = self.audio_phase.checked_sub(7) {
                    self.audio_phase = phase;
                    self.xa_buffer.push((*raw_left, *raw_right));
                }

                self.audio_index += 1;
            }
        }

        self.volume_matrix.apply(left.into(), right.into())
    }

    pub fn run(&mut self, schedule: &mut Schedule) {
        self.exec_cmd(schedule);
    }

    fn start_seek(&mut self, schedule: &mut Schedule, ty: SeekType, after: AfterSeek) -> SysTime {
        let target = match self.pending_seek.take() {
            Some(msf) => msf,
            None => {
                warn!("seeking without setting a location");
                Msf::ZERO
            }
        };

        // TODO: Do some kind of seek time heuristic.
        let time = SysTime::new(225_000);

        schedule.schedule(time, Event::CdRom(Self::sector_done));
        self.state = DriveState::Seeking(target, ty, after);

        time
    }

    fn start_read(&mut self, schedule: &mut Schedule) {
        let time = SysTime::new(225_000);

        schedule.schedule(time, Event::CdRom(Self::sector_done));
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
                match self.disc.borrow_mut().cd() {
                    None => unreachable!(),
                    Some(cd) => {
                        self.position = self.position.next_sector().unwrap();

                        // TODO: Heuristics.
                        let time = SysTime::new(225_000);

                        schedule.schedule(time, Event::CdRom(Self::sector_done));

                        // Maybe unshedule any other 'sector_done' events, since it's possible
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
                // stop
                0x08 => {
                    self.finish_cmd(schedule, Interrupt::Ack);

                    let time = if self.motor_on() {
                        SysTime::new(1_000_000)
                    } else {
                        SysTime::new(7_000)
                    };

                    schedule.schedule(time, Event::CdRom(Self::async_stop));
                }
                // pause
                0x09 => {
                    self.finish_cmd(schedule, Interrupt::Ack);

                    let time = match self.state {
                        DriveState::Paused | DriveState::Idle => SysTime::new(9_000),
                        _ => SysTime::new(1_000_000),
                    };

                    self.state = DriveState::Paused;
                    schedule.schedule(time, Event::CdRom(Self::async_pause));
                }
                // init
                0x0a => {
                    self.finish_cmd(schedule, Interrupt::Ack);

                    // Should this be idle?
                    self.state = DriveState::Paused;

                    self.position = Msf::ZERO;
                    self.pending_seek = None;

                    schedule.schedule(SysTime::new(900_000), Event::CdRom(Self::async_init));
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
                    schedule.schedule(cycles, Event::CdRom(Self::async_seekl));
                }
                // test: It's behavior depent on the first argument.
                0x19 => match self.arg_fifo.pop() {
                    0x20 => {
                        // These represent year, month, day and version respectively.
                        self.response_fifo.push_slice(&[0x98, 0x06, 0x10, 0xc3]);
                        self.set_interrupt(schedule, Interrupt::Ack);
                    }
                    _ => todo!(),
                },
                // get_id
                0x1a => {
                    if !self.disc.borrow().is_loaded() {
                        self.response_fifo.push_slice(&[0x11, 0x80]);
                        self.set_interrupt(schedule, Interrupt::Error);
                    } else {
                        self.finish_cmd(schedule, Interrupt::Ack);
                        schedule.schedule(SysTime::new(33868), Event::CdRom(Self::async_get_id));
                    }
                }
                // read_toc
                0x1e => {
                    self.state = DriveState::Reading;
                    self.finish_cmd(schedule, Interrupt::Ack);

                    // Reading the table of content takes about 1 second.
                    let time = SysTime::from_duration(Duration::from_secs(1));

                    schedule.schedule(time, Event::CdRom(Self::async_read_toc));
                }
                _ => todo!("CDROM Command: {:08x}", cmd),
            }

            self.arg_fifo.clear();
        }
    }

    fn set_interrupt(&mut self, schedule: &mut Schedule, int: Interrupt) {
        self.irq_flags = int as u8;

        if self.irq_active() {
            schedule.trigger(Event::Irq(Irq::CdRom));
        }
    }

    fn irq_active(&self) -> bool {
        (self.irq_flags & self.irq_mask) != 0
    }

    fn motor_on(&self) -> bool {
        !matches!(self.state, DriveState::Idle)
    }

    fn drive_stat(&self) -> u8 {
        if !self.disc.borrow().is_loaded() {
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

    pub fn dump(&self, d: &mut impl Dumper) {
        dump!(d, "drive state", "{}", self.state);
        dump!(d, "position", "{}", self.position);

        let pending = self.pending_seek
            .map(|msf| msf.to_string())
            .unwrap_or_else(|| "none".to_string());

        dump!(d, "pending seek", "{pending}");

        // TODO: Mode register.
       
        dump!(d, "sector msf", "{}", self.sector.abs_msf);
        dump!(d, "sector track msf", "{}", &self.sector.track_msf);

        dump!(d, "sector format", "{}", self.sector.format);
        dump!(d, "sector track", "{}", self.sector.track);
        dump!(d, "sector index", "{}", self.sector.index);

        dump!(d, "audio frequency", "{}", self.audio_freq);
        dump!(d, "audio index", "{}", self.audio_index);
        dump!(d, "audio phase", "{}", self.audio_phase);
    }
}

/// Implementation of asynchronous responses for commands.
impl CdRom {
    fn async_init(&mut self, schedule: &mut Schedule) {
        self.finish_cmd(schedule, Interrupt::Complete);
    }

    fn async_pause(&mut self, schedule: &mut Schedule) {
        self.state = DriveState::Paused;
        self.finish_cmd(schedule, Interrupt::Complete);
    }

    fn async_stop(&mut self, schedule: &mut Schedule) {
        self.state = DriveState::Idle;
        self.finish_cmd(schedule, Interrupt::Complete);
    }

    fn async_read_toc(&mut self, schedule: &mut Schedule) {
        self.state = DriveState::Paused;
        self.finish_cmd(schedule, Interrupt::Complete);
    }

    fn async_seekl(&mut self, schedule: &mut Schedule) {
        self.finish_cmd(schedule, Interrupt::Complete);
    }

    fn async_get_id(&mut self, schedule: &mut Schedule) {
        // TODO: Handle error where there is no disc here. It can't be reached at the
        // moment, but in the future if removing the disc during execution is supported,
        // this should return an error instead.

        // TODO: Change the response to represent the disc region.

        let response = [self.drive_stat(), 0x0, 0x20, 0x00, b'S', b'C', b'E', b'A'];

        self.state = DriveState::Idle;

        self.response_fifo.push_slice(&response);
        self.set_interrupt(schedule, Interrupt::Complete);
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct CdRomCmd(u8);

impl fmt::Display for CdRomCmd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            0x01 => f.write_str("status"),
            0x02 => f.write_str("set_loc"),
            0x06 => f.write_str("read_n"),
            0x09 => f.write_str("pause"),
            0x0a => f.write_str("init"),
            0x0e => f.write_str("set_mode"),
            0x15 => f.write_str("seek_l"),
            0x19 => f.write_str("test"),
            0x1a => f.write_str("get_id"),
            0x1e => f.write_str("readtoc"),
            cmd => todo!("cdrom command {:08x}", cmd),
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
    // TODO:
    #[allow(dead_code)]
    Audio,
}

impl fmt::Display for SeekType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SeekType::Data => f.write_str("data"),
            SeekType::Audio => f.write_str("audio"),
        }
    }
}

#[derive(Clone, Copy)]
enum AfterSeek {
    Pause,
    Read,
    // TODO:
    #[allow(dead_code)]
    Play,
}

impl fmt::Display for AfterSeek {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AfterSeek::Pause => f.write_str("pause"),
            AfterSeek::Read => f.write_str("read"),
            AfterSeek::Play => f.write_str("play"),
        }
    }
}

#[derive(Clone, Copy)]
enum AudioFreq {
    Da1x = 7,
    Da2x = 14,
    Xa18k9 = 3,
    Xa37k8 = 6,
}

impl fmt::Display for AudioFreq {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AudioFreq::Da1x => f.write_str("da1x"),
            AudioFreq::Da2x => f.write_str("da2x"),
            AudioFreq::Xa18k9 => f.write_str("xa18k9"),
            AudioFreq::Xa37k8 => f.write_str("xa37k8"),
        }
    }
}

/// Represents the state of the CDROM drive.
#[derive(Clone, Copy)]
enum DriveState {
    /// The drive is idle meaning that the CD isn't spinning.
    Idle,
    /// The drive is seeking a sector on the CD.
    Seeking(Msf, SeekType, AfterSeek),
    /// The motor is running but no data is being read.
    Paused,
    /// The drive is reading the data of a sector.
    Reading,
    /// The drive is reading the table of content.
    ReadingToc,
}

impl fmt::Display for DriveState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DriveState::Idle => f.write_str("idle"),
            DriveState::Paused => f.write_str("paused"),
            DriveState::Reading => f.write_str("reading"),
            DriveState::ReadingToc => f.write_str("reading table of content"),
            DriveState::Seeking(target, kind, after) => {
                write!(f, "seeking {kind} to {target}, {after} after")
            }
        }
    }
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

#[derive(Default)]
pub struct Disc(Option<CdImage>);

impl Disc {
    pub fn cd(&self) -> Option<&CdImage> {
        self.0.as_ref()
    }

    pub fn is_loaded(&self) -> bool {
        self.0.is_some()
    }

    pub fn unload(&mut self) {
        self.0.take();
    }

    pub fn load(&mut self, cd: CdImage) {
        self.0 = Some(cd);
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

    fn peek_byte(&self) -> u8 {
        self.data[self.index as usize]
    }

    fn read_byte(&mut self) -> u8 {
        let byte = self.peek_byte();

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

#[derive(Default)]
struct VolMatrix([[u8; 2]; 2]);

impl VolMatrix {
    fn apply(&self, left: i32, right: i32) -> (i16, i16) {
        let mul = |vec: &[u8; 2]| -> i16 {
            let val = vec[0] as i32 * left + vec[1] as i32 * right;
            (val >> 7).clamp(i16::MIN.into(), u16::MAX.into()) as i16
        };
        (mul(&self.0[0]), mul(&self.0[1]))
    }
}

impl dma::Channel for CdRom {
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

    fn dma_ready(&self, _: dma::Direction) -> bool {
        // This is probably wrong.
        true
    }
}

impl BusMap for CdRom {
    const BUS_BEGIN: u32 = 0x1f801800;
    const BUS_END: u32 = Self::BUS_BEGIN + 4 - 1;
}
