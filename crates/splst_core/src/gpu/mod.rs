//! Emulation of the Playstations 1 GPU.
//!
//! TODO:
//! * Maybe switch to integer fixed point for rasterization instead of floats.
//!
//! * DMA chopping wouldn't work correctly with the GPU, since the GPU updates it's DMA channel
//!   after each GP0 write, which isn't handeled by the DMA.

mod fifo;
mod primitive;
mod rasterize;
mod gp0;
mod gp1;

pub mod vram;

use splst_util::{Bit, BitSet};
use crate::cpu::Irq;
use crate::bus::{DmaChan, ChanDir, BusMap, AddrUnit};
use crate::bus::dma::Port;
use crate::schedule::{Event, Schedule};
use crate::timing;
use crate::timer::Timers;
use crate::{Cycle, DrawInfo};

use fifo::Fifo;
use primitive::Color;
use gp0::draw_mode;

use std::fmt;

pub use vram::Vram;

pub struct Gpu {
    /// The current state of the GPU.
    state: State,
    /// The GPU FIFO. Used to recieve commands and some kinds of data.
    fifo: Fifo,
    /// The Video Memory used to store texture data and the image buffer(s).
    vram: Vram,
    /// State used to emulate the timing of the GPU such as if it's in HBlank or VBlank. Also
    /// handles the timing differences between PAL and NTSC video modes.
    timing: Timing,
    /// The status register.
    pub status: Status,
    /// The GPUREAD register. This contains various info about the GPU, and is generated after each
    /// GP1(10) command call. It is read through the BUS at GPUREAD, if no transfer is ongoing.
    gpu_read: u32,
    /// Flips the texture of rectangles on the x-axis.
    tex_x_flip: bool,
    /// Flips the texture of rectangles on the y-axis.
    tex_y_flip: bool,
    /// The width of the texture window in VRAM.
    tex_win_w: u8,
    /// The height of the texture window in VRAM.
    tex_win_h: u8,
    /// The x-coordinate of the start of the texture window in VRAM.
    tex_win_x: u8,
    /// The y-coordinate of the start of the texture window in VRAM.
    tex_win_y: u8,
    /// The right edge of the draw area in VRAM.
    da_x_max: i32,
    /// The left edge of the draw area in VRAM.
    da_x_min: i32,
    /// The top edge of the draw area in VRAM.
    da_y_max: i32,
    /// The bottom edge of the draw area in VRAM.
    da_y_min: i32,
    /// Draw offset x. This is added to the x-coordinate of vertex and point drawn.
    pub x_offset: i16,
    /// Draw offset y. This is added to the y-coordinate of vertex and point drawn.
    pub y_offset: i16,
    /// The first column to be displayed on the screen.
    pub vram_x_start: u16,
    /// The first line to be displayed on the screen.
    pub vram_y_start: u16,
    /// Which column the the display area starts on the screen.
    pub dis_x_start: u16,
    /// Which column the the display area ends on the screen.
    pub dis_x_end: u16,
    /// Which row the the display area starts on the screen.
    pub dis_y_start: u16,
    /// Which row the the display area ends on the screen.
    pub dis_y_end: u16,
}

impl Gpu {
    pub fn new() -> Self {
        // Sets the reset values.
        let status = Status(0x14802000);

        Self {
            state: State::Idle,
            fifo: Fifo::new(),
            vram: Vram::new(),
            timing: Timing::new(status.video_mode()),
            status,
            gpu_read: 0x0,
            tex_x_flip: false,
            tex_y_flip: false,
            tex_win_w: 0x0,
            tex_win_h: 0x0,
            tex_win_x: 0x0,
            tex_win_y: 0x0,
            da_x_max: 0x0,
            da_x_min: 0x0,
            da_y_max: 0x0,
            da_y_min: 0x0,
            x_offset: 0x0,
            y_offset: 0x0,
            vram_x_start: 0x0,
            vram_y_start: 0x0,
            dis_x_start: 0x200,
            dis_x_end: 0xc00,
            dis_y_start: 0x10,
            dis_y_end: 0x100,
        }
    }

