//! Emulation of the Playstations 1 GPU.
//!
//! # TODO
//!
//! - Maybe switch to integer fixed point for vertex attributes instead of floats in the
//!   rasterizer.
//!
//! - DMA chopping wouldn't work correctly with the GPU, since the GPU updates it's DMA channel
//!   after each GP0 write, which isn't handeled by the DMA.

pub mod fifo;
mod primitive;
mod rasterize;
mod gp0;
mod gp1;
mod vram;
mod texture;

use splst_util::{Bit, BitSet};
use crate::cpu::Irq;
use crate::bus::{self, dma, Bus, BusMap, AddrUnit};
use crate::schedule::{Event, EventId, Schedule};
use crate::timer::Timers;
use crate::{VideoOutput, SysTime};
use crate::{dump, dump::Dumper};

use fifo::PushAction;
use primitive::Color;
use gp0::draw_mode;
use texture::ClutCache;

use std::fmt;
use std::cell::RefCell;
use std::rc::Rc;

pub use vram::Vram;
pub use fifo::Fifo;

pub struct Gpu {
    pub(super) renderer: Rc<RefCell<dyn VideoOutput>>,
    /// The current state of the GPU.
    state: State,
    clut_cache: ClutCache,
    /// The GPU FIFO. Used to recieve commands and some kinds of data.
    fifo: Fifo,
    /// The Video Memory used to store texture data and the image buffer(s).
    vram: Box<Vram>,
    /// The status register.
    status: Status,
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
    x_offset: i16,
    /// Draw offset y. This is added to the y-coordinate of vertex and point drawn.
    y_offset: i16,
    /// The first column to be displayed on the screen.
    vram_x_start: u16,
    /// The first line to be displayed on the screen.
    vram_y_start: u16,
    /// Which column the the display area starts on the screen.
    dis_x_start: u16,
    /// Which column the the display area ends on the screen.
    dis_x_end: u16,
    /// Which row the the display area starts on the screen.
    dis_y_start: u16,
    /// Which row the the display area ends on the screen.
    dis_y_end: u16,
    /// The current scanline, between 0 and `scanline_count`.
    scanline: u16,
    /// The total number of scanlines.
    scanline_count: u16,
    /// The amount of time it takes to display a single scanline including the time in Hblank.
    scanline_time: SysTime,
    /// If the GPU is in VBlank. This is when 'scanline' is outside the display area, defined by
    /// `dis_y_start` and `dis_y_end`.
    in_vblank: bool,
    scanline_event: EventId,
}

impl Gpu {
    pub fn new(schedule: &mut Schedule, renderer: Rc<RefCell<dyn VideoOutput>>) -> Self {
        let dis_x_start = 0x200;
        let dis_y_start = 0x0;

        let dis_x_end = 0xc00;
        let dis_y_end = 0x100;

        let status = Status(0x14802000);

        let scanline_time = scanline_time(status);
        let scanline_count = scanline_count(status);
        
        let scanline_event =
            schedule.schedule_repeat(scanline_time, Event::Gpu(Self::end_of_scanline));
        
        Self {
            renderer,
            state: State::Idle,
            clut_cache: ClutCache::default(),
            fifo: Fifo::new(),
            vram: Box::new(Vram::new()),
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
            dis_x_start,
            dis_x_end,
            dis_y_start,
            dis_y_end,
            scanline: 0,
            scanline_count,
            scanline_time,
            in_vblank: false,
            scanline_event,
        }
    }

    pub fn store<T: AddrUnit>(
        &mut self,
        schedule: &mut Schedule,
        offset: u32,
        val: T,
    ) {
        if !T::WIDTH.is_word() {
            warn!("store of {} to GPU", T::WIDTH);
        }

        // This may not be correct.
        let val = val.as_u32();

        match bus::align_as::<u32>(offset) {
            0 => self.gp0_store(schedule, val),
            4 => self.gp1_store(schedule, val),
            offset => unreachable!("invalid GPU store at offset {offset:08x}"),
        }

        // `dma_ready` can have changed here, which means that the GPU DMA should be updated.
        // Maybe only check if something has changed which could change 'dma_ready', but that
        // could be sketchy. Running a DMA channel is pretty fast anyway, but maybe just running
        // the GPU channel every 1500 cycles or something could be faster?
        
        // This can be mess up chopping for CPU transfers. If theres already a pending event for a
        // chopped transfer, the block will be transfered early.

        schedule.trigger(Event::Dma(dma::Port::Gpu, Bus::run_dma_chan));
    }

