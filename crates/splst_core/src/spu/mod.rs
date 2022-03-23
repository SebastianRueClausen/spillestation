use splst_util::{Bit, BitSet};
use crate::bus::{MemUnit, MemUnitKind, BusMap};
use crate::schedule::{Schedule, Event};
use crate::cpu::Irq;
use crate::SysTime;

use std::ops::{Index, IndexMut};

pub struct Spu {
    regs: Regs,
    voices: [Voice; 24],
    last_run: SysTime,
    active_irq: bool,
    ram: [u16; Self::RAM_SIZE],
}

impl Spu {
    const RAM_SIZE: usize = 256 * 1024;

    pub fn new() -> Self {
        Self {
            regs: Regs::default(),
            voices: Default::default(),
            last_run: SysTime::ZERO,
            active_irq: false,
            ram: [0x0; Self::RAM_SIZE],
        }
    }

    pub fn store<T: MemUnit>(&mut self, schedule: &mut Schedule, addr: u32, val: u32) {
        match T::KIND {
            MemUnitKind::Byte => unreachable!("byte store to SPU"),
            MemUnitKind::HalfWord => self.reg_store(schedule, addr, val as u16),
            MemUnitKind::Word => {
                let (lo, hi) = (val as u16, val.bit_range(16, 31) as u16);
                self.reg_store(schedule, addr, lo);
                self.reg_store(schedule, addr | 2, hi);
            }
        }
    }

    pub fn load<T: MemUnit>(&mut self, addr: u32) -> u32 {
        match T::KIND {
            MemUnitKind::Byte => unreachable!("byte load from SPU"),
            MemUnitKind::HalfWord => self.reg_load(addr) as u32,
            MemUnitKind::Word => {
                let lo = self.reg_load(addr) as u32;
                let hi = self.reg_load(addr | 2) as u32;
                lo | (hi << 16)
            }
        }
    }

    /// Store to register.
    fn reg_store(&mut self, schedule: &mut Schedule, addr: u32, val: u16) {
        let reg = addr as usize / 2;

        self.regs[reg] = val;

        match reg {
            // irq_address or transfer_addr.
            210 | 211 => {
                self.maybe_trigger_irq(schedule, self.regs.trans_addr);
            }
            // control register.
            213 => {
                if self.regs.control.irq_enabled() {
                    self.maybe_trigger_irq(schedule, self.regs.trans_addr);
                } else {
                    self.active_irq = false;
                }
            }
            _ => (),
        }
    }

    /// Load from register.
    fn reg_load(&mut self, addr: u32) -> u16 {
        self.regs[addr as usize / 2]
    }

    fn maybe_trigger_irq(&mut self, schedule: &mut Schedule, addr: u16) {
        if self.regs.control.irq_enabled() && addr == self.regs.irq_addr {
            schedule.schedule_now(Event::IrqTrigger(Irq::Spu));
            self.active_irq = true;
        }
    }

    fn update_status(&mut self) {
        // Bit 0..5 of the status register is updated to be the same as bit 0..5 of the control
        // register. Bit 6 of the status represent's if there is an active interrupt which hasn't
        // been acknowledged yet.
        self.regs.status.0 = self.regs.status.0
            .set_bit_range(0, 5, self.regs.control.0)
            .set_bit(6, self.active_irq);

        // TODO: Mednafen does something weird with the transfer control register.
    }

    fn run(&mut self, schedule: &mut Schedule) {
        self.update_status();
    }
}

fn run_voice(voice: &mut Voice) {
    
}

fn ram_idx(val: u16) -> usize {
    (usize::from(val) << 2).bit_range(0, 17)
}