    pub fn store<T: AddrUnit>(
        &mut self,
        schedule: &mut Schedule,
        addr: u32,
        val: u32
    ) {
        debug_assert_eq!(T::WIDTH, 4);
        match addr {
            0 => self.gp0_store(schedule, val),
            4 => self.gp1_store(val),
            _ => unreachable!("Invalid GPU store at offset {:08x}.", addr),
        }

        // 'dma_ready' can have changed here, which means that the GPU DMA should be updated.
        // Maybe only check if something has changed which could change 'dma_ready', but that
        // could be skechy. Running a DMA channel is pretty fast anyway, but maybe just running
        // the GPU channel every 1500 cycles could be faster?
        schedule.unschedule(Event::RunDmaChan(Port::Gpu));
        schedule.schedule_now(Event::RunDmaChan(Port::Gpu));
    }

    pub fn load<T: AddrUnit>(
        &mut self,
        addr: u32,
        schedule: &mut Schedule,
        timers: &mut Timers
    ) -> u32 {
        debug_assert_eq!(T::WIDTH, 4);

        // Run mainly to update the status register. Doesn't schedule a new run, so it will still
        // just run at the regular interval.
        self.run_internal(schedule, timers);

        match addr {
            0 => self.gpu_read(),
            4 => self.status_read(),
            _ => unreachable!("Invalid GPU load at offset {:08x}.", addr),
        }
    }

    /// The GPU read register. Either loads data from the VRAM or results from the GPU 
    fn gpu_read(&mut self) -> u32 {
        if let State::VramLoad(ref mut tran) = self.state {
            self.gpu_read = [0, 16].iter().fold(0, |state, shift| {
                let val = self.vram.load_16(tran.x, tran.y) as u32;
                tran.next();
                state | (val << shift)
            });
            if tran.is_done() {
                self.state = State::Idle;
            }
        }
        self.gpu_read
    }

    /// Calculate if the GPU is ready to recieve data from the DMA.
    pub fn dma_block_ready(&self) -> bool {
        match self.state {
            State::VramStore(..) => !self.fifo.is_full(),
            State::Drawing | State::VramLoad(..) => false,
            State::Idle => {
                if let Some(cmd) = self.fifo.next_cmd() {
                    // If the command is a line or polygon command, the dma ready flag get's
                    // clearead after the first word ie. the command itself rather than when the
                    // GPU is busy executing the command.
                    if (0x20..=0x5a).contains(&cmd) {
                        false
                    } else {
                        !self.fifo.has_full_cmd()
                    }
                } else {
                    true
                }
            }
        }
    }

    fn status_read(&mut self) -> u32 {
        self.status.0 = self.status.0
            .set_bit(27, self.state.is_vram_load())
            .set_bit(28, self.dma_block_ready())
            .set_bit(26, self.state.is_idle() && self.fifo.is_empty())
            .set_bit(25, match self.status.dma_direction() {
                DmaDir::Off => false,
                DmaDir::Fifo => !self.fifo.is_full(),
                DmaDir::CpuToGp0 => self.status.dma_block_ready(),
                DmaDir::VramToCpu => self.status.vram_to_cpu_ready(),
            });

        trace!("GPU status load");

        self.status.0
    }

    fn gp0_store(&mut self, schedule: &mut Schedule, val: u32) {
        if self.fifo.try_push(val) {
            self.try_run_cmd(schedule);
        }
    }

    fn try_run_cmd(&mut self, schedule: &mut Schedule) {
        match self.state {
            State::Idle => {
                while self.fifo.has_full_cmd() {
                    self.gp0_exec(schedule);
                }
            }
            State::VramStore(ref mut tran) => {
                while !self.fifo.is_empty() {
                    let val = self.fifo.pop();

                    for (lo, hi) in [(0, 15), (16, 31)] {
                        let val = val.bit_range(lo, hi) as u16;
                        self.vram.store_16(tran.x, tran.y, val);
                        tran.next();
                    }

                    if tran.is_done() {
                        self.state = State::Idle;
                        break;
                    }
                }
            }
            State::Drawing | State::VramLoad(..) => (),
        }
    }

