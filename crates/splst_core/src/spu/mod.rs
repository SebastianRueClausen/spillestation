#![allow(dead_code)]

use splst_util::{Bit, BitSet};
use crate::bus::{AddrUnit, AddrUnitWidth, BusMap};
use crate::schedule::{Schedule, Event};
use crate::cpu::Irq;
use crate::cdrom::CdRom;
use crate::{SysTime, AudioOutput};
use crate::fifo::Fifo;

use std::rc::Rc;
use std::cell::RefCell;
use std::ops::{Index, IndexMut};

pub struct Spu {
    regs: Regs,
    voices: [Voice; 24],
    last_run: SysTime,
    active_irq: bool,
    ram: Ram,
    noise_lsfr: u16,
    capture_addr: u16,
    audio_output: Rc<RefCell<dyn AudioOutput>>,
}

impl Spu {
    pub fn new(schedule: &mut Schedule, audio_output: Rc<RefCell<dyn AudioOutput>>) -> Self {
        schedule.schedule_repeat(SysTime::new(0x300), Event::Spu(Self::run_cycle));
        
        Self {
            regs: Regs::default(),
            voices: create_voices(),
            last_run: SysTime::ZERO,
            active_irq: false,
            ram: Ram::default(),
            noise_lsfr: 0,
            capture_addr: 0,
            audio_output,
        }
    }

    pub fn store<T: AddrUnit>(&mut self, schedule: &mut Schedule, addr: u32, val: T) {
        let val = val.as_u32();
        match T::WIDTH {
            AddrUnitWidth::Byte => unreachable!("byte store to SPU"),
            AddrUnitWidth::HalfWord => self.reg_store(schedule, addr, val as u16),
            AddrUnitWidth::Word => {
                let (lo, hi) = (val as u16, val.bit_range(16, 31) as u16);
                self.reg_store(schedule, addr, lo);
                self.reg_store(schedule, addr | 2, hi);
            }
        }
    }

    pub fn load<T: AddrUnit>(&self, addr: u32) -> T {
        let val = match T::WIDTH {
            AddrUnitWidth::Byte => unreachable!("byte load from SPU"),
            AddrUnitWidth::HalfWord => self.reg_load(addr) as u32,
            AddrUnitWidth::Word => {
                let lo = self.reg_load(addr) as u32;
                let hi = self.reg_load(addr | 2) as u32;
                lo | (hi << 16)
            }
        };

        T::from_u32(val)
    }