#[repr(C)]
#[derive(Default)]
struct Regs {
    voice_regs: [VoiceRegs; 24],
    /// Main volume left.
    left_main_vol: i16,
    /// Main volume right.
    right_main_vol: i16,
    /// Left reverb volume.
    left_reverb_vol: i16,
    /// Right reverb volume.
    right_reverb_vol: i16,
    /// Bit 0..23 represents if the each of the 24 voices should start attack/decay/sustain.
    voice_on: u32,
    /// Bit 0..23 represents if the each of the 24 voices should start release.
    voice_off: u32,
    /// Voice 0..23 pitch modulation enabled flags.
    voice_pitch_mod_on: u32,
    /// Voice 0..23 noise enabled flags.
    voice_noise_on: u32,
    /// Voice 0..23 reverb enabled flags.
    voice_reverb_on: u32,
    /// Voice 0..23 enabled flags.
    voice_status: u32,
    _unkown1: u16,
    /// Reverb base address.
    reverb_base_addr: u16,
    /// Interrupt address.
    irq_addr: u16,
    /// Transfer start address.
    trans_addr: u16,
    /// Transfer FIFO.
    trans_fifo: u16,
    /// Control register.
    control: ControlReg,
    /// Transfer control.
    trans_control: u16, 
    /// Status register.
    status: StatusReg,
    /// Left CD volume.
    left_cd_vol: i16,
    /// Right CD volume.
    right_cd_vol: i16,
    /// Extern volume left.
    left_ext_vol: i16,
    /// Extern volume right.
    right_ext_vol: i16,
    /// Current main volume left.
    left_curr_main_vol: i16,
    /// Current main volume right.
    right_curr_main_vol: i16,
    _unknown2: u16,
    /// Reverb registers.
    reverb_regs: ReverbRegs,
}

impl Index<usize> for Regs {
    type Output = u16;

    fn index(&self, idx: usize) -> &Self::Output {
        const SIZE: usize = std::mem::size_of::<Regs>() / std::mem::size_of::<u16>();
        unsafe {
            &std::slice::from_raw_parts(self as *const Self as *const u16, SIZE)[idx]  
        }
    }
}

impl IndexMut<usize> for Regs {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        const SIZE: usize = std::mem::size_of::<Regs>() / std::mem::size_of::<u16>();
        unsafe {
            &mut std::slice::from_raw_parts_mut(self as *mut Self as *mut u16, SIZE)[idx]  
        }
    }
}

/// Volume register.
#[derive(Default, Clone, Copy)]
struct VolReg(u16);

impl VolReg {
    #[allow(dead_code)]
    fn sweep_mode(self) -> bool {
        self.0.bit(15)
    }

    /// Sweep exponential.
    #[allow(dead_code)]
    fn sweep_exp(self) -> bool {
        self.0.bit(14)
    }

    #[allow(dead_code)]
    fn sweep_dir_decrease(self) -> bool {
        self.0.bit(13)
    }

    /// Sweep phase negative.
    #[allow(dead_code)]
    fn sweep_phase_neg(self) -> bool {
        self.0.bit(13)
    }

    #[allow(dead_code)]
    fn sweep_rate(self) -> u16 {
        self.0.bit_range(0, 6)
    }

    #[allow(dead_code)]
    fn fixed_vol(self) -> i16 {
        self.0.bit_range(0, 14) as i16 / 2
    }
}

/// Attack decay sustain release register.
#[derive(Default, Clone, Copy)]
struct AdsrReg(u32);

impl AdsrReg {
    #[allow(dead_code)]
    fn sustain_lvl(self) -> u32 {
        (self.0.bit_range(0, 3) + 1) * 0x800
    }

    #[allow(dead_code)]
    fn decay_shift(self) -> u32 {
        self.0.bit_range(4, 7)
    }

    #[allow(dead_code)]
    fn attack_step(self) -> u32 {
        self.0.bit_range(8, 9)
    }

    #[allow(dead_code)]
    fn attack_shift(self) -> u32 {
        self.0.bit_range(10, 14)
    }

    #[allow(dead_code)]
    fn attack_mode(self) -> ScalingMode {
        match self.0.bit(15) {
            true => ScalingMode::Exponential,
            false => ScalingMode::Linear,
        }
    }

    #[allow(dead_code)]
    fn release_shift(self) -> u32 {
        self.0.bit_range(16, 20)
    }

    #[allow(dead_code)]
    fn release_mode(self) -> ScalingMode {
        match self.0.bit(21) {
            true => ScalingMode::Exponential,
            false => ScalingMode::Linear,
        }
    }