    pub fn vram(&self) -> &Vram {
        &self.vram
    }

    pub fn draw_info(&self) -> DrawInfo {
        DrawInfo {
            vram_x_start: self.vram_x_start as u32,
            vram_y_start: self.vram_y_start as u32,
        }
    }

    pub fn in_vblank(&self) -> bool {
        self.timing.in_vblank
    }

    pub fn frame_count(&self) -> u64 {
        self.timing.frame_count
    }

    pub fn cmd_done(&mut self) {
        self.state = State::Idle;
    }

    /// Run and schedule next run.
    pub fn run(&mut self, schedule: &mut Schedule, timers: &mut Timers) {
        self.run_internal(schedule, timers);
        schedule.schedule_in(5_000, Event::RunGpu);
    }

    /// Emulate the period since the last time the GPU run. It 
    pub fn run_internal(&mut self, schedule: &mut Schedule, timers: &mut Timers) {
        self.try_run_cmd(schedule);

        let elapsed = schedule.cycle() - self.timing.last_update;

        self.timing.scln_prog += timing::cpu_to_gpu_cycles(elapsed);
        self.timing.last_update = schedule.cycle();

        // If the progress is less than a single scanline. This is just to have a fast path to
        // allow running the GPU often without a big performance loss.
        if self.timing.scln_prog < self.timing.cycles_per_scln {
            let in_hblank = self.timing.scln_prog >= timing::HSYNC_CYCLES;

            // If we have entered Hblank.
            if in_hblank && !self.timing.in_hblank {
                timers.hblank(schedule, 1);
            }

            self.timing.in_hblank = in_hblank;

            return;
        }

        // Calculate the number of lines to be drawn.
        let mut lines = self.timing.scln_prog / self.timing.cycles_per_scln;
        self.timing.scln_prog %= self.timing.cycles_per_scln;

        // At there must have been atleast a single Hblank, this calculates the amount.
        // If the GPU wasn't in Hblank, it must have entered since then, which adds one to the
        // count. We know it's going to enter into Hblank on each scanline, except the current
        // one it's on, which is represented by 'in_hblank'.
        let in_hblank = self.timing.scln_prog >= timing::HSYNC_CYCLES;
        let hblank_count = u64::from(!self.timing.in_hblank)
            + u64::from(in_hblank)
            + lines - 1;

        timers.hblank(schedule, hblank_count);
        self.timing.in_hblank = in_hblank;

        while lines > 0 {
            // Can't move past the last line.
            let max_lines = u64::from(self.timing.scln_count - self.timing.scln);

            let line_count = lines.min(max_lines);
            lines -= line_count;

            let line_count = line_count as u16;
            let scln = self.timing.scln + line_count;

            // Calculate if the scanlines being drawn enters the display area, and clear the
            // Vblank flag if not.
            if self.timing.scln < self.timing.vbegin && scln >= self.timing.vend {
                self.timing.in_vblank = false;

                // TODO: Timer sync.
            }

            self.timing.scln = scln;

            let in_vblank = !timing::NTSC_VERTICAL_RANGE.contains(&(scln.into()));

            // If we are either leaving or entering Vblank.
            if self.timing.in_vblank != in_vblank {
                if in_vblank {
                    self.timing.frame_count += 1;
                    schedule.schedule_now(Event::IrqTrigger(Irq::VBlank));
                }

                self.timing.in_vblank = in_vblank;

                // TODO: Timer sync.
            }

            if self.timing.scln >= self.timing.scln_count {
                self.timing.scln = 0;

                // Bit 13 of the status register toggle just before each new frame in interlaced
                // 480 bit mode
                match self.status.interlaced_480() {
                    true => self.status.0 ^= 1 << 13,
                    false => self.status.0 &= !(1 << 13),
                }
            }
        }

        // The the current line being displayed in VRAM. It's used here to determine
        // the value of bit 31 of status.
        let line_offset = self.timing.display_line(
            self.status.interlaced_480(),
            self.status.interlace_field(),
        );

        let even = (self.vram_y_start + line_offset).bit(0);

        self.status.0 = self.status.0.set_bit(31, even);
    }

