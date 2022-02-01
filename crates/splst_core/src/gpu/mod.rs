//! Represent's the GPU of the Playstation 1.

mod fifo;
mod primitive;
mod rasterize;
pub mod vram;

use splst_util::{Bit, BitSet};
use crate::cpu::Irq;
use crate::bus::{DmaChan, ChanDir, Schedule, Event, BusMap, AddrUnit};
use crate::timing;
use crate::timer::Timers;
use crate::{Cycle, DrawInfo};

use fifo::Fifo;
use primitive::{Color, Point, TexCoord, TextureParams, Vertex};
use rasterize::{Opaque, Shaded, Shading, Textured, Textureing, Transparency, UnShaded, UnTextured};

use std::fmt;

pub use vram::Vram;

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
    /// ~60 Hz.
    Ntsc = 60,
    /// ~50 Hz.
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
            2 => TexelDepth::B15,
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

    fn next(&mut self) -> bool {
        self.x += 1;
        if self.x == self.x_end {
            self.x = self.x_start;
            self.y += 1;
            !self.is_done()
        } else {
            true
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
    pub fn texture_page_x_base(self) -> u32 {
        self.0.bit_range(0, 3) * 64
    }

    /// Texture page y base coordinate. N * 256.
    pub fn texture_page_y_base(self) -> u32 {
        self.0.bit(4) as u32 * 256
    }

    /// How to blend source and destination colors.
    pub fn trans_blending(self) -> TransBlend {
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
    fn interlaced480(self) -> bool {
        self.vertical_interlace() && self.vertical_res() == 480
    }
}

struct Timing {
    /// The current scanline.
    scanline: u64,
    /// The current progress into the scanline in dot cycles.
    scanline_prog: u64,
    in_hblank: bool,
    in_vblank: bool,
    last_run: Cycle,
    frame_count: u64,
    /// How many cycles it takes to draw a scanline which depend on ['VideoMode'].
    cycles_per_scln: Cycle,
    /// How many scanlines there are which depend on ['VideoMode'].
    scln_count: u64,
    /// Vertical begin.
    vbegin: u64, 
    /// Vertical end.
    vend: u64,
}

impl Timing {
    fn new(vmode: VideoMode) -> Self {
        Self {
            scanline: 0,
            scanline_prog: 0,
            in_hblank: false,
            in_vblank: false,
            last_run: 0,
            frame_count: 0,
            cycles_per_scln: vmode.cycles_per_scln(),
            scln_count: vmode.scln_count(),
            vbegin: vmode.vbegin(),
            vend: vmode.vend(),
        }
    }

    fn update_video_mode(&mut self, vmode: VideoMode) {
        self.cycles_per_scln = vmode.cycles_per_scln();
        self.scln_count = vmode.scln_count();
        self.vbegin = vmode.vbegin();
        self.vend = vmode.vend();
        
        self.scanline %= self.scln_count;
        self.scanline_prog %= self.cycles_per_scln;

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

pub struct Gpu {
    state: State,
    fifo: Fifo,
    vram: Vram,
    timing: Timing,
    pub status: Status,
    /// The GPUREAD register. This contains various info about the GPU, and is generated after each
    /// GP1(10) command call. It is read through the BUS at GPUREAD, if no transfer is ongoing.
    gpu_read: u32,
    /// Mirros textured rectangles on the x axis if true,
    rect_tex_x_flip: bool,
    /// Mirros textured rectangles on the y axis if true,
    #[allow(dead_code)]
    rect_tex_y_flip: bool,
    /// Texture window x mask.
    tex_window_x_mask: u8,
    /// Texture window y mask.
    tex_window_y_mask: u8,
    /// Texture window x offset.
    tex_window_x_offset: u8,
    /// Texture window y offset.
    tex_window_y_offset: u8,
    draw_area_left: u16,
    draw_area_right: u16,
    draw_area_top: u16,
    draw_area_bottom: u16,
    pub draw_x_offset: i16,
    pub draw_y_offset: i16,
    /// The first column display area in VRAM.
    pub display_vram_x_start: u16,
    /// The first line display area in VRAM.
    pub display_vram_y_start: u16,
    pub display_column_start: u16,
    pub display_column_end: u16,
    pub display_line_start: u16,
    pub display_line_end: u16,
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
            rect_tex_x_flip: false,
            rect_tex_y_flip: false,
            tex_window_x_mask: 0x0,
            tex_window_y_mask: 0x0,
            tex_window_x_offset: 0x0,
            tex_window_y_offset: 0x0,
            draw_area_left: 0x0,
            draw_area_right: 0x0,
            draw_area_top: 0x0,
            draw_area_bottom: 0x0,
            draw_x_offset: 0x0,
            draw_y_offset: 0x0,
            display_vram_x_start: 0x0,
            display_vram_y_start: 0x0,
            display_column_start: 0x200,
            display_column_end: 0xc00,
            display_line_start: 0x10,
            display_line_end: 0x100,
        }
    }

    pub fn store<T: AddrUnit>(&mut self, schedule: &mut Schedule, addr: u32, val: u32) {
        debug_assert_eq!(T::WIDTH, 4);
        match addr {
            0 => self.gp0_store(schedule, val),
            4 => self.gp1_store(val),
            _ => unreachable!("Invalid GPU store at offset {:08x}.", addr),
        }
    }

    pub fn load<T: AddrUnit>(&mut self, addr: u32) -> u32 {
        debug_assert_eq!(T::WIDTH, 4);
        match addr {
            0 => self.gpu_read(),
            4 => self.status_read(),
            _ => unreachable!("Invalid GPU load at offset {:08x}.", addr),
        }
    }

    fn gpu_read(&mut self) -> u32 {
        if let State::VramLoad(ref mut tran) = self.state {
            self.gpu_read = [0, 16].iter().fold(0, |state, shift| {
                let value = self.vram.load_16(tran.x, tran.y) as u32;
                tran.next();
                state | (value << shift)
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
            State::Drawing | State::VramLoad(..) => false,
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
        self.fifo.push(val);
        self.try_run_cmd(schedule);
    }

    fn try_run_cmd(&mut self, schedule: &mut Schedule) {
        match self.state {
            State::Idle => {
                if self.fifo.has_full_cmd() {
                    self.gp0_exec(schedule);
                }
            }
            State::VramStore(ref mut tran) => {
                let val = self.fifo.pop();
                for (lo, hi) in [(0, 15), (16, 31)] {
                    let value = val.bit_range(lo, hi) as u16;
                    self.vram.store_16(tran.x, tran.y, value);
                    tran.next();
                }
                if tran.is_done() {
                    self.state = State::Idle;
                }
            }
            State::VramLoad(..) => {
                warn!("GPU state is VramLoad when trying to execute command");
            }
            State::Drawing => (),
        }
    }

    pub fn vram(&self) -> &Vram {
        &self.vram
    }

    pub fn draw_info(&self) -> DrawInfo {
        DrawInfo {
            vram_x_start: self.display_vram_x_start as u32,
            vram_y_start: self.display_vram_y_start as u32,
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

    pub fn run(&mut self, schedule: &mut Schedule, timers: &mut Timers) {
        self.try_run_cmd(schedule);

        self.timing.scanline_prog += timing::cpu_to_gpu_cycles(schedule.cycle() - self.timing.last_run);
        self.timing.last_run = schedule.cycle();

        // If the progress is less than a single scanline.
        if self.timing.scanline_prog < self.timing.cycles_per_scln {
            let in_hblank = self.timing.scanline_prog >= timing::HSYNC_CYCLES;

            // If we have entered Hblank.
            if in_hblank && !self.timing.in_hblank {
                timers.hblank(schedule, 1);
            }

            self.timing.in_hblank = in_hblank;
        } else {
            // Calculate the number of lines to be drawn.
            let mut lines = self.timing.scanline_prog / self.timing.cycles_per_scln;
            self.timing.scanline_prog %= self.timing.cycles_per_scln;

            // At there must have been atleast a single Hblank, this calculates the amount.
            //
            // If the GPU wasn't in Hblank, it must have entered since then, which adds one to the
            // count. We know it's going to enter into Hblank on each scanline, except the current
            // one it's on, which is represented by 'in_hblank'.
            let in_hblank = self.timing.scanline_prog >= timing::HSYNC_CYCLES;
            let hblank_count = u64::from(!self.timing.in_hblank)
                + u64::from(in_hblank)
                + lines - 1;

            timers.hblank(schedule, hblank_count);
            self.timing.in_hblank = in_hblank;

            while lines > 0 {
                let line_count = u64::min(lines, self.timing.scln_count - self.timing.scanline);
                lines -= line_count;

                let scanline = self.timing.scanline + line_count;

                // Calculate if the scanlines being drawn enters the display area, and clear the
                // Vblank flag if not.
                if self.timing.scanline < self.timing.vbegin && scanline >= self.timing.vend {
                    // TODO: Timer sync.
                    self.timing.in_vblank = false;
                }

                self.timing.scanline = scanline;

                let in_vblank = !timing::NTSC_VERTICAL_RANGE.contains(&scanline);

                // If we are either leaving or entering Vblank.
                if self.timing.in_vblank != in_vblank {
                    if in_vblank {
                        self.timing.frame_count += 1;
                        schedule.schedule_now(Event::IrqTrigger(Irq::VBlank));
                    }
                    self.timing.in_vblank = in_vblank;
                    // TODO: Timer sync.
                }

                // Prepare new frame if we are at the end of Vblank.
                if self.timing.scanline == self.timing.scln_count {
                    self.timing.scanline = 0;
                    // The interlace field is toggled every frame if vertical interlace is turned on.
                    if self.status.interlaced480() {
                        self.status.0 ^= 1 << 13;
                    } else {
                        self.status.0 &= !(1 << 13);
                    }
                }
            }
        }
        // The the current line being displayed in VRAM. It's used here to determine
        // the value of bit 31 of 'status'.
        let line_offset = if self.status.interlaced480() {
            let offset = if self.timing.in_vblank {
                (self.status.interlace_field() == InterlaceField::Bottom) as u16
            } else {
                0
            };
            (self.timing.scanline << 1) as u16 | offset
        } else {
            self.timing.scanline as u16
        };
        let vram_line = self.display_vram_y_start + line_offset;
        self.status.0 = self.status.0.set_bit(31, vram_line.bit(0));

        schedule.schedule_in(10_000, Event::RunGpu);
    }

    fn gp1_store(&mut self, val: u32) {
        let cmd = val.bit_range(24, 31);
        match cmd {
            // GP1(0) - Resets the state of the GPU.
            0x0 => {
                *self = Self::new();
            }
            // GP1(1) - Reset command buffer.
            0x1 => {
                self.fifo.clear();
            }
            // GP1(2) - Acknowledge GPU Interrupt.
            0x2 => {
                self.status.0 &= !(1 << 24); 
            }
            // GP1(3) - Display Enable.
            // - 0 - Display On/Off.
            0x3 => {
                self.status.0 = self.status.0.set_bit(23, val.bit(0));
            }
            // GP1(4) - Set DMA Direction.
            // - 0..1 - DMA direction.
            0x4 => {
                let val = val.bit_range(0, 1);
                self.status.0 = self.status.0.set_bit_range(29, 30, val);
            }
            // GP1(5) - Start display area in VRAM.
            // - 0..9 - x (address in VRAM).
            // - 10..18 - y (address in VRAM).
            0x5 => {
                self.display_vram_x_start = val.bit_range(0, 9) as u16;
                self.display_vram_y_start = val.bit_range(10, 18) as u16;
            }
            // GP1(6) - Horizontal display range.
            // - 0..11 - column start.
            // - 12..23 - column end.
            0x6 => {
                self.display_column_start = val.bit_range(0, 11) as u16;
                self.display_column_end = val.bit_range(12, 23) as u16;
            }
            // GP1(7) - Vertical display range.
            // - 0..11 - line start.
            // - 12..23 - line end.
            0x7 => {
                self.display_line_start = val.bit_range(0, 11) as u16;
                self.display_line_end = val.bit_range(12, 23) as u16;
            }
            // GP1(8) - Set display mode.
            // - 0..1 - Horizontal resolution 1.
            // - 2 - Vertical resolution.
            // - 3 - Display mode.
            // - 4 - Display area color depth.
            // - 5 - Vertical interlace.
            // - 6 - Horizontal resolution 2.
            // - 7 - Reverseflag.
            0x8 => {
                self.status.0 = self.status.0
                    .set_bit_range(17, 22, val.bit_range(0, 5))
                    .set_bit(16, val.bit(6))
                    .set_bit(14, val.bit(7));
                self.timing.update_video_mode(self.status.video_mode());
            }
            0xff => {
                warn!("Weird GP1 command: GP1(ff)");
            }
            _ => unimplemented!("Invalid GP1 command {:08x}.", cmd),
        }
    }

    fn gp0_exec(&mut self, schedule: &mut Schedule) {
        let cycles = match self.fifo[0].bit_range(24, 31) {
            // GP0(00) - Nop.
            0x0 => {
                self.fifo.pop();
                0
            }
            // GP0(01) - Clear texture cache.
            0x1 => {
                self.fifo.pop();
                // TODO: clear texture cache.
                0
            }
            // GP0(02) - Fill rectanlge in VRAM.
            0x2 => {
                let color = Color::from_cmd(self.fifo.pop());
                let val = self.fifo.pop() as i32;
                let start = Point {
                    x: val & 0x3f0,
                    y: (val >> 16) & 0x3ff,
                };
                let val = self.fifo.pop() as i32;
                let dim = Point {
                    x: ((val & 0x3ff) + 0xf) & !0xf,
                    y: (val >> 16) & 0x1ff,
                };
                self.fill_rect(color, start, dim); 
                0    
            }
            // GP0(e1) - Draw Mode Setting.
            // - 0..10 - Same as status register.
            // - 11 - Texture disabled.
            // - 12 - Texture rectangle x-flip.
            // - 13 - Texture rectangle y-flip.
            // - 14..23 - Not used.
            0xe1 => {
                let val = self.fifo.pop();
                self.status.0 = self.status.0
                    .set_bit_range(0, 10, val.bit_range(0, 10))
                    .set_bit(15, val.bit(11));
                self.rect_tex_x_flip = val.bit(12);
                self.rect_tex_x_flip = val.bit(13);
                0
            }
            // GP0(e2) - Texture window setting.
            // - 0..4 - Texture window mask x.
            // - 5..9 - Texture window mask y.
            // - 10..14 - Texture window offset x.
            // - 15..19 - Texture window offset y.
            0xe2 => {
                let val = self.fifo.pop();
                self.tex_window_x_mask = val.bit_range(0, 4) as u8;
                self.tex_window_y_mask = val.bit_range(5, 9) as u8;
                self.tex_window_x_offset = val.bit_range(10, 14) as u8;
                self.tex_window_y_offset = val.bit_range(15, 19) as u8;
                0
            }
            // GP0(e3) - Set draw area top left.
            // - 0..9 - Draw area left.
            // - 10..18 - Draw area top.
            // TODO this differs between GPU versions.
            0xe3 => {
                let val = self.fifo.pop();
                self.draw_area_left = val.bit_range(0, 9) as u16;
                self.draw_area_top = val.bit_range(10, 18) as u16;
                0
            }
            // GP0(e4) - Set draw area bottom right.
            // - 0..9 - Draw area right.
            // - 10..18 - Draw area bottom.
            0xe4 => {
                let val = self.fifo.pop();
                self.draw_area_right = val.bit_range(0, 9) as u16;
                self.draw_area_bottom = val.bit_range(10, 18) as u16;
                0
            }
            // GP0(e5) - Set drawing offset.
            // - 0..10 - x-offset.
            // - 11..21 - y-offset.
            // - 24..23 - Not used.
            0xe5 => {
                let val = self.fifo.pop();
                let x_offset = val.bit_range(0, 10) as u16;
                let y_offset = val.bit_range(11, 21) as u16;
                // Because the command stores the values as 11 bit signed integers, the values have to be
                // bit-shifted to the most significant bits in order to make Rust generate sign extension.
                self.draw_x_offset = ((x_offset << 5) as i16) >> 5;
                self.draw_y_offset = ((y_offset << 5) as i16) >> 5;
                0
            }
            // GP0(e6) - Mask bit setting.
            // - 0 - Set mask while drawing.
            // - 1 - Check mask before drawing.
            0xe6 => {
                let val = self.fifo.pop().bit_range(0, 1);
                self.status.0 = self.status.0.set_bit_range(11, 12, val);
                0
            }
            0x28 => self.gp0_quad_poly::<UnShaded, UnTextured, Opaque>(),
            0x2c => self.gp0_quad_poly::<UnShaded, Textured, Opaque>(),
            0x30 => self.gp0_tri_poly::<Shaded, UnTextured, Opaque>(),
            0x38 => self.gp0_quad_poly::<Shaded, UnTextured, Opaque>(),
            // Opaque no shading.
            0x44 => {
                self.gp0_line();
                0
            }
            // GP0(a0) - Copy rectangle from CPU to VRAM.
            0xa0 => {
                self.fifo.pop();
                let (pos, dim) = (self.fifo.pop(), self.fifo.pop());
                let x = pos.bit_range(00, 15) as i32;
                let y = pos.bit_range(16, 31) as i32;
                let w = dim.bit_range(00, 15) as i32;
                let h = dim.bit_range(16, 31) as i32;
                self.state = State::VramStore(MemTransfer::new(x, y, w, h));
                0
            }
            // GP0(c0) - Copy rectanlge from VRAM to CPU.
            0xc0 => {
                self.fifo.pop();
                let (pos, dim) = (self.fifo.pop(), self.fifo.pop());
                let x = pos.bit_range(00, 15) as i32;
                let y = pos.bit_range(16, 31) as i32;
                let w = dim.bit_range(00, 15) as i32;
                let h = dim.bit_range(16, 31) as i32;
                self.state = State::VramLoad(MemTransfer::new(x, y, w, h));
                0
            }
            cmd => unimplemented!("Invalid GP0 command {:08x}.", cmd),
        };
        if cycles != 0 {
            self.state = State::Drawing;
            schedule.schedule_in(cycles, Event::GpuCmdDone);
        }
    }

    fn gp0_tri_poly<Shade, Tex, Trans>(&mut self) -> Cycle
    where
        Shade: Shading,
        Tex: Textureing,
        Trans: Transparency,
    {
        let mut verts = [Vertex::default(); 3];
        let mut params = TextureParams::default();

        let color = if !Shade::IS_SHADED {
            Color::from_cmd(self.fifo.pop())
        } else {
            Color::from_rgb(0, 0, 0)
        };

        for (i, vertex) in verts.iter_mut().enumerate() {
            if Shade::IS_SHADED {
                vertex.color = Color::from_cmd(self.fifo.pop());
            }

            vertex.point = Point::from_cmd(self.fifo.pop()).with_offset(
                self.draw_x_offset as i32,
                self.draw_y_offset as i32,
            );

            if Tex::IS_TEXTURED {
                let value = self.fifo.pop();
                match i {
                    0 => {
                        let value = (value >> 16) as i32;
                        params.clut_x = value.bit_range(0, 5) * 16;
                        params.clut_y = value.bit_range(6, 14);
                    }
                    1 => {
                        let value = (value >> 16) as i32;
                        params.texture_x = value.bit_range(0, 3) * 64;
                        params.texture_y = value.bit(4) as i32 * 256;
                        params.blend_mode = TransBlend::from_value(
                            value.bit_range(5, 6) as u32
                        );
                        params.texel_depth = TexelDepth::from_value(
                            value.bit_range(7, 8) as u32
                        );
                    }
                    _ => {}
                }
                vertex.texcoord = TexCoord {
                    u: value.bit_range(0, 7) as u8,
                    v: value.bit_range(8, 15) as u8,
                };
            }
        }
        let cycles = self.draw_triangle::<Shade, Tex, Trans>(color, &params, &verts[0], &verts[1], &verts[2]);
        timing::gpu_to_cpu_cycles(cycles)
    }

    fn gp0_quad_poly<Shade, Tex, Trans>(&mut self) -> Cycle
    where
        Shade: Shading,
        Tex: Textureing,
        Trans: Transparency,
    {
        let mut verts = [Vertex::default(); 4];
        let mut params = TextureParams::default();

        let color = if !Shade::IS_SHADED {
            Color::from_cmd(self.fifo.pop())
        } else {
            Color::from_rgb(0, 0, 0)
        };

        for (i, vertex) in verts.iter_mut().enumerate() {
            // If it's shaded the color is always the first attribute.
            if Shade::IS_SHADED {
                vertex.color = Color::from_cmd(self.fifo.pop());
            }

            vertex.point = Point::from_cmd(self.fifo.pop()).with_offset(
                self.draw_x_offset as i32,
                self.draw_y_offset as i32,
            );

            if Tex::IS_TEXTURED {
                let value = self.fifo.pop();
                match i {
                    0 => {
                        let value = (value >> 16) as i32;
                        params.clut_x = value.bit_range(0, 5) * 16;
                        params.clut_y = value.bit_range(6, 14);
                    }
                    1 => {
                        let value = (value >> 16) as i32;
                        params.texture_x = value.bit_range(0, 3) * 64;
                        params.texture_y = value.bit(4) as i32 * 256;
                        params.blend_mode = TransBlend::from_value(
                            value.bit_range(5, 6) as u32
                        );
                        params.texel_depth = TexelDepth::from_value(
                            value.bit_range(7, 8) as u32
                        );
                    }
                    _ => {}
                }
                vertex.texcoord = TexCoord {
                    u: value.bit_range(0, 7) as u8,
                    v: value.bit_range(8, 15) as u8,
                };
            }
        }
        let tri1 = self.draw_triangle::<Shade, Tex, Trans>(color, &params, &verts[0], &verts[1], &verts[2]);
        let tri2 = self.draw_triangle::<Shade, Tex, Trans>(color, &params, &verts[1], &verts[2], &verts[3]);
        timing::gpu_to_cpu_cycles(tri1 + tri2)
    }

    fn gp0_line(&mut self) {
        warn!("Drawing line");
        self.fifo.pop();
        let start = Point::from_cmd(self.fifo.pop()).with_offset(
            self.draw_x_offset as i32,
            self.draw_y_offset as i32,
        );
        let end = Point::from_cmd(self.fifo.pop()).with_offset(
            self.draw_x_offset as i32,
            self.draw_y_offset as i32,
        );
        self.draw_line(start, end);
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