    #[allow(dead_code)]
    fn sustain_step(self) -> u32 {
        self.0.bit_range(22, 23)
    }

    #[allow(dead_code)]
    fn sustain_shift(self) -> u32 {
        self.0.bit_range(24, 28)
    }

    #[allow(dead_code)]
    fn sustain_direction(self) -> Direction {
        match self.0.bit(30) {
            true => Direction::Increase,
            false => Direction::Decrease,
        }
    }

    #[allow(dead_code)]
    fn sustain_mode(self) -> ScalingMode {
        match self.0.bit(31) {
            true => ScalingMode::Exponential,
            false => ScalingMode::Linear,
        }
    }
}

enum AdsrPhase {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

impl Default for AdsrPhase {
    fn default() -> Self {
        AdsrPhase::Off
    }
}

#[repr(C)]
#[derive(Default)]
struct VoiceRegs {
    vol_left: VolReg,
    vol_right: VolReg,
    adpcm_sample_rate: u16,
    adpcm_start_addr: u16,
    adsr: AdsrReg,
    adsr_vol: i16,
    adpcm_repeat_addr: u16,
}

/// Adaptive differential pulse-code modulation flags.
#[derive(Default, Clone, Copy)]
struct AdpcmBlockFlags(u8);

impl AdpcmBlockFlags {
    #[allow(dead_code)]
    fn end(self) -> bool {
        self.0.bit(0)
    }

    #[allow(dead_code)]
    fn repeat(self) -> bool {
        self.0.bit(1)
    }

    #[allow(dead_code)]
    fn start(self) -> bool {
        self.0.bit(2)
    }
}

#[derive(Default)]
struct VolEnvelope {
    counter: i16,
    rate: u8,
    direction: Direction,
    mode: ScalingMode,
}

#[derive(Default)]
struct VolSweep {
    envelope: VolEnvelope,
    active: bool,
    lvl: i16,
}

#[derive(Default)]
struct Voice {
    addr: u16,
    counter: u16,
    adpcm_block_flags: AdpcmBlockFlags,

    left_vol: VolSweep,
    right_vol: VolSweep,

    adsr_envelope: VolEnvelope,
    adsr_phase: AdsrPhase,
    adsr_target: i16,
}

/// Reverb registers.
#[repr(C)]
#[derive(Default)]
struct ReverbRegs {
    /// Output volume left.
    left_out_vol: i16,
    /// Output volume right.
    right_out_vol: i16,
    /// Start address in SPU RAM.
    base_addr: u16,
    /// All pass filter offset 1.
    apf_off_1: u16,
    /// All pass filter offset 2.
    apf_off_2: u16,
    /// Reflection volume 1.
    reflection_vol_1: i16,
    /// Comb volume 1.
    comb_vol_1: i16,
    /// Comb volume 2.
    comb_vol_2: i16,
    /// Comb volume 3.
    comb_vol_3: i16,
    /// Comb volume 4.
    comb_vol_4: i16,
    /// Reflection volumn 2.
    reflection_vol_2: i16,
    /// All pass filter volume 1.
    apf_vol_1: i16,
    /// All pass filter volume 2.
    apf_vol_2: i16,
    /// Same side reflection addr 1 left.
    left_ss_ref_addr_1: u16,
    /// Same side reflection addr 1 right.
    right_ss_ref_addr_1: u16,
    /// Left comb address 1.
    left_comb_addr_1: u16,
    /// Right comb address 1.
    right_comb_addr_1: u16,
    /// Left comb address 2.
    left_comb_addr_2: u16,
    /// Right comb address 2.
    right_comb_addr_2: u16,
    /// Same side reflection addr 2 left.
    left_ss_ref_addr_2: u16,
    /// Same side reflection addr 2 left.
    right_ss_ref_addr_2: u16,
    /// Different side reflection addr 1 left.
    left_ds_ref_addr_1: u16,
    /// Different side reflection addr 1 right.
    right_ds_ref_addr_1: u16,
    /// Left comb address 3.
    left_comb_addr_3: u16,
    /// Right comb address 3.
    right_comb_addr_3: u16,
    /// Left comb address 4.
    left_comb_addr_4: u16,
    /// Right comb address 4.
    right_comb_addr_4: u16,
    /// Different side reflection addr 2 left.
    left_ds_ref_addr_2: u16,
    /// Different side reflection addr 2 right.
    right_ds_ref_addr_2: u16,
    /// Left all pass filter address 1.
    left_apf_addr_1: u16,
    /// Right all pass filter address 1.
    right_apf_addr_1: u16,
    /// Left all pass filter address 2.
    left_apf_addr_2: u16,
    /// Right all pass filter address 2.
    right_apf_addr_2: u16,
    /// Input value left.
    left_in_vol: i16,
    /// Input value right.
    right_in_vol: i16,
}

#[derive(Clone, Copy)]
enum TransferMode {
    Stop = 0,
    ManualWrite = 1,
    DmaWrite = 2,
    DmaRead = 3,
}

#[derive(Default, Clone, Copy)]
struct ControlReg(u16);

impl ControlReg {
    #[allow(dead_code)]
    fn cd_audio_enabled(self) -> bool {
        self.0.bit(0)
    }