    fn gp1_store(&mut self, val: u32) {
        let cmd = val.bit_range(24, 31);
        match cmd {
            0x0 => self.gp1_reset(),
            0x1 => self.gp1_reset_fifo(),
            0x2 => self.gp1_ack_gpu_irq(),
            0x3 => self.gp1_display_enable(val),
            0x4 => self.gp1_dma_direction(val),
            0x5 => self.gp1_display_start(val),
            0x6 => self.gp1_horizontal_display_range(val),
            0x7 => self.gp1_vertical_display_range(val),
            0x8 => self.gp1_display_mode(val),
            0xff => {
                warn!("Weird GP1 command: GP1(ff)");
            }
            _ => {
                unimplemented!("Invalid GP1 command {:08x}.", cmd)
            }
        }
    }

    fn gp0_exec(&mut self, schedule: &mut Schedule) {
        let cmd = self.fifo[0].bit_range(24, 31);

        let cycles = match cmd {
            0x0 | 0x3 | 0x6 | 0x8 | 0x9 | 0xff => {
                self.gp0_nop();
                None
            }
            0x1 => {
                self.gp0_clear_texture_cache();
                None
            }
            0x2 => {
                self.gp0_fill_rect();
                None
            }
            0xe1 => {
                self.gp0_draw_mode();
                None
            }
            0xe2 => {
                self.gp0_texture_window_settings();
                None
            }
            0xe3 => {
                self.gp0_draw_area_top_left();
                None
            }
            0xe4 => {
                self.gp0_draw_area_bottom_right();
                None
            }
            0xe5 => {
                self.gp0_draw_offset();
                None
            }
            0xe6 => {
                self.gp0_mask_bit_setting();
                None
            }
            0x27 => Some(self.gp0_tri_poly::<
                draw_mode::UnShaded,
                draw_mode::TexturedRaw,
                draw_mode::Transparent,
            >()),
            0x28 => Some(self.gp0_quad_poly::<
                draw_mode::UnShaded,
                draw_mode::UnTextured,
                draw_mode::Opaque,
            >()),
            0x2c => Some(self.gp0_quad_poly::<
                draw_mode::UnShaded,
                draw_mode::Textured,
                draw_mode::Opaque,
            >()),
            0x2d => Some(self.gp0_quad_poly::<
                draw_mode::UnShaded,
                draw_mode::TexturedRaw,
                draw_mode::Opaque,
            >()),
            0x30 => Some(self.gp0_tri_poly::<
                draw_mode::Shaded,
                draw_mode::UnTextured,
                draw_mode::Opaque,
            >()),
            0x38 => Some(self.gp0_quad_poly::<
                draw_mode::Shaded,
                draw_mode::UnTextured,
                draw_mode::Opaque,
            >()),
            0x40 => Some(self.gp0_line::<
                draw_mode::UnShaded,
                draw_mode::Opaque,
            >()),
            0x44 => Some(self.gp0_line::<
                draw_mode::UnShaded,
                draw_mode::Opaque,
            >()),
            0x65 => Some(self.gp0_rect::<
                draw_mode::Textured,
                draw_mode::Opaque,
            >(None)),
            0xa0 => {
                self.gp0_copy_rect_cpu_to_vram();
                None
            }
            0xc0 => {
                self.gp0_copy_rect_vram_to_cpu();
                None
            }
            cmd => unimplemented!("Invalid GP0 command {:08x}.", cmd),
        };

        if let Some(cycles) = cycles {
            self.state = State::Drawing;
            schedule.schedule_in(cycles, Event::GpuCmdDone);
        }
    }
}

/// How to blend two colors. Used mainly for blending the color of a shape being drawn with the color
/// behind it.
#[derive(Clone, Copy)]
pub enum TransBlend {
    Avg = 0,
    Add = 1,
    Sub = 2,
    AddDiv = 3,
}

impl TransBlend {
    fn from_value(value: u32) -> Self {
        match value {
            0 => TransBlend::Avg,
            1 => TransBlend::Add,
            2 => TransBlend::Sub,
            3 => TransBlend::AddDiv,
            _ => unreachable!("Invalid transparency blending"),
        }
    }

