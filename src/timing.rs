
#![allow(dead_code)]

/// The CPU cycle speed.
pub const CPU_HZ: u64 = 33_868_800;

/// The Video cycle speed, also called dotclock.
pub const GPU_HZ: u64 = 53_222_400;

// The same for PAL and NTSC.
pub const HSYNC_CYCLES: u64 = 200;

pub const PAL_SCLN_PER_FRAME: u64 = 314;
pub const PAL_CYCLES_PER_SCLN: u64 = 3406;
pub const PAL_SCLN_COUNT: u64 = 314;
pub const PAL_HBEGIN: u64 = 487;
pub const PAL_HEND: u64 = 3282;
pub const PAL_VBEGIN: u64 = 20;
pub const PAL_VEND: u64 = 308;
pub const PAL_VERTICAL_RANGE: std::ops::Range<u64> = PAL_VBEGIN..PAL_VEND;

pub const NTSC_SCLN_PER_FRAME: u64 = 263;
pub const NTSC_CYCLES_PER_SCLN: u64 = 3413;
pub const NTSC_SCLN_COUNT: u64 = 263;
pub const NTSC_HBEGIN: u64 = 488;
pub const NTSC_HEND: u64 = 3288;
pub const NTSC_VBEGIN: u64 = 16;
pub const NTSC_VEND: u64 = 256;
pub const NTSC_VERTICAL_RANGE: std::ops::Range<u64> = NTSC_VBEGIN..NTSC_VEND;

pub const PAL_FPS: u64 = GPU_HZ / PAL_SCLN_PER_FRAME / PAL_CYCLES_PER_SCLN;
pub const NTSC_FPS: u64 = GPU_HZ / NTSC_SCLN_PER_FRAME / NTSC_CYCLES_PER_SCLN;

pub const fn cpu_to_gpu_cycles(cycles: u64) -> u64 {
    cycles * (11 / 7)
}