    /// Store to register.
    fn reg_store(&mut self, schedule: &mut Schedule, addr: u32, val: u16) {
        match addr as usize / 2 {
            // Voice registers.
            reg @ 0..=191 => {
                let voice_idx: usize = (reg / 8).into();
                let voice_reg: usize = (reg % 8).into();
                
                self.voices[voice_idx].regs[voice_reg] = val;

                match reg % 8 {
                    7 => {
                        // TODO: Maybe the voice has to be on.
                        self.voices[voice_idx].ignore_loop_addr = true;
                    }
                    _ => (),
                }
            }
            reg => {
                let reg = reg - 191;

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
        }
    }

    /// Load from register.
    fn reg_load(&self, addr: u32) -> u16 {
        match addr as usize / 2 {
            reg @ 0..=191 => {
                let voice_idx: usize = (reg / 8).into();
                let voice_reg: usize = (reg % 8).into();

                self.voices[voice_idx].regs[voice_reg]
            }
            reg => self.regs[reg - 191],
        }
    }

    fn maybe_trigger_irq(&mut self, schedule: &mut Schedule, addr: u16) {
        if self.regs.control.irq_enabled() && addr == self.regs.irq_addr {
            schedule.trigger(Event::Irq(Irq::Spu));
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

    fn run_cycle(&mut self, schedule: &mut Schedule, cdrom: &mut CdRom) {
        self.update_status();

        let mut sweep_factor = 0;
        let mut irq = false;

        let (mut left, mut right) = self.voices
            .iter_mut()
            .fold((0, 0), |(lmix, rmix), voice| {
                irq |= voice.run_decoder(&mut self.regs, &self.ram);
                let (left, right) = voice.run(
                    self.noise_lsfr,
                    self.capture_addr,
                    &mut sweep_factor,
                    &mut self.regs,
                    &mut self.ram,
                );
                (left + lmix, right + rmix) 
            });
        
        if irq {
            schedule.trigger(Event::Irq(Irq::Spu));
        }

        self.run_noise_cycle();

        // We can clear there now since they are being handled by 'Voice::run'.
        self.regs.voice_on = VoiceFlags::default();
        self.regs.voice_off = VoiceFlags::default();

        if self.regs.control.muted() {
            left = 0;
            right = 0;
        }

        if self.regs.control.cd_audio_enabled() {
            let (cd_l, cd_r) = cdrom.run_audio_cycle(true);
            left += cd_l as i32;
            right += cd_r as i32;
        } else {
            cdrom.run_audio_cycle(false);
        }

        left = self.regs.left_main_vol.apply_vol(left);
        right = self.regs.right_main_vol.apply_vol(right);

        // TODO: Main volume sweep cycle.

        let output = [
            left.clamp(i16::MIN.into(), i16::MAX.into()) as i16,
            right.clamp(i16::MIN.into(), i16::MAX.into()) as i16,
        ];

        self.audio_output.borrow_mut().send_audio(output);
        self.capture_addr = self.capture_addr.wrapping_add(1);
    }

    fn run_noise_cycle(&mut self) {
        // TODO
    }
}

struct Ram([u16; Self::SIZE]);

impl Default for Ram {
    fn default() -> Self {
        Self([0x0; Self::SIZE])
    }
}

impl Ram {
    const SIZE: usize = 256 * 1024;

    fn load(&self, addr: u16) -> u16 {
        let addr: usize = addr.into();
        // The 16 bit addresses stored in the registers are stored in 8 byte units, and since we
        // store the RAM as u16's, the address get's multiplied by 4.
        self.0[addr * 4]
    }

    fn store(&mut self, addr: u16, val: u16) {
        let addr: usize = addr.into();
        self.0[addr * 4] = val;
    }
}

/// A single flag for each voice.
#[derive(Default, Clone, Copy)]
struct VoiceFlags(u32);

impl VoiceFlags {
    fn get(self, idx: VoiceIndex) -> bool {
        self.0.bit(idx.0 as usize)
    }
    
    fn set(&mut self, idx: VoiceIndex, enabled: bool) {
        self.0.set_bit(idx.0 as usize, enabled);
    }
}

#[repr(C)]
#[derive(Default)]
struct Regs {
    /// Main volume left.
    left_main_vol: VolReg,
    /// Main volume right.
    right_main_vol: VolReg,
    /// Left reverb volume.
    left_reverb_vol: VolReg,
    /// Right reverb volume.
    right_reverb_vol: VolReg,
    /// Bit 0..23 represents if the each of the 24 voices should start attack/decay/sustain.
    voice_on: VoiceFlags,
    /// Bit 0..23 represents if the each of the 24 voices should start release.
    voice_off: VoiceFlags,
    /// Voice 0..23 pitch modulation enabled flags.
    voice_pitch_mod_on: VoiceFlags,
    /// Voice 0..23 noise enabled flags.
    voice_noise_on: VoiceFlags,
    /// Voice 0..23 reverb enabled flags.
    voice_reverb_on: VoiceFlags,
    /// Voice 0..23 enabled flags.
    voice_status: VoiceFlags,
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

impl Regs {
    fn irq_for_addr(&self, addr: u16) -> bool {
        self.control.irq_enabled() && addr == self.irq_addr
    }
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

enum VolMode {
   Fixed,
   Sweep,
}

/// Volume register.
#[derive(Default, Clone, Copy)]
struct VolReg(u16);

impl VolReg {
    fn mode(self) -> VolMode {
        match self.0.bit(15) {
            false => VolMode::Fixed,
            true => VolMode::Sweep,
        }
    }

    fn apply_vol(self, sample: i32) -> i32 {
        match self.mode() {
            // TODO: Implement sweep mode.
            VolMode::Fixed | VolMode::Sweep => {
                let vol = self.0.bit_range(0, 14) as i32 * 2;
                (sample * vol) >> 15
            }
        }
    }
}

/// Attack decay sustain release register.
#[derive(Default, Clone, Copy)]
struct AdsrReg(u32);

impl AdsrReg {
    fn sustain_lvl(self) -> i16 {
        ((self.0.bit_range(0, 3) + 1) * 0x800) as i16
    }

    fn decay_rate(self) -> u8 {
        (self.0.bit_range(4, 7) as u8) << 2
    }

    fn attack_rate(self) -> u8 {
        self.0.bit_range(8, 14) as u8
    }

    #[allow(dead_code)]
    fn attack_mode(self) -> ScalingMode {
        match self.0.bit(15) {
            true => ScalingMode::Exponential,
            false => ScalingMode::Linear,
        }
    }

    fn release_rate(self) -> u8 {
        (self.0.bit_range(16, 20) as u8) << 2
    }

    fn release_mode(self) -> ScalingMode {
        match self.0.bit(21) {
            true => ScalingMode::Exponential,
            false => ScalingMode::Linear,
        }
    }

    fn sustain_rate(self) -> u8 {
        self.0.bit_range(22, 28) as u8
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

    fn run_cycle(&mut self) {
        
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AdsrPhase {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

impl AdsrPhase {
    fn next(self) -> AdsrPhase {
        match self {
            AdsrPhase::Attack => AdsrPhase::Decay,
            AdsrPhase::Decay | AdsrPhase::Sustain => AdsrPhase::Sustain,
            AdsrPhase::Release | AdsrPhase::Off => AdsrPhase::Off,
        }
    }
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

impl Index<usize> for VoiceRegs {
    type Output = u16;

    fn index(&self, idx: usize) -> &Self::Output {
        const SIZE: usize = std::mem::size_of::<VoiceRegs>() / std::mem::size_of::<u16>();
        unsafe {
            &std::slice::from_raw_parts(self as *const Self as *const u16, SIZE)[idx]  
        }
    }
}

impl IndexMut<usize> for VoiceRegs {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        const SIZE: usize = std::mem::size_of::<VoiceRegs>() / std::mem::size_of::<u16>();
        unsafe {
            &mut std::slice::from_raw_parts_mut(self as *mut Self as *mut u16, SIZE)[idx]  
        }
    }
}


/// Adaptive differential pulse-code modulation flags.
#[derive(Default, Clone, Copy)]
struct AdpcmBlockFlags(u16);

impl AdpcmBlockFlags {
    fn shift(self) -> u16 {
        self.0.bit_range(0, 3)
    }

    fn weights(self) -> (i32, i32) {
        const WEIGHTS: [(i32, i32); 16] = [
            (0, 0),
            (60, 0),
            (115, -52),
            (98, -55),
            (122, -60),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
        ];

        let idx = self.0.bit_range(4, 7);

        WEIGHTS[idx as usize]
    }

    fn end(self) -> bool {
        self.0.bit(8)
    }

    fn repeat(self) -> bool {
        self.0.bit(9)
    }

    #[allow(dead_code)]
    fn start(self) -> bool {
        self.0.bit(10)
    }
}

#[derive(Default)]
struct VolEnvelope {
    counter: i32,
    rate: u8,
    direction: Direction,
    mode: ScalingMode,
}

impl VolEnvelope {
    fn new(rate: u8, direction: Direction, mode: ScalingMode) -> Self {
        let (counter, _) = ADSR_TABLE[direction as usize][rate as usize];
        Self {
            counter,
            rate,
            direction,
            mode,
        }
    }

    fn run_cycle(&mut self, level: i16) -> i16 {
        self.counter -= 1;
        if self.counter > 0 {
            level
        } else {
            let (ticks, step) = ADSR_TABLE[self.direction as usize][self.rate as usize];
                    
            let step = match self.mode {
                ScalingMode::Linear => step,
                ScalingMode::Exponential => match self.direction {
                    Direction::Decrease => (step * level as i32) >> 15,
                    Direction::Increase => {
                        if level >= 0x6000 {
                            match self.rate {
                                00..=39 => step >> 2,
                                40..=44 => {
                                    self.counter >>= 2;
                                    step
                                }
                                _ => {
                                    self.counter >>= 1;
                                    step >> 1
                                }
                            }
                        } else {
                            step
                        }
                    }
                }
            };

            (level as i32 + step).clamp(0, 0x7fff) as i16
        }
    }
}

#[derive(Default)]
struct VolSweep {
    envelope: VolEnvelope,
    active: bool,
    lvl: i16,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct VoiceIndex(u8);

struct Voice {
    idx: VoiceIndex,

    addr: u16,
    
    counter: u16,
    adpcm_block_flags: AdpcmBlockFlags,

    left_vol: VolSweep,
    right_vol: VolSweep,

    adsr_envelope: VolEnvelope,
    adsr_phase: AdsrPhase,
    adsr_target: i16,

    prev_samples: [i16; 2],
    decode_fifo: Fifo<i16, 16>,

    ignore_loop_addr: bool,
    start_delay: u8,

    regs: VoiceRegs,
}

impl Voice {
    fn new(idx: VoiceIndex) -> Self {
        Self {
            idx,
            addr: 0,
            counter: 0,
            adpcm_block_flags: AdpcmBlockFlags::default(),
            left_vol: VolSweep::default(),
            right_vol: VolSweep::default(),
            adsr_envelope: VolEnvelope::default(),
            adsr_phase: AdsrPhase::default(),
            adsr_target: 0,
            prev_samples: [0; 2],
            decode_fifo: Fifo::default(),
            ignore_loop_addr: false,
            start_delay: 0,
            regs: VoiceRegs::default(),
        }        
    }
    
    fn run(
        &mut self,
        noise_lsfr: u16,
        capture_addr: u16,
        sweep_factor: &mut i32,
        regs: &mut Regs,
        ram: &mut Ram,
    ) -> (i32, i32) {
        let sample: i32 = if regs.voice_noise_on.get(self.idx) {
            (noise_lsfr as i16).into()
        } else {
            self.next_decoded_sample() 
        };

        let sample = (self.regs.adsr_vol as i32 * sample) >> 15;

        match self.idx {
            VoiceIndex(1) => ram.store(256 | capture_addr, sample as u16),
            VoiceIndex(3) => ram.store(384 | capture_addr, sample as u16),
            _ => (),
        }

        let (left, right) = (
            self.regs.vol_left.apply_vol(sample),
            self.regs.vol_right.apply_vol(sample),
        );

        // TODO: Run sweep cycle.
        
        if self.start_delay != 0 {
            self.start_delay -= 1;
        } else {
            self.adsr_envelope_cycle(); 

            let sample_rate: u32 = self.regs.adpcm_sample_rate.into();

            let step = if self.idx != VoiceIndex(0) && regs.voice_pitch_mod_on.get(self.idx) {
                ((sample_rate as i32 * *sweep_factor) >> 15) as u32
            } else {
                sample_rate as u32
            };

            let step = step.min(0x3fff) as u16;
            self.consume_samples(step);
        }

        if regs.voice_off.get(self.idx) {
            self.start_release_phase();
        }

        if regs.voice_on.get(self.idx) {
            self.reset();
            regs.voice_status.set(self.idx, false);
        }

        if !regs.control.spu_enabled() {
            self.start_release_phase();
            self.mute();
        }

        *sweep_factor = sample;

        (left, right)
    }

    /// Returns true if an interrupt should be triggered.
    fn run_decoder(&mut self, regs: &mut Regs, ram: &Ram) -> bool {
        let mut irq = false;
        
        if self.decode_fifo.len() > 10 {
            irq |= regs.irq_for_addr(self.addr.wrapping_sub(1));
        } else {
            if self.addr % 8 == 0 {
                if self.adpcm_block_flags.end() {
                    // Do new loop.
                    self.addr = self.regs.adpcm_repeat_addr;

                    // Set voice status flag since the loop is done.
                    regs.voice_status.set(self.idx, true);
                  
                    // Mednafen doesnt release when in noise mode.
                    if !regs.voice_noise_on.get(self.idx) {
                        if !self.adpcm_block_flags.repeat() {
                            self.adsr_phase = AdsrPhase::Release;    
                            self.regs.adsr_vol = 0;
                        }
                    }
                }

                irq |= regs.control.irq_enabled() && regs.irq_for_addr(self.addr);

                let header = ram.load(self.addr);

                self.addr = self.addr.wrapping_add(1);
                self.adpcm_block_flags = AdpcmBlockFlags(header);

                if self.adpcm_block_flags.start() && !self.ignore_loop_addr {
                    self.regs.adpcm_repeat_addr = self.addr;
                }
            } else {
                irq |= regs.control.irq_enabled() && regs.irq_for_addr(self.addr);
            };
            
            let encoded = ram.load(self.addr);

            self.addr = self.addr.wrapping_add(1);
            self.decode(encoded); 
        }
        
        irq
    }

    fn start_release_phase(&mut self) {
       self.adsr_phase = AdsrPhase::Release;
       self.adsr_envelope.counter = 0;
    }

    fn start_attack_phase(&mut self) {
       self.adsr_phase = AdsrPhase::Attack;
       self.adsr_envelope.counter = 0;

       self.mute();
    }

    fn reset(&mut self) {
        self.start_attack_phase();

        self.counter = 0;
        self.addr = self.regs.adpcm_start_addr;
        self.adpcm_block_flags = AdpcmBlockFlags(0);
        self.prev_samples = [0; 2];
        self.decode_fifo.clear();
        self.start_delay = 4;
        self.ignore_loop_addr = false;
    }

    fn mute(&mut self) {
        self.regs.adsr_vol = 0;
    }

    fn adsr_envelope_cycle(&mut self) {
        self.regs.adsr_vol = self
            .adsr_envelope
            .run_cycle(self.regs.adsr_vol);

        if self.adsr_phase != AdsrPhase::Sustain {
            let reached = match self.adsr_envelope.direction {
                Direction::Decrease => self.regs.adsr_vol <= self.adsr_target,
                Direction::Increase => self.regs.adsr_vol >= self.adsr_target,
            };
            if reached {
                self.adsr_phase = self.adsr_phase.next();
                self.update_adsr_envelope();
            }
        }
    }

    fn update_adsr_envelope(&mut self) {
        match self.adsr_phase {
            AdsrPhase::Off => {
                self.adsr_target = 0;
                self.adsr_envelope = VolEnvelope::new(
                    0,
                    Direction::Increase,
                    ScalingMode::Linear
                );
            }
            AdsrPhase::Attack => {
                self.adsr_target = 32767;
                self.adsr_envelope = VolEnvelope::new(
                    self.regs.adsr.attack_rate(),
                    Direction::Increase,
                    self.regs.adsr.attack_mode(),
                );
            }
            AdsrPhase::Decay => {
                self.adsr_target = self.regs.adsr.sustain_lvl().min(0x7fff);
                self.adsr_envelope = VolEnvelope::new(
                    self.regs.adsr.decay_rate(),
                    Direction::Decrease,
                    ScalingMode::Exponential,
                );
            }
            AdsrPhase::Sustain => {
                self.adsr_target = 0;
                self.adsr_envelope = VolEnvelope::new(
                    self.regs.adsr.sustain_rate(),
                    self.regs.adsr.sustain_direction(),
                    self.regs.adsr.sustain_mode(),
                );
            }
            AdsrPhase::Release => {
                self.adsr_target = 0;
                self.adsr_envelope = VolEnvelope::new(
                    self.regs.adsr.release_rate(),
                    Direction::Decrease,
                    self.regs.adsr.release_mode(),
                );
            }
        }
    }
    
    fn decode(&mut self, val: u16) {
        let (w1, w2) = self.adpcm_block_flags.weights();
        let shift = self.adpcm_block_flags.shift();

        for (lo, hi) in [(12, 15), (8, 11), (4, 7), (0, 3)] {
            let sample = val.bit_range(lo, hi) as i16;
            let sample = (sample >> shift) as i32;

            let s1: i32 = self.prev_samples[0].into();
            let s2: i32 = self.prev_samples[1].into();

            let sample = sample + (s1 * w1) >> 6;
            let sample = sample + (s2 * w2) >> 6;

            let sample = sample.clamp(i16::MIN.into(), i16::MAX.into()) as i16;
            self.decode_fifo.push(sample);
            
            self.prev_samples[1] = self.prev_samples[0];
            self.prev_samples[0] = sample;
        }
    }

    fn next_decoded_sample(&self) -> i32 {
        let phase: usize = (self.counter >> 4).into(); 
        let samples: [i32; 4] = [
            self.decode_fifo[0].into(),
            self.decode_fifo[1].into(),
            self.decode_fifo[2].into(),
            self.decode_fifo[3].into(),
        ];
        let coeffs = FIR_COEFF[phase as usize];
        let s = samples.into_iter().zip(coeffs.into_iter()).fold(0, |r, (s, c)| {
            r + s * c as i32
        });
        s >> 15
    }

    fn consume_samples(&mut self, step: u16) {
        let step = self.counter + step;
        self.counter = self.counter.bit_range(0, 11);
        self.decode_fifo.pop_n((step >> 12).into()); 
    }
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
    fn muted(self) -> bool {
        !self.0.bit(14)
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

#[derive(Clone, Copy)]
enum Direction {
    Increase,
    Decrease,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Increase
    }
}

const FIR_COEFF: [[i16; 4]; 256] = [
    [0x12c7, 0x59b3, 0x1307, -1],
    [0x1288, 0x59b2, 0x1347, -1],
    [0x1249, 0x59b0, 0x1388, -1],
    [0x120b, 0x59ad, 0x13c9, -1],
    [0x11cd, 0x59a9, 0x140b, -1],
    [0x118f, 0x59a4, 0x144d, -1],
    [0x1153, 0x599e, 0x1490, -1],
    [0x1116, 0x5997, 0x14d4, -1],
    [0x10db, 0x598f, 0x1517, -1],
    [0x109f, 0x5986, 0x155c, -1],
    [0x1065, 0x597c, 0x15a0, -1],
    [0x102a, 0x5971, 0x15e6, -1],
    [0x0ff1, 0x5965, 0x162c, -1],
    [0x0fb7, 0x5958, 0x1672, -1],
    [0x0f7f, 0x5949, 0x16b9, -1],
    [0x0f46, 0x593a, 0x1700, -1],
    [0x0f0f, 0x592a, 0x1747, 0x0000],
    [0x0ed7, 0x5919, 0x1790, 0x0000],
    [0x0ea1, 0x5907, 0x17d8, 0x0000],
    [0x0e6b, 0x58f4, 0x1821, 0x0000],
    [0x0e35, 0x58e0, 0x186b, 0x0000],
    [0x0e00, 0x58cb, 0x18b5, 0x0000],
    [0x0dcb, 0x58b5, 0x1900, 0x0000],
    [0x0d97, 0x589e, 0x194b, 0x0001],
    [0x0d63, 0x5886, 0x1996, 0x0001],
    [0x0d30, 0x586d, 0x19e2, 0x0001],
    [0x0cfd, 0x5853, 0x1a2e, 0x0001],
    [0x0ccb, 0x5838, 0x1a7b, 0x0002],
    [0x0c99, 0x581c, 0x1ac8, 0x0002],
    [0x0c68, 0x57ff, 0x1b16, 0x0002],
    [0x0c38, 0x57e2, 0x1b64, 0x0003],
    [0x0c07, 0x57c3, 0x1bb3, 0x0003],
    [0x0bd8, 0x57a3, 0x1c02, 0x0003],
    [0x0ba9, 0x5782, 0x1c51, 0x0004],
    [0x0b7a, 0x5761, 0x1ca1, 0x0004],
    [0x0b4c, 0x573e, 0x1cf1, 0x0005],
    [0x0b1e, 0x571b, 0x1d42, 0x0005],
    [0x0af1, 0x56f6, 0x1d93, 0x0006],
    [0x0ac4, 0x56d1, 0x1de5, 0x0007],
    [0x0a98, 0x56ab, 0x1e37, 0x0007],
    [0x0a6c, 0x5684, 0x1e89, 0x0008],
    [0x0a40, 0x565b, 0x1edc, 0x0009],
    [0x0a16, 0x5632, 0x1f2f, 0x0009],
    [0x09eb, 0x5609, 0x1f82, 0x000a],
    [0x09c1, 0x55de, 0x1fd6, 0x000b],
    [0x0998, 0x55b2, 0x202a, 0x000c],
    [0x096f, 0x5585, 0x207f, 0x000d],
    [0x0946, 0x5558, 0x20d4, 0x000e],
    [0x091e, 0x5529, 0x2129, 0x000f],
    [0x08f7, 0x54fa, 0x217f, 0x0010],
    [0x08d0, 0x54ca, 0x21d5, 0x0011],
    [0x08a9, 0x5499, 0x222c, 0x0012],
    [0x0883, 0x5467, 0x2282, 0x0013],
    [0x085d, 0x5434, 0x22da, 0x0015],
    [0x0838, 0x5401, 0x2331, 0x0016],
    [0x0813, 0x53cc, 0x2389, 0x0018],
    [0x07ef, 0x5397, 0x23e1, 0x0019],
    [0x07cb, 0x5361, 0x2439, 0x001b],
    [0x07a7, 0x532a, 0x2492, 0x001c],
    [0x0784, 0x52f3, 0x24eb, 0x001e],
    [0x0762, 0x52ba, 0x2545, 0x0020],
    [0x0740, 0x5281, 0x259e, 0x0021],
    [0x071e, 0x5247, 0x25f8, 0x0023],
    [0x06fd, 0x520c, 0x2653, 0x0025],
    [0x06dc, 0x51d0, 0x26ad, 0x0027],
    [0x06bb, 0x5194, 0x2708, 0x0029],
    [0x069b, 0x5156, 0x2763, 0x002c],
    [0x067c, 0x5118, 0x27be, 0x002e],
    [0x065c, 0x50da, 0x281a, 0x0030],
    [0x063e, 0x509a, 0x2876, 0x0033],
    [0x061f, 0x505a, 0x28d2, 0x0035],
    [0x0601, 0x5019, 0x292e, 0x0038],
    [0x05e4, 0x4fd7, 0x298b, 0x003a],
    [0x05c7, 0x4f95, 0x29e7, 0x003d],
    [0x05aa, 0x4f52, 0x2a44, 0x0040],
    [0x058e, 0x4f0e, 0x2aa1, 0x0043],
    [0x0572, 0x4ec9, 0x2aff, 0x0046],
    [0x0556, 0x4e84, 0x2b5c, 0x0049],
    [0x053b, 0x4e3e, 0x2bba, 0x004d],
    [0x0520, 0x4df7, 0x2c18, 0x0050],
    [0x0506, 0x4db0, 0x2c76, 0x0054],
    [0x04ec, 0x4d68, 0x2cd4, 0x0057],
    [0x04d2, 0x4d20, 0x2d33, 0x005b],
    [0x04b9, 0x4cd7, 0x2d91, 0x005f],
    [0x04a0, 0x4c8d, 0x2df0, 0x0063],
    [0x0488, 0x4c42, 0x2e4f, 0x0067],
    [0x0470, 0x4bf7, 0x2eae, 0x006b],
    [0x0458, 0x4bac, 0x2f0d, 0x006f],
    [0x0441, 0x4b5f, 0x2f6c, 0x0074],
    [0x042a, 0x4b13, 0x2fcc, 0x0078],
    [0x0413, 0x4ac5, 0x302b, 0x007d],
    [0x03fc, 0x4a77, 0x308b, 0x0082],
    [0x03e7, 0x4a29, 0x30ea, 0x0087],
    [0x03d1, 0x49d9, 0x314a, 0x008c],
    [0x03bc, 0x498a, 0x31aa, 0x0091],
    [0x03a7, 0x493a, 0x3209, 0x0096],
    [0x0392, 0x48e9, 0x3269, 0x009c],
    [0x037e, 0x4898, 0x32c9, 0x00a1],
    [0x036a, 0x4846, 0x3329, 0x00a7],
    [0x0356, 0x47f4, 0x3389, 0x00ad],
    [0x0343, 0x47a1, 0x33e9, 0x00b3],
    [0x0330, 0x474e, 0x3449, 0x00ba],
    [0x031d, 0x46fa, 0x34a9, 0x00c0],
    [0x030b, 0x46a6, 0x3509, 0x00c7],
    [0x02f9, 0x4651, 0x3569, 0x00cd],
    [0x02e7, 0x45fc, 0x35c9, 0x00d4],
    [0x02d6, 0x45a6, 0x3629, 0x00db],
    [0x02c4, 0x4550, 0x3689, 0x00e3],
    [0x02b4, 0x44fa, 0x36e8, 0x00ea],
    [0x02a3, 0x44a3, 0x3748, 0x00f2],
    [0x0293, 0x444c, 0x37a8, 0x00fa],
    [0x0283, 0x43f4, 0x3807, 0x0101],
    [0x0273, 0x439c, 0x3867, 0x010a],
    [0x0264, 0x4344, 0x38c6, 0x0112],
    [0x0255, 0x42eb, 0x3926, 0x011b],
    [0x0246, 0x4292, 0x3985, 0x0123],
    [0x0237, 0x4239, 0x39e4, 0x012c],
    [0x0229, 0x41df, 0x3a43, 0x0135],
    [0x021b, 0x4185, 0x3aa2, 0x013f],
    [0x020d, 0x412a, 0x3b00, 0x0148],
    [0x0200, 0x40d0, 0x3b5f, 0x0152],
    [0x01f2, 0x4074, 0x3bbd, 0x015c],
    [0x01e5, 0x4019, 0x3c1b, 0x0166],
    [0x01d9, 0x3fbd, 0x3c79, 0x0171],
    [0x01cc, 0x3f62, 0x3cd7, 0x017b],
    [0x01c0, 0x3f05, 0x3d35, 0x0186],
    [0x01b4, 0x3ea9, 0x3d92, 0x0191],
    [0x01a8, 0x3e4c, 0x3def, 0x019c],
    [0x019c, 0x3def, 0x3e4c, 0x01a8],
    [0x0191, 0x3d92, 0x3ea9, 0x01b4],
    [0x0186, 0x3d35, 0x3f05, 0x01c0],
    [0x017b, 0x3cd7, 0x3f62, 0x01cc],
    [0x0171, 0x3c79, 0x3fbd, 0x01d9],
    [0x0166, 0x3c1b, 0x4019, 0x01e5],
    [0x015c, 0x3bbd, 0x4074, 0x01f2],
    [0x0152, 0x3b5f, 0x40d0, 0x0200],
    [0x0148, 0x3b00, 0x412a, 0x020d],
    [0x013f, 0x3aa2, 0x4185, 0x021b],
    [0x0135, 0x3a43, 0x41df, 0x0229],
    [0x012c, 0x39e4, 0x4239, 0x0237],
    [0x0123, 0x3985, 0x4292, 0x0246],
    [0x011b, 0x3926, 0x42eb, 0x0255],
    [0x0112, 0x38c6, 0x4344, 0x0264],
    [0x010a, 0x3867, 0x439c, 0x0273],
    [0x0101, 0x3807, 0x43f4, 0x0283],
    [0x00fa, 0x37a8, 0x444c, 0x0293],
    [0x00f2, 0x3748, 0x44a3, 0x02a3],
    [0x00ea, 0x36e8, 0x44fa, 0x02b4],
    [0x00e3, 0x3689, 0x4550, 0x02c4],
    [0x00db, 0x3629, 0x45a6, 0x02d6],
    [0x00d4, 0x35c9, 0x45fc, 0x02e7],
    [0x00cd, 0x3569, 0x4651, 0x02f9],
    [0x00c7, 0x3509, 0x46a6, 0x030b],
    [0x00c0, 0x34a9, 0x46fa, 0x031d],
    [0x00ba, 0x3449, 0x474e, 0x0330],
    [0x00b3, 0x33e9, 0x47a1, 0x0343],
    [0x00ad, 0x3389, 0x47f4, 0x0356],
    [0x00a7, 0x3329, 0x4846, 0x036a],
    [0x00a1, 0x32c9, 0x4898, 0x037e],
    [0x009c, 0x3269, 0x48e9, 0x0392],
    [0x0096, 0x3209, 0x493a, 0x03a7],
    [0x0091, 0x31aa, 0x498a, 0x03bc],
    [0x008c, 0x314a, 0x49d9, 0x03d1],
    [0x0087, 0x30ea, 0x4a29, 0x03e7],
    [0x0082, 0x308b, 0x4a77, 0x03fc],
    [0x007d, 0x302b, 0x4ac5, 0x0413],
    [0x0078, 0x2fcc, 0x4b13, 0x042a],
    [0x0074, 0x2f6c, 0x4b5f, 0x0441],
    [0x006f, 0x2f0d, 0x4bac, 0x0458],
    [0x006b, 0x2eae, 0x4bf7, 0x0470],
    [0x0067, 0x2e4f, 0x4c42, 0x0488],
    [0x0063, 0x2df0, 0x4c8d, 0x04a0],
    [0x005f, 0x2d91, 0x4cd7, 0x04b9],
    [0x005b, 0x2d33, 0x4d20, 0x04d2],
    [0x0057, 0x2cd4, 0x4d68, 0x04ec],
    [0x0054, 0x2c76, 0x4db0, 0x0506],
    [0x0050, 0x2c18, 0x4df7, 0x0520],
    [0x004d, 0x2bba, 0x4e3e, 0x053b],
    [0x0049, 0x2b5c, 0x4e84, 0x0556],
    [0x0046, 0x2aff, 0x4ec9, 0x0572],
    [0x0043, 0x2aa1, 0x4f0e, 0x058e],
    [0x0040, 0x2a44, 0x4f52, 0x05aa],
    [0x003d, 0x29e7, 0x4f95, 0x05c7],
    [0x003a, 0x298b, 0x4fd7, 0x05e4],
    [0x0038, 0x292e, 0x5019, 0x0601],
    [0x0035, 0x28d2, 0x505a, 0x061f],
    [0x0033, 0x2876, 0x509a, 0x063e],
    [0x0030, 0x281a, 0x50da, 0x065c],
    [0x002e, 0x27be, 0x5118, 0x067c],
    [0x002c, 0x2763, 0x5156, 0x069b],
    [0x0029, 0x2708, 0x5194, 0x06bb],
    [0x0027, 0x26ad, 0x51d0, 0x06dc],
    [0x0025, 0x2653, 0x520c, 0x06fd],
    [0x0023, 0x25f8, 0x5247, 0x071e],
    [0x0021, 0x259e, 0x5281, 0x0740],
    [0x0020, 0x2545, 0x52ba, 0x0762],
    [0x001e, 0x24eb, 0x52f3, 0x0784],
    [0x001c, 0x2492, 0x532a, 0x07a7],
    [0x001b, 0x2439, 0x5361, 0x07cb],
    [0x0019, 0x23e1, 0x5397, 0x07ef],
    [0x0018, 0x2389, 0x53cc, 0x0813],
    [0x0016, 0x2331, 0x5401, 0x0838],
    [0x0015, 0x22da, 0x5434, 0x085d],
    [0x0013, 0x2282, 0x5467, 0x0883],
    [0x0012, 0x222c, 0x5499, 0x08a9],
    [0x0011, 0x21d5, 0x54ca, 0x08d0],
    [0x0010, 0x217f, 0x54fa, 0x08f7],
    [0x000f, 0x2129, 0x5529, 0x091e],
    [0x000e, 0x20d4, 0x5558, 0x0946],
    [0x000d, 0x207f, 0x5585, 0x096f],
    [0x000c, 0x202a, 0x55b2, 0x0998],
    [0x000b, 0x1fd6, 0x55de, 0x09c1],
    [0x000a, 0x1f82, 0x5609, 0x09eb],
    [0x0009, 0x1f2f, 0x5632, 0x0a16],
    [0x0009, 0x1edc, 0x565b, 0x0a40],
    [0x0008, 0x1e89, 0x5684, 0x0a6c],
    [0x0007, 0x1e37, 0x56ab, 0x0a98],
    [0x0007, 0x1de5, 0x56d1, 0x0ac4],
    [0x0006, 0x1d93, 0x56f6, 0x0af1],
    [0x0005, 0x1d42, 0x571b, 0x0b1e],
    [0x0005, 0x1cf1, 0x573e, 0x0b4c],
    [0x0004, 0x1ca1, 0x5761, 0x0b7a],
    [0x0004, 0x1c51, 0x5782, 0x0ba9],
    [0x0003, 0x1c02, 0x57a3, 0x0bd8],
    [0x0003, 0x1bb3, 0x57c3, 0x0c07],
    [0x0003, 0x1b64, 0x57e2, 0x0c38],
    [0x0002, 0x1b16, 0x57ff, 0x0c68],
    [0x0002, 0x1ac8, 0x581c, 0x0c99],
    [0x0002, 0x1a7b, 0x5838, 0x0ccb],
    [0x0001, 0x1a2e, 0x5853, 0x0cfd],
    [0x0001, 0x19e2, 0x586d, 0x0d30],
    [0x0001, 0x1996, 0x5886, 0x0d63],
    [0x0001, 0x194b, 0x589e, 0x0d97],
    [0x0000, 0x1900, 0x58b5, 0x0dcb],
    [0x0000, 0x18b5, 0x58cb, 0x0e00],
    [0x0000, 0x186b, 0x58e0, 0x0e35],
    [0x0000, 0x1821, 0x58f4, 0x0e6b],
    [0x0000, 0x17d8, 0x5907, 0x0ea1],
    [0x0000, 0x1790, 0x5919, 0x0ed7],
    [0x0000, 0x1747, 0x592a, 0x0f0f],
    [-1, 0x1700, 0x593a, 0x0f46],
    [-1, 0x16b9, 0x5949, 0x0f7f],
    [-1, 0x1672, 0x5958, 0x0fb7],
    [-1, 0x162c, 0x5965, 0x0ff1],
    [-1, 0x15e6, 0x5971, 0x102a],
    [-1, 0x15a0, 0x597c, 0x1065],
    [-1, 0x155c, 0x5986, 0x109f],
    [-1, 0x1517, 0x598f, 0x10db],
    [-1, 0x14d4, 0x5997, 0x1116],
    [-1, 0x1490, 0x599e, 0x1153],
    [-1, 0x144d, 0x59a4, 0x118f],
    [-1, 0x140b, 0x59a9, 0x11cd],
    [-1, 0x13c9, 0x59ad, 0x120b],
    [-1, 0x1388, 0x59b0, 0x1249],
    [-1, 0x1347, 0x59b2, 0x1288],
    [-1, 0x1307, 0x59b3, 0x12c7],
];

const ADSR_TABLE: [[(i32, i32); 128]; 2] = [
	[
		(1, 14336), 
		(1, 12288), 
		(1, 10240), 
		(1, 8192), 
		(1, 7168), 
		(1, 6144), 
		(1, 5120), 
		(1, 4096), 
		(1, 3584), 
		(1, 3072), 
		(1, 2560), 
		(1, 2048), 
		(1, 1792), 
		(1, 1536), 
		(1, 1280), 
		(1, 1024), 
		(1, 896), 
		(1, 768), 
		(1, 640), 
		(1, 512), 
		(1, 448), 
		(1, 384), 
		(1, 320), 
		(1, 256), 
		(1, 224), 
		(1, 192), 
		(1, 160), 
		(1, 128), 
		(1, 112), 
		(1, 96), 
		(1, 80), 
		(1, 64), 
		(1, 56), 
		(1, 48), 
		(1, 40), 
		(1, 32), 
		(1, 28), 
		(1, 24), 
		(1, 20), 
		(1, 16), 
		(1, 14), 
		(1, 12), 
		(1, 10), 
		(1, 8), 
		(1, 7), 
		(1, 6), 
		(1, 5), 
		(1, 4), 
		(2, 7), 
		(2, 6), 
		(2, 5), 
		(2, 4), 
		(4, 7), 
		(4, 6), 
		(4, 5), 
		(4, 4), 
		(8, 7), 
		(8, 6), 
		(8, 5), 
		(8, 4), 
		(16, 7), 
		(16, 6), 
		(16, 5), 
		(16, 4), 
		(32, 7), 
		(32, 6), 
		(32, 5), 
		(32, 4), 
		(64, 7), 
		(64, 6), 
		(64, 5), 
		(64, 4), 
		(128, 7), 
		(128, 6), 
		(128, 5), 
		(128, 4), 
		(256, 7), 
		(256, 6), 
		(256, 5), 
		(256, 4), 
		(512, 7), 
		(512, 6), 
		(512, 5), 
		(512, 4), 
		(1024, 7), 
		(1024, 6), 
		(1024, 5), 
		(1024, 4), 
		(2048, 7), 
		(2048, 6), 
		(2048, 5), 
		(2048, 4), 
		(4096, 7), 
		(4096, 6), 
		(4096, 5), 
		(4096, 4), 
		(8192, 7), 
		(8192, 6), 
		(8192, 5), 
		(8192, 4), 
		(16384, 7), 
		(16384, 6), 
		(16384, 5), 
		(16384, 4), 
		(32768, 7), 
		(32768, 6), 
		(32768, 5), 
		(32768, 4), 
		(65536, 7), 
		(65536, 6), 
		(65536, 5), 
		(65536, 4), 
		(131072, 7), 
		(131072, 6), 
		(131072, 5), 
		(131072, 4), 
		(262144, 7), 
		(262144, 6), 
		(262144, 5), 
		(262144, 4), 
		(524288, 7), 
		(524288, 6), 
		(524288, 5), 
		(524288, 4), 
		(1048576, 7), 
		(1048576, 6), 
		(1048576, 5), 
		(1048576, 4), 
	],
	[
		(1, -16384), 
		(1, -14336), 
		(1, -12288), 
		(1, -10240), 
		(1, -8192), 
		(1, -7168), 
		(1, -6144), 
		(1, -5120), 
		(1, -4096), 
		(1, -3584), 
		(1, -3072), 
		(1, -2560), 
		(1, -2048), 
		(1, -1792), 
		(1, -1536), 
		(1, -1280), 
		(1, -1024), 
		(1, -896), 
		(1, -768), 
		(1, -640), 
		(1, -512), 
		(1, -448), 
		(1, -384), 
		(1, -320), 
		(1, -256), 
		(1, -224), 
		(1, -192), 
		(1, -160), 
		(1, -128), 
		(1, -112), 
		(1, -96), 
		(1, -80), 
		(1, -64), 
		(1, -56), 
		(1, -48), 
		(1, -40), 
		(1, -32), 
		(1, -28), 
		(1, -24), 
		(1, -20), 
		(1, -16), 
		(1, -14), 
		(1, -12), 
		(1, -10), 
		(1, -8), 
		(1, -7), 
		(1, -6), 
		(1, -5), 
		(2, -8), 
		(2, -7), 
		(2, -6), 
		(2, -5), 
		(4, -8), 
		(4, -7), 
		(4, -6), 
		(4, -5), 
		(8, -8), 
		(8, -7), 
		(8, -6), 
		(8, -5), 
		(16, -8), 
		(16, -7), 
		(16, -6), 
		(16, -5), 
		(32, -8), 
		(32, -7), 
		(32, -6), 
		(32, -5), 
		(64, -8), 
		(64, -7), 
		(64, -6), 
		(64, -5), 
		(128, -8), 
		(128, -7), 
		(128, -6), 
		(128, -5), 
		(256, -8), 
		(256, -7), 
		(256, -6), 
		(256, -5), 
		(512, -8), 
		(512, -7), 
		(512, -6), 
		(512, -5), 
		(1024, -8), 
		(1024, -7), 
		(1024, -6), 
		(1024, -5), 
		(2048, -8), 
		(2048, -7), 
		(2048, -6), 
		(2048, -5), 
		(4096, -8), 
		(4096, -7), 
		(4096, -6), 
		(4096, -5), 
		(8192, -8), 
		(8192, -7), 
		(8192, -6), 
		(8192, -5), 
		(16384, -8), 
		(16384, -7), 
		(16384, -6), 
		(16384, -5), 
		(32768, -8), 
		(32768, -7), 
		(32768, -6), 
		(32768, -5), 
		(65536, -8), 
		(65536, -7), 
		(65536, -6), 
		(65536, -5), 
		(131072, -8), 
		(131072, -7), 
		(131072, -6), 
		(131072, -5), 
		(262144, -8), 
		(262144, -7), 
		(262144, -6), 
		(262144, -5), 
		(524288, -8), 
		(524288, -7), 
		(524288, -6), 
		(524288, -5), 
		(1048576, -8), 
		(1048576, -7), 
		(1048576, -6), 
		(1048576, -5), 
	]
];

fn create_voices() -> [Voice; 24] {
    [
        Voice::new(VoiceIndex(0)),
        Voice::new(VoiceIndex(1)),
        Voice::new(VoiceIndex(2)),
        Voice::new(VoiceIndex(3)),
        Voice::new(VoiceIndex(4)),
        Voice::new(VoiceIndex(5)),
        Voice::new(VoiceIndex(6)),
        Voice::new(VoiceIndex(7)),
        Voice::new(VoiceIndex(8)),
        Voice::new(VoiceIndex(9)),
        Voice::new(VoiceIndex(10)),
        Voice::new(VoiceIndex(11)),
        Voice::new(VoiceIndex(12)),
        Voice::new(VoiceIndex(13)),
        Voice::new(VoiceIndex(14)),
        Voice::new(VoiceIndex(15)),
        Voice::new(VoiceIndex(16)),
        Voice::new(VoiceIndex(17)),
        Voice::new(VoiceIndex(18)),
        Voice::new(VoiceIndex(19)),
        Voice::new(VoiceIndex(20)),
        Voice::new(VoiceIndex(21)),
        Voice::new(VoiceIndex(22)),
        Voice::new(VoiceIndex(23)),
    ]
}

impl BusMap for Spu {
    const BUS_BEGIN: u32 = 0x1f801c00;
    const BUS_END: u32 = Self::BUS_BEGIN + 640 - 1;
}