    pub fn blend(self, a: Color, b: Color) -> Color {
        match self {
            TransBlend::Avg => a.avg_blend(b),
            TransBlend::Add => a.add_blend(b),
            TransBlend::Sub => a.sub_blend(b),
            TransBlend::AddDiv => a.add_div_blend(b),
        }
    }
}

impl fmt::Display for TransBlend {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            TransBlend::Avg => "average",
            TransBlend::Add => "add",
            TransBlend::Sub => "subtract",
            TransBlend::AddDiv => "add and divide",
        })
    }
}

/// Video mode mainly determines the output framerate. It depends on the region of the console,
/// North American consoles uses NTSC for instance, while European consoles uses PAL. Every console
/// can output both modes, so it purely determined by bios.
#[derive(Clone, Copy)]
pub enum VideoMode {
    /// ~ 60 Hz.
    Ntsc = 60,
    /// ~ 50 Hz.
    Pal = 50,
}

impl VideoMode {
    fn cycles_per_scln(self) -> Cycle {
        match self {
            VideoMode::Ntsc => timing::NTSC_CYCLES_PER_SCLN,
            VideoMode::Pal => timing::PAL_CYCLES_PER_SCLN,
        }
    }

    fn scln_count(self) -> Cycle {
        match self {
            VideoMode::Ntsc => timing::NTSC_SCLN_COUNT,
            VideoMode::Pal => timing::PAL_SCLN_COUNT,
        }
    }

    fn vbegin(self) -> Cycle {
        match self {
            VideoMode::Ntsc => timing::NTSC_VBEGIN,
            VideoMode::Pal => timing::PAL_VBEGIN,
        }
    }

    fn vend(self) -> Cycle {
        match self {
            VideoMode::Ntsc => timing::NTSC_VEND,
            VideoMode::Pal => timing::PAL_VEND,
        }
    }
}

impl fmt::Display for VideoMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            VideoMode::Ntsc => "NTSC(60hz)",
            VideoMode::Pal => "PAL(50hz)",
        })
    }
}

#[derive(PartialEq, Eq)]
pub enum DmaDir {
    Off = 0,
    Fifo = 1,
    CpuToGp0 = 2,
    VramToCpu = 3,
}

impl fmt::Display for DmaDir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}",match *self {
            DmaDir::Off => "off",
            DmaDir::Fifo => "FIFO",
            DmaDir::CpuToGp0 => "CPU to GP0",
            DmaDir::VramToCpu => "VRAM to CPU",
        })
    }
}

/// Which lines to show.
#[derive(PartialEq, Eq)]
pub enum InterlaceField {
    Bottom = 0,
    Top = 1,
}

impl fmt::Display for InterlaceField {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            InterlaceField::Bottom => "top/even",
            InterlaceField::Top => "bottom/odd",
        })
    }
}

/// Number of bits used to represent a single pixel.
pub enum ColorDepth {
    B15 = 15,
    B24 = 24,
}

impl fmt::Display for ColorDepth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            ColorDepth::B15 => "15 bit",
            ColorDepth::B24 => "24 bit",
        })
    }
}

/// Number of bits used to represent a single texel.
pub enum TexelDepth {
    B4 = 4,
    B8 = 8,
    B15 = 15,
}

impl TexelDepth {
    fn from_value(value: u32) -> Self {
        match value {
            0 => TexelDepth::B4,
            1 => TexelDepth::B8,
            2 | 3 => TexelDepth::B15,
            _ => unreachable!("Invalid texture depth"),
        }
    }
}

impl fmt::Display for TexelDepth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            TexelDepth::B4 => "4 bit",
            TexelDepth::B8 => "8 bit",
            TexelDepth::B15 => "15 bit",
        })
    }
}

/// An ongoing memory transfer between Bus and VRAM.
#[derive(Clone, Copy)]
struct MemTransfer {
    x: i32,
    y: i32,
    x_start: i32,
    x_end: i32,
    y_end: i32,
}