    pub fn load<T: AddrUnit>(&mut self, offset: u32) -> T {
        if !T::WIDTH.is_word() {
            warn!("load of {} from GPU", T::WIDTH);
        }

        let val = match bus::align_as::<u32>(offset) {
            0 => self.gpu_read(),
            4 => self.status().0,
            offset => unreachable!("invalid GPU load at offset {offset:08x}"),
        };

        T::from_u32_aligned(val, offset)
    }

    /// 'load' without side effects.
    pub fn peek<T: AddrUnit>(&self, offset: u32) -> T {
        let val = match bus::align_as::<u32>(offset) {
            0 => self.gpu_read,
            4 => self.status().0,
            offset => unreachable!("invalid GPU load at offset {offset:08x}"),
        };
        T::from_u32_aligned(val, offset)
    }

    /// The GPU read register. Either loads data from the VRAM or results from the GPU 
    fn gpu_read(&mut self) -> u32 {
        if let State::VramLoad(ref mut tran) = self.state {
            self.gpu_read = [0, 16].iter().fold(0, |state, shift| {
                let val = self.vram.load_16(tran.x, tran.y) as u32;
                tran.next();
                (val << shift) | state
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

    pub fn status(&self) -> Status {
        trace!("GPU status load");
        let status = self.status.0
            .set_bit(27, self.state.is_vram_load())
            .set_bit(28, self.dma_block_ready())
            .set_bit(26, self.state.is_idle() && self.fifo.is_empty())
            .set_bit(25, match self.status.dma_direction() {
                DmaDir::Off => false,
                DmaDir::Fifo => !self.fifo.is_full(),
                DmaDir::CpuToGp0 => self.status.dma_block_ready(),
                DmaDir::VramToCpu => self.status.vram_to_cpu_ready(),
            });
        Status(status)
    }

    /// Store value in the GP0 register.
    fn gp0_store(&mut self, schedule: &mut Schedule, val: u32) {
        match self.state {
            State::Idle => match self.fifo.push_cmd(val) {
                Some(PushAction::FullCmd) => self.gp0_exec(schedule),
                Some(PushAction::ImmCmd) => self.gp0_imm_exec(val),
                None => (),
            }
            State::VramStore(_) => {
                self.fifo.push(val);
                self.fifo_to_vram_store();
            }
            State::Drawing => {
                if let Some(PushAction::ImmCmd) = self.fifo.push_cmd(val) {
                    self.gp0_imm_exec(val);
                }
            }
            State::VramLoad(_) => (),
        }
    }
    
    /// Transform dot cycles into [`SysTime`], which depend on the [`VideoMode`].
    pub(super) fn dot_cycles_to_systime(&self, cycles: u64) -> SysTime {
        match self.status.video_mode() {
            VideoMode::Ntsc => SysTime::from_gpu_ntsc_cycles(cycles),
            VideoMode::Pal => SysTime::from_gpu_pal_cycles(cycles),
        }
    }

    pub fn vram(&self) -> &Vram {
        &self.vram
    }
    
    pub fn fifo(&self) -> &Fifo {
        &self.fifo
    }
    
    pub fn state(&self) -> &State {
        &self.state
    }

    /// If the GPU is currently in VRAM.
    pub fn in_vblank(&self) -> bool {
        self.in_vblank
    }

    /// Handle that the GPU is at the end of the current scanline. This is used as an event
    /// callback.
    fn end_of_scanline(&mut self, schedule: &mut Schedule, timers: &mut Timers) {
        timers.hblank(schedule, 1);
        
        self.scanline += 1;

        if self.scanline == self.scanline_count {
            self.scanline = 0;
        } 
        
        // We are on the last line.
        if self.scanline == self.scanline_count - 1 {
            let top_field = if self.status.vertical_interlace() {
                !self.status.0.bit(13)
            } else {
                false  
            };

            self.status.0.set_bit(13, top_field);
        }
        
        // If we are entering leaving display area and thus entering Vblank.
        if self.scanline == self.dis_y_end {
            schedule.trigger(Event::Irq(Irq::VBlank));
            
            if self.status.interlaced_480() {
                self.status.0 ^= 1 << 13;
            }
            
            self.renderer.borrow_mut().send_frame(
                (self.vram_x_start as u32, self.vram_y_start as u32),
                &self.vram.raw_data(),
            );

            self.in_vblank = true;

            // TODO: Timer sync. Enter vblank.
        } else if self.scanline == self.dis_y_start {
            self.in_vblank = false;

            // TODO: Timer sync. No longer in vblank.
        }
        
        let vram_offset = {
            // Calculate offset from the first line in VRAM.
            let offset = if self.status.interlaced_480() {
                let bottom = self.in_vblank
                    && self.status.interlace_field() == InterlaceField::Bottom;
                (self.scanline  << 1) | bottom as u16
            } else {
                self.scanline
            };
            
            self.vram_y_start + offset
        };
        
        // Set bit 31 of the status register to indicite wether the current line being displayed
        // in VRAM is even.
        self.status.0 = self.status.0.set_bit(31, vram_offset.bit(0));
    }

    /// Store value in GP0 register.
    fn gp1_store(&mut self, schedule: &mut Schedule, val: u32) {
        match val.bit_range(24, 31) {
            0x0 => self.gp1_reset(schedule),
            0x1 => self.gp1_reset_fifo(),
            0x2 => self.gp1_ack_gpu_irq(),
            0x3 => self.gp1_display_enable(val),
            0x4 => self.gp1_dma_direction(val),
            0x5 => self.gp1_display_start(val),
            0x6 => self.gp1_horizontal_display_range(val),
            0x7 => self.gp1_vertical_display_range(val),
            0x8 => self.gp1_display_mode(schedule, val),
            0xff => warn!("weird GP1 command: GP1(ff)"),
            cmd => unimplemented!("invalid GP1 command {cmd:08x}"),
        }
    }

    /// Transfer data from the FIFO into VRAM until there either isn't any data left in the FIFO or
    /// or the transfer is done. Should only be called when the state of the GPU is
    /// [`State::VramStore`], otherwise nothing will happen.
    fn fifo_to_vram_store(&mut self) {
        if let State::VramStore(ref mut tran) = self.state {
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
    }

    /// Execute "immediate" GP0 commands which never enters the FIFO.
    fn gp0_imm_exec(&mut self, val: u32) {
        match val.bit_range(24, 31) {
            0x0 | 0x3..=0x1e => (),
            0xe3 => self.gp0_draw_area_top_left(val),
            0xe4 => self.gp0_draw_area_bottom_right(val),
            0xe5 => self.gp0_draw_offset(val),
            cmd => {
                unreachable!("invalid immediate GP0 command GP0({cmd:08x})");
            }
        }
    }

    /// Try to execute the next command in the command buffer. It will only execute if the state
    /// of the GPU is [`State::Idle`].
    fn try_gp0_exec(&mut self, schedule: &mut Schedule) {
        if let State::Idle = self.state {
            if self.fifo.has_full_cmd() {
                self.gp0_exec(schedule);
            }
        }
    }
    
    /// Execute GP0 command in FIFO. Should only be called if the FIFO has a full command.
    fn gp0_exec(&mut self, schedule: &mut Schedule) {
        let cycles = match self.fifo[0].bit_range(24, 31) {
            0x1 => {
                self.gp0_clear_texture_cache();
                None
            }
            0x2 => Some(self.gp0_fill_rect()),
            0xe1 => {
                self.gp0_draw_mode();
                None
            }
            0xe2 => {
                self.gp0_texture_window_settings();
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
            0x2f => Some(self.gp0_quad_poly::<
                draw_mode::UnShaded,
                draw_mode::TexturedRaw,
                draw_mode::Transparent,
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
            0x55 => Some(self.gp0_line::<
                draw_mode::Shaded,
                draw_mode::Opaque,
            >()),
            0x64 => Some(self.gp0_rect::<
                draw_mode::Textured,
                draw_mode::Opaque,
            >(None)),
            0x65 => Some(self.gp0_rect::<
                draw_mode::TexturedRaw,
                draw_mode::Opaque,
            >(None)),
            0xa0 => {
                self.gp0_copy_rect_cpu_to_vram();
                None
            }
            0xc0 => {
                self.gp0_copy_rect_vram_to_cpu();
                
                // There might be data in the FIFO here to be transfered to VRAM, and if no data
                // is written to GP0, then it might not be transfered for a while.
                self.fifo_to_vram_store(); 

                None
            }
            0xff => {
                self.gp0_useless();
                None
            }
            cmd => unimplemented!("Invalid GP0 command {:08x}.", cmd),
        };

        if let Some(cycles) = cycles {
            self.state = State::Drawing;
            
            // Schedule when the command is done drawing.
            schedule.schedule(cycles, Event::Gpu(|gpu, schedule, _| {
                gpu.state = State::Idle;
                gpu.try_gp0_exec(schedule);
            }));
        }
    }

    pub fn dump(&self, d: &mut impl Dumper) {
        dump!(d, "state", "{}", self.state);
        dump!(d, "fifo length", "{} / {} words", self.fifo().len(), Fifo::SIZE);
        dump!(d, "scanline", "{} / {}", self.scanline, self.scanline_count);
        dump!(d, "in vblank", "{}", self.in_vblank);
        dump!(d, "draw x-offset", "{}", self.x_offset);
        dump!(d, "draw y-offset", "{}", self.y_offset);
        dump!(d, "display vram x-start", "{}", self.vram_x_start);
        dump!(d, "display vram y-start", "{}", self.vram_y_start);
        dump!(d, "display x-start", "{}", self.dis_x_start);
        dump!(d, "display x-end", "{}", self.dis_x_end);
        dump!(d, "display y-start", "{}", self.dis_y_start);
        dump!(d, "display y-end", "{}", self.dis_y_end);
        dump!(d, "texture window width", "{}", self.tex_win_w);
        dump!(d, "texture window height", "{}", self.tex_win_h);
        dump!(d, "texture window x-start", "{}", self.tex_win_x);
        dump!(d, "texture window y-start", "{}", self.tex_win_y);
    }
}

/// How to blend two colors. Used only for blending with background color, not blending texture
/// and shading.
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
            _ => unreachable!("invalid transparency blending"),
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
/// can output both modes, so it purely determined by bios and game.
#[derive(Clone, Copy, PartialEq)]
pub enum VideoMode {
    /// ~ 60 Hz.
    Ntsc = 60,
    /// ~ 50 Hz.
    Pal = 50,
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

/// Which lines being displayed.
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

/// Number of bits used to represent a single pixel shown to the screen.
pub enum ColorDepth {
    /// The main mode used the majority of the time. This is the only resolution the GPU can
    /// rasterize to.
    ///
    /// Each pixel actually takes up 16 bits, however bit 15 is used as a mask pixel / 1 bit alpha channel.
    B15 = 15,
    /// This can only be used through direct writes to the framebuffer. It's used mainly for 
    /// MDEC cinematics.
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
#[derive(Clone, Copy)]
pub enum TexelDepth {
    /// This can output textures with 32 different colors. It contains an index into the color
    /// lookup table.
    B4 = 4,
    /// This can output textures with 256 different colors. It contains an index into the color
    /// lookup table.
    B8 = 8,
    /// Unlike `B4` and `B8`, this contains the full rgb values, which means it doesn't have to
    /// do a lookup in the color lookup table, and can therefore show full the 16.7 million
    /// different colors. Because VRAM and the texture cache is so limited in size, this format
    /// is rarely used.
    B15 = 15,
}

impl TexelDepth {
    fn from_value(val: u32) -> Self {
        match val {
            0 => TexelDepth::B4,
            1 => TexelDepth::B8,
            2 | 3 => TexelDepth::B15,
            _ => unreachable!("invalid texture depth"),
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

/// Horizontal resolution.
#[derive(Clone, Copy)]
pub enum HorizontalRes {
    P256 = 256,
    P320 = 320,
    P368 = 368,
    P512 = 512,
    P640 = 640,
}

impl fmt::Display for HorizontalRes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} pixels", *self as usize)
    }
}

/// Vertical resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VerticalRes {
    P240 = 240,
    P480 = 480,
}

impl fmt::Display for VerticalRes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} pixels", *self as usize)
    }
}

/// An ongoing memory transfer between Bus and VRAM.
#[derive(Clone, Copy, Debug)]
pub struct MemTransfer {
    /// The current x coordinate.
    x: i32,
    /// The current y coordinate.
    y: i32,
    /// The leftmost x coordinate in VRAM.
    x_start: i32,
    /// The rightmost x coordinate.
    x_end: i32,
    /// The biggest y coordinate.
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

    /// The next halfword in the transfer. Will go past the end of the transfer.
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

impl fmt::Display for MemTransfer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({} / {}, {} / {})", self.x, self.x_end, self.y, self.y_end)
    }
}

/// Status register of the GPU.
#[derive(Clone, Copy)]
pub struct Status(pub u32);

impl Status {
    /// Texture page x base coordinate.
    pub fn tex_page_x(self) -> i32 {
        self.0.bit_range(0, 3) as i32 * 64
    }

    /// Texture page y base coordinate.
    pub fn tex_page_y(self) -> i32 {
        self.0.bit(4) as i32 * 256
    }

    /// How to blend background and texture/shade colors.
    pub fn blend_mode(self) -> TransBlend {
        TransBlend::from_value(self.0.bit_range(5, 6))
    }

    /// Depth of the texture colors.
    pub fn texel_depth(self) -> TexelDepth {
        TexelDepth::from_value(self.0.bit_range(7, 8))
    }

    /// Ditering is noice added to primitives used to create in illusion of greater color depth.
    pub fn dithering_enabled(self) -> bool {
        self.0.bit(9)
    }

    /// If this is set, the GPU will draw to both displayed and non displayed lines, otherwise it
    /// will only draw to lines not being displayed. For this to have any effect, the GPU has to
    /// have interlace enabled and vertical resolution has to be 480. If this is case, draw times
    /// takes almost twice as long if this is set.
    pub fn draw_to_display(self) -> bool {
        self.0.bit(10)
    }

    /// Set the mask bit of each pixel when writing to VRAM.
    pub fn set_mask_bit(self) -> bool {
        self.0.bit(11)
    }

    /// Draw pixels with mask bit set.
    pub fn draw_masked_pixels(self) -> bool {
        self.0.bit(12)
    }

    /// The interlace field currently being displayed. If interlace is disabled and/or vertical
    /// resolution 240, this will always be [`InterlaceField::Top`], otherwise it changes each
    /// frame.
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

    /// If textures are disabled primitives which otherwise would be textured are instead colored
    /// by either shading or base color.
    pub fn texture_disabled(self) -> bool {
        self.0.bit(15)
    }

    /// Horizontal resolution.
    pub fn horizontal_res(self) -> HorizontalRes {
        match self.0.bit(16) {
            true => HorizontalRes::P368,
            false => match self.0.bit_range(17, 18) {
                0 => HorizontalRes::P256,
                1 => HorizontalRes::P320,
                2 => HorizontalRes::P512,
                3 => HorizontalRes::P640,
                _ => unreachable!(),
            }
        }
    }

    /// Vertical resolution.
    pub fn vertical_res(self) -> VerticalRes {
        match self.0.bit(19) {
            false => VerticalRes::P240,
            true => VerticalRes::P480,
        }
    }

    /// See [`VideoMode`].
    pub fn video_mode(self) -> VideoMode {
        match self.0.bit(20) {
            false => VideoMode::Ntsc,
            true => VideoMode::Pal,
        }
    }

    /// Depth of each pixel being drawn. This affects how the display area in VRAM is displayed not
    /// how it's drawn. Drawing primitives such as triangles, rectangles or lines always draws
    /// 15 bit depth. 24 bit image data has to be uploaded via direct VRAM transfers.
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

    /// If image data gets send to the TV.
    pub fn display_enabled(self) -> bool {
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
            _ => unreachable!("invalid dma direction"),
        }
    }

    /// If `vertical_interlace` is enabled for when `vertical_res` is 240 bits, it switches between
    /// each between the same line each frame, so this tells if the GPU actually uses two fields in
    /// VRAM.
    fn interlaced_480(self) -> bool {
        self.vertical_interlace() && self.vertical_res() == VerticalRes::P480
    }

    pub fn dump(&self, d: &mut impl Dumper) {
        dump!(d, "texture page x-base", "{}", self.tex_page_x());
        dump!(d, "texture page y-base", "{}", self.tex_page_y());
        dump!(d, "background blend mode", "{}", self.blend_mode());
        dump!(d, "texel depth", "{}", self.texel_depth());
        dump!(d, "dithering enabled", "{}", self.dithering_enabled());
        dump!(d, "draw to display", "{}", self.draw_to_display());
        dump!(d, "set mask bit", "{}", self.set_mask_bit());
        dump!(d, "draw masked pixels", "{}", self.draw_masked_pixels());
        dump!(d, "interlace field", "{}", self.interlace_field());
        dump!(d, "textures disabled", "{}", self.texture_disabled());
        dump!(d, "horizontal resolution", "{}", self.horizontal_res());
        dump!(d, "vertical resolution", "{}", self.vertical_res());
        dump!(d, "video mode", "{}", self.video_mode());
        dump!(d, "color depth", "{}", self.color_depth());
        dump!(d, "vertical interalce enabled", "{}", self.vertical_interlace());
        dump!(d, "vram to cpu ready", "{}", self.vram_to_cpu_ready());
        dump!(d, "dma block ready", "{}", self.dma_block_ready());
        dump!(d, "dma direction", "{}", self.dma_direction());
         
    }
}

pub(super) fn scanline_time(status: Status) -> SysTime {
    match status.video_mode() {
        VideoMode::Ntsc => SysTime::from_gpu_ntsc_cycles(3413),
        VideoMode::Pal => SysTime::from_gpu_pal_cycles(3405),
    }
}

pub(super) fn scanline_count(status: Status) -> u16 {
    if status.vertical_interlace() {
        let mut count = match status.video_mode() {
            VideoMode::Ntsc => 263,
            VideoMode::Pal => 313,
        };
        if status.interlace_field() == InterlaceField::Bottom {
            count -= 1;
        }
        count
    } else {
        match status.video_mode() {
            VideoMode::Ntsc => 263,
            VideoMode::Pal => 314,
        }
    }
}

/// The current state of the GPU.
#[derive(Debug)]
pub enum State {
    /// The GPU is not doing anything, and is simply waiting for the next command to come in.
    Idle,
    /// The GPU is currently drawing and can therefore not execute the majority if commands until
    /// it's done drawing.
    Drawing,
    /// The GPU is doing a transfer from DRAM to VRAM.
    VramStore(MemTransfer),
    /// The GPU is doing a transfer from VRAM to DRAM.
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

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            State::Idle => f.write_str("idle"),
            State::Drawing => f.write_str("drawing"),
            State::VramStore(tran) => write!(f, "vram store - {tran}"),
            State::VramLoad(tran) => write!(f, "vram load - {tran}"),
        }
    }
}

impl dma::Channel for Gpu {
    fn dma_store(&mut self, schedule: &mut Schedule, val: u32) {
        self.gp0_store(schedule, val);
    }

    fn dma_load(&mut self, _: &mut Schedule, _: (u16, u32)) -> u32 {
        if self.status.dma_direction() != DmaDir::VramToCpu {
            warn!("invalid DMA load from GPU");
            u32::MAX
        } else {
            self.gpu_read()
        }
    }

    fn dma_ready(&self, dir: dma::Direction) -> bool {
        match dir {
            dma::Direction::ToRam => true,
            dma::Direction::ToPort => self.dma_block_ready(),
        }
    }
}

impl BusMap for Gpu {
    const BUS_BEGIN: u32 = 0x1f801810;
    const BUS_END: u32 = Self::BUS_BEGIN + 8 - 1;
}