    #[allow(dead_code)]
    fn external_audio_enabled(self) -> bool {
        self.0.bit(1)
    }

    #[allow(dead_code)]
    fn cd_audio_reverb(self) -> bool {
        self.0.bit(2)
    }

    #[allow(dead_code)]
    fn external_audio_reverb(self) -> bool {
        self.0.bit(3)
    }

    #[allow(dead_code)]
    fn transfer_mode(self) -> TransferMode {
        match self.0.bit_range(4, 5) {
            0 => TransferMode::Stop,
            1 => TransferMode::ManualWrite,
            2 => TransferMode::DmaWrite,
            3 => TransferMode::DmaRead,
            _ => unreachable!(),
        }
    }

    fn irq_enabled(self) -> bool {
        // From Nocash. Mednafen doesn't seem to check that bit 15 is set.
        self.0.bit(6) && self.spu_enabled()
    }

    #[allow(dead_code)]
    fn master_reverb_enabled(self) -> bool {
        self.0.bit(7)
    }

    #[allow(dead_code)]
    fn noice_freq_step(self) -> u32 {
        self.0.bit_range(8, 9) as u32
    }

    #[allow(dead_code)]
    fn noice_freq_shift(self) -> u32 {
        self.0.bit_range(10, 13) as u32
    }

    #[allow(dead_code)]
    fn spu_unmuted(self) -> bool {
        self.0.bit(14)
    }

    fn spu_enabled(self) -> bool {
        self.0.bit(15)
    }
}

/// Capture buffer sector.
#[derive(Clone, Copy)]
enum CaptureBufSec {
    First,
    Second,
}

#[derive(Default, Clone, Copy)]
struct StatusReg(u16);

impl StatusReg {
    #[allow(dead_code)]
    fn irq_flag(self) -> bool {
        self.0.bit(6)
    }

    /// DMA read/write request.
    #[allow(dead_code)]
    fn dma_rw_req(self) -> bool {
        self.0.bit(7)
    }

    /// DMA write request.
    #[allow(dead_code)]
    fn dma_write_req(self) -> bool {
        self.0.bit(8)
    }

    /// DMA read request.
    #[allow(dead_code)]
    fn dma_read_req(self) -> bool {
        self.0.bit(9)
    }

    #[allow(dead_code)]
    fn dma_busy(self) -> bool {
        self.0.bit(10)
    }

    /// Which capture buffer half the SPU is writing to.
    #[allow(dead_code)]
    fn writing_to(self) -> CaptureBufSec {
        match self.0.bit(11) {
            true => CaptureBufSec::First,
            false => CaptureBufSec::Second,
        }
    }
}

enum ScalingMode {
    Linear,
    Exponential,
}

impl Default for ScalingMode {
    fn default() -> Self {
        ScalingMode::Linear
    }
}

enum Direction {
    Increase,
    Decrease,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Increase
    }
}

impl BusMap for Spu {
    const BUS_BEGIN: u32 = 0x1f801c00;
    const BUS_END: u32 = Self::BUS_BEGIN + 640 - 1;
}