impl MemTransfer {
    fn new(x_start: i32, y_start: i32, width: i32, height: i32) -> Self {
        Self {
            x: x_start,
            y: y_start,
            x_start,
            x_end: x_start + width,
            y_end: y_start + height,
        }
    }

    fn next(&mut self) {
        self.x += 1;
        if self.x == self.x_end {
            self.x = self.x_start;
            self.y += 1;
        }
    }

    fn is_done(&self) -> bool {
        self.y >= self.y_end
    }
}


/// Status register of the GPU.
#[derive(Clone, Copy)]
pub struct Status(pub u32);

impl Status {
    /// Texture page x base coordinate. N * 64.
    pub fn tex_page_x(self) -> i32 {
        self.0.bit_range(0, 3) as i32 * 64
    }

    /// Texture page y base coordinate. N * 256.
    pub fn tex_page_y(self) -> i32 {
        self.0.bit(4) as i32 * 256
    }

    /// How to blend source and destination colors.
    pub fn blend_mode(self) -> TransBlend {
        TransBlend::from_value(self.0.bit_range(5, 6))
    }

    /// Depth of the texture colors.
    pub fn texture_depth(self) -> TexelDepth {
        TexelDepth::from_value(self.0.bit_range(7, 8))
    }

    pub fn dithering_enabled(self) -> bool {
        self.0.bit(9)
    }

    /// Draw pixels to display if true.
    pub fn draw_to_display(self) -> bool {
        self.0.bit(10)
    }

    /// Set the mask bit of each pixel when writing to VRAM.
    pub fn set_mask_bit(self) -> bool {
        self.0.bit(11)
    }

    /// Draw pixels with mask bit set if true.
    pub fn draw_masked_pixels(self) -> bool {
        self.0.bit(12)
    }

    /// The interlace field currently being displayed.
    pub fn interlace_field(self) -> InterlaceField {
        match self.0.bit(13) {
            false => InterlaceField::Bottom,
            true => InterlaceField::Top,
        }
    }

    #[allow(dead_code)]
    fn reversed(self) -> bool {
        self.0.bit(14)
    }

    pub fn texture_disabled(self) -> bool {
        self.0.bit(15)
    }

    pub fn horizontal_res(self) -> u32 {
        match self.0.bit(16) {
            true => 368,
            false => match self.0.bit_range(17, 18) {
                0 => 256,
                1 => 480,
                2 => 512,
                3 => 640,
                _ => unreachable!("Invalid vres"),
            },
        }
    }

    pub fn vertical_res(self) -> u32 {
        240 * (self.0.bit(19) as u32 + 1)
    }

    pub fn video_mode(self) -> VideoMode {
        match self.0.bit(20) {
            false => VideoMode::Ntsc,
            true => VideoMode::Pal,
        }
    }

    /// Depth of each pixel being drawn.
    pub fn color_depth(self) -> ColorDepth {
        match self.0.bit(21) {
            false => ColorDepth::B15,
            true => ColorDepth::B24,
        }
    } 

    /// # Vertical interlacing
    ///
    /// PAL and NTSC Tv's are normally able to display between 25-30 frames per second. To allow
    /// for higher framerates, the Playstation has the ability to use vertical interlacing.
    /// Vertical interlacing sends only half the scanlines at a time, which effectively halfs the
    /// bandwidth for each frame. It switches between even and odd lines each frame, creating an
    /// illusion of showing the full image at double the native speed.
    pub fn vertical_interlace(self) -> bool {
        self.0.bit(22)
    }

    /// Tells if the GPU should draw to the screen currently being displayed in interlace mode.
    pub fn draw_to_displayed(self) -> bool {
        self.0.bit(23)
    }

    pub fn irq_enabled(self) -> bool {
        self.0.bit(24)
    }

    pub fn dma_data_request(self) -> bool {
        self.0.bit(25)
    }

    /// Ready to recieve commands.
    pub fn cmd_ready(self) -> bool {
        self.0.bit(26)
    }

    /// Ready to transfer from vram to CPU/Memory.
    pub fn vram_to_cpu_ready(self) -> bool {
        self.0.bit(27)
    }

