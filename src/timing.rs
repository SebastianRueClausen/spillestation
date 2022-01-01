
#![allow(dead_code)]

/// The CPU cycle speed.
pub const CPU_HZ: u64 = 33_868_800;

/// The Video cycle speed, also called dotclock.
pub const GPU_HZ: u64 = 53_222_400;

/// Scanlines per frame for PAL.
pub const PAL_SCLN_PER_FRAME: u64 = 314;

/// Scanlines per frame for NTSC.
pub const NTSC_SCLN_PER_FRAME: u64 = 263;

/// Video cycles per scanline on PAL.
pub const PAL_CYCLES_PER_SCLN: u64 = 3406;

/// Video cycles per scanline on NTSC.
pub const NTSC_CYCLES_PER_SCLN: u64 = 3413;

/// Frames per second for PAL, about 50 fps.
pub const PAL_FPS: u64 = GPU_HZ / PAL_SCLN_PER_FRAME / PAL_CYCLES_PER_SCLN;

/// Frames per second for NTSC, about 59 fps;
pub const NTSC_FPS: u64 = GPU_HZ / NTSC_SCLN_PER_FRAME / NTSC_CYCLES_PER_SCLN;

pub const fn cpu_to_gpu_cycles(cycles: u64) -> u64 {
    cycles * (11 / 7)
}