    /// Ready to do DMA block transfer.
    pub fn dma_block_ready(self) -> bool {
        self.0.bit(28)
    }

    /// Direction of DMA request.
    pub fn dma_direction(self) -> DmaDir {
        match self.0.bit_range(29, 30) {
            0 => DmaDir::Off,
            1 => DmaDir::Fifo,
            2 => DmaDir::CpuToGp0,
            3 => DmaDir::VramToCpu,
            _ => unreachable!("Invalid dma direction"),
        }
    }

    /// If 'vertical_interlace' is enabled for when 'vertical_res' is 240 bits, it switches between
    /// each between the same line each frame, so this tells if the GPU actually uses two fields in
    /// VRAM.
    fn interlaced_480(self) -> bool {
        self.vertical_interlace() && self.vertical_res() == 480
    }
}

struct Timing {
    /// The current scanline.
    scln: u16,
    /// The current progress into the scanline in dot cycles.
    scln_prog: u64,
    /// The absolute CPU cycle the timings was last updated.
    last_update: Cycle,
    /// The absolute number of the previous frame.
    frame_count: u64,
    /// How many cycles it takes to draw a scanline which depend on ['VideoMode'].
    cycles_per_scln: Cycle,
    /// How many scanlines there are which depend on ['VideoMode'].
    scln_count: u16,
    /// Vertical begin.
    vbegin: u16, 
    /// Vertical end.
    vend: u16,
    in_hblank: bool,
    in_vblank: bool,
}

impl Timing {
    fn new(vmode: VideoMode) -> Self {
        Self {
            scln: 0,
            scln_prog: 0,
            in_hblank: false,
            in_vblank: false,
            last_update: 0,
            frame_count: 0,
            cycles_per_scln: vmode.cycles_per_scln(),
            scln_count: vmode.scln_count() as u16,
            vbegin: vmode.vbegin() as u16,
            vend: vmode.vend() as u16,
        }
    }

    fn update_video_mode(&mut self, vmode: VideoMode) {
        self.cycles_per_scln = vmode.cycles_per_scln();
        self.scln_count = vmode.scln_count() as u16;
        self.vbegin = vmode.vbegin() as u16;
        self.vend = vmode.vend() as u16;

        // Make sure the current scanline is in range.
        self.scln %= self.scln_count;
        self.scln_prog %= self.cycles_per_scln;
    }

    /// The current displayed line in VRAM. The absolute line depend on the first line displayed.
    fn display_line(&self, interlaced_480: bool, field: InterlaceField) -> u16 {
        let offset = match interlaced_480 {
            false => self.scln,
            true => {
                let bottom = match self.in_vblank {
                    true => (field == InterlaceField::Bottom) as u16,
                    false => 0,
                };
                (self.scln << 1) | bottom
            }
        };

        self.scln + offset
    }
}

/// The current state of the GPU.
enum State {
    Idle,
    Drawing,
    VramStore(MemTransfer),
    VramLoad(MemTransfer),
}

impl State {
    fn is_vram_load(&self) -> bool {
        matches!(self, State::VramLoad(..))
    }

    fn is_idle(&self) -> bool {
        matches!(self, State::Idle)
    }
}

impl DmaChan for Gpu {
    fn dma_store(&mut self, schedule: &mut Schedule, val: u32) {
        self.gp0_store(schedule, val);
    }

    fn dma_load(&mut self, _: &mut Schedule, _: (u16, u32)) -> u32 {
        if self.status.dma_direction() != DmaDir::VramToCpu {
            warn!("Invalid DMA load from GPU");
            u32::MAX
        } else {
            self.gpu_read()
        }
    }

    fn dma_ready(&self, dir: ChanDir) -> bool {
        match dir {
            ChanDir::ToRam => true,
            ChanDir::ToPort => self.dma_block_ready(),
        }
    }
}

impl BusMap for Gpu {
    const BUS_BEGIN: u32 = 0x1f801810;
    const BUS_END: u32 = Self::BUS_BEGIN + 8 - 1;
}
