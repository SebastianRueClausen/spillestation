//! # TODO
//!
//! - SIMD optimize.

use crate::{dump, dump::Dumper};
use splst_util::{Bit, BitSet};

use std::fmt;

#[derive(Default)]
pub struct Gte {
    data: DataRegs,
    control: ControlRegs,
}

impl Gte {
    pub(super) fn control_store(&mut self, offset: u32, val: u32) {
        assert!(offset < 32, "invalid control register store at {offset}");

        match offset {
            4 | 12 | 20 | 26 | 27 | 29 | 30 => unsafe {
                let val = (val as u16) as i32;
                self.control.store_unchecked(offset, val as u32);
            },
            31 => self.control.flags.0 = val & 0x7ffff000,
            _ => unsafe {
                self.control.store_unchecked(offset, val);
            },
        }
    }

    pub(super) fn control_load(&mut self, offset: u32) -> u32 {
        assert!(offset < 32, "invalid control register store at {offset}");
        unsafe { self.control.load_unchecked(offset) }
    }

    pub(super) fn data_store(&mut self, offset: u32, val: u32) {
        assert!(offset < 32, "invalid control register store at {offset}");

        match offset {
            1 | 3 | 5 | 8..=11 => unsafe {
                // Sign extend z values of vectors and accumulators.
                let val = (val as u16) as i32;
                self.data.store_unchecked(offset, val as u32);
            },
            15 => unsafe {
                // Writes to `sxyp` pushes it onto the screen xy coordinate FIFO, which means that
                // the rest of the values in the FIFO get's pushed one spot down.
                self.data.sxy0 = self.data.sxy1;
                self.data.sxy1 = self.data.sxy2;
                self.data.sxy2 = self.data.sxyp;

                self.data.store_unchecked(offset, val);
            },
            7 | 16..=19 => unsafe {
                self.data.store_unchecked(offset, val & 0xffff);
            },
            28 => unsafe {
                // truncate to 16 bit intergs for rgb values.
                self.data.irgb = val & 0x7fff;

                fn as_ir(val: u32) -> i16 {
                    ((val & 0x1f) << 7) as i16
                }

                self.data.store_unchecked(09, as_ir(val) as u32);
                self.data.store_unchecked(10, as_ir(val >> 05) as u32);
                self.data.store_unchecked(11, as_ir(val >> 10) as u32);
            },
            30 => {
                self.data.lzcs = val as i32;
                // `val` is in this case a signed 32 bit int. If bit 31 is set, it's a negative
                // value, in which case the `lzcr` counts the leading ones instead of leading
                // zeroes.
                self.data.lzcr = if val.bit(31) {
                    val.leading_ones()
                } else {
                    val.leading_zeros()
                };
            }
            29 | 31 => (),
            _ => unsafe {
                self.data.store_unchecked(offset, val);
            },
        }
    }

    pub(super) fn data_load(&mut self, offset: u32) -> u32 {
        assert!(offset < 32, "invalid control register store at {offset}");

        match offset {
            // `sxyp` is a mirror of `sxy2`.
            15 => unsafe { self.data.load_unchecked(14) },
            28 | 29 => {
                let r = (self.data.ir[0].0 >> 7).clamp(0x0, 0x1f) as u32;
                let g = (self.data.ir[1].0 >> 7).clamp(0x0, 0x1f) as u32;
                let b = (self.data.ir[2].0 >> 7).clamp(0x0, 0x1f) as u32;

                r | (g << 5) | (b << 10)
            }
            _ => unsafe { self.data.load_unchecked(offset) },
        }
    }

    pub(super) fn exec(&mut self, val: u32) {
        self.control.flags.clear();

        let op = Opcode(val);
        debug!("{op}");

        match op.cmd() {
            0x01 => self.cmd_rtps(op),
            0x06 => self.cmd_nclip(),
            0x0c => self.cmd_op(op),
            0x10 => self.cmd_dpcs(op),
            0x11 => self.cmd_intpl(op),
            0x12 => self.cmd_mvmva(op),
            0x13 => self.cmd_ncds(op),
            0x16 => self.cmd_ncdt(op),
            0x1b => self.cmd_nccs(op),
            0x1c => self.cmd_cc(op),
            0x1e => self.cmd_ncs(op),
            0x20 => self.cmd_nct(op),
            0x28 => self.cmd_sqr(op),
            0x29 => self.cmd_dcpl(op),
            0x2a => self.cmd_dpct(op),
            0x2d => self.cmd_avsz3(),
            0x2e => self.cmd_avsz4(),
            0x30 => self.cmd_rtpt(op),
            0x3d => self.cmd_gpf(op),
            0x3e => self.cmd_gpl(op),
            0x3f => self.cmd_ncct(op),
            cmd => panic!("invalid GTE command {cmd:08x}"),
        }
    }

    pub fn data_regs(&self) -> &DataRegs {
        &self.data
    }

    pub fn control_regs(&self) -> &ControlRegs {
        &self.control
    }

    /// Truncate `val`, check for overflow and set `mac[idx]`.
    ///
    /// Returns `val` shifted and truncated.
    #[inline]
    fn set_mac(&mut self, idx: usize, shift: u8, val: i64) -> i32 {
        let val = saturate_to_mac(&mut self.control.flags, idx, shift, val);
        self.data.mac[idx] = val;
        val
    }

    /// Truncate `val`, check for overflow and set `mac0`.
    #[inline]
    fn set_mac0(&mut self, val: i64) {
        self.data.mac0 = saturate_to_mac0(&mut self.control.flags, val);
    }

    /// Saturate `val` at set `ir[idx]`.
    #[inline]
    fn set_ir(&mut self, idx: usize, clamp: bool, val: i32) {
        self.data.ir[idx].0 = saturate_to_ir(&mut self.control.flags, idx, clamp, val);
    }

    /// Saturate `val` and set `ir0`.
    #[inline]
    fn set_ir0(&mut self, val: i32) {
        self.data.ir0 = saturate_to_ir0(&mut self.control.flags, val);
    }

    #[inline]
    fn set_ir_and_mac(&mut self, idx: usize, shift: u8, clamp: bool, val: i64) {
        let val = self.set_mac(idx, shift, val);
        self.set_ir(idx, clamp, val);
    }

    /// Saturate `val` and set `otz`.
    #[inline]
    fn set_otz(&mut self, val: i32) {
        let (val, of) = saturate(0x0, 0xffff, val);
        self.control.flags.set_flag(18, of);
        self.data.otz = val as u16;
    }

    #[inline]
    fn ir_vec(&self) -> Vec3<i16> {
        Vec3 {
            x: self.data.ir[0].0,
            y: self.data.ir[1].0,
            z: self.data.ir[2].0,
        }
    }

    /// Push `val` into `sz` FIFO.
    ///
    /// Returns saturated `val`.
    fn sz_push(&mut self, val: i32) -> u16 {
        const MAX: i32 = 0xffff;
        const MIN: i32 = 0x0000;

        self.control.flags.set_flag(18, !(MIN..=MAX).contains(&val));
        let val = val.clamp(MIN, MAX) as u16;

        self.data.sz[0] = self.data.sz[1];
        self.data.sz[1] = self.data.sz[2];
        self.data.sz[2] = self.data.sz[3];
        self.data.sz[3] = (val, 0);

        val
    }

    fn sxy_push(&mut self, x: i32, y: i32) {
        const MAX: i32 = 0x3ff;
        const MIN: i32 = -0x400;

        self.control.flags.set_flag(14, !(MIN..=MAX).contains(&x));
        self.control.flags.set_flag(13, !(MIN..=MAX).contains(&y));

        let xy = [x.clamp(MIN, MAX) as i16, y.clamp(MIN, MAX) as i16];

        self.data.sxy0 = self.data.sxy1;
        self.data.sxy1 = self.data.sxy2;
        self.data.sxy2 = self.data.sxyp;

        self.data.sxyp = xy;
    }

    fn rgb_push_from_mac(&mut self) {
        let rgbc = Rgb {
            r: saturate_to_rgb(&mut self.control.flags, 0, self.data.mac[0] >> 4),
            g: saturate_to_rgb(&mut self.control.flags, 1, self.data.mac[1] >> 4),
            b: saturate_to_rgb(&mut self.control.flags, 2, self.data.mac[2] >> 4),
            _pad: self.data.rgbc[3],
        };

        self.data.rgb0 = self.data.rgb1;
        self.data.rgb1 = self.data.rgb2;
        self.data.rgb2 = rgbc;
    }

    /// Rotate, translate, perspective transform `vec`.
    ///
    /// Returns projection factor.
    fn rtp(&mut self, vec: Vec3<i16>, shift: u8, clamp: bool) -> i64 {
        // Translate and rotate `vec`, which is done with the formula `tr` + `vec` * `rt`.

        let mut dot_row = |add: i32, row: usize| {
            let tr =
                (i64::from(add) << 12) + i64::from(self.control.rt.0[row].x) * i64::from(vec.x);
            let rt = sign_extend_mac(&mut self.control.flags, row, tr)
                + i64::from(self.control.rt.0[row].y) * i64::from(vec.y)
                + i64::from(self.control.rt.0[row].z) * i64::from(vec.z);
            sign_extend_mac(&mut self.control.flags, row, rt)
        };

        let x = dot_row(self.control.tr.x, 0);
        let y = dot_row(self.control.tr.y, 1);
        let z = dot_row(self.control.tr.z, 2);

        self.set_ir_and_mac(0, shift, clamp, x);
        self.set_ir_and_mac(1, shift, clamp, y);

        self.set_mac(2, shift, z);

        // So apparently there is a hardware bug here when clamping the z into the IR registers.
        // The value of ir3 is taken from MAC as usual, but the overflow flag is calculated from
        // the z value shifted 12 bits to the right.
        //
        // We just use `set_ir` as usual to calculate the flags and then overwrite it.
        let z_shift = (z >> 12) as i32;

        self.set_ir(2, false, z_shift);

        // Set the value as usual.
        self.data.ir[2].0 = {
            let val = self.data.mac[2];
            let min = if clamp { 0 } else { -(1 << 15) };
            val.clamp(min, (1 << 15) - 1) as i16
        };

        let z = self.sz_push(z_shift);

        // Calculate projection factor. `pf` is a 1.16 unsigned fixed-point integer.

        let pf: i64 = if z > self.control.h / 2 {
            nr_divide(self.control.h, z).into()
        } else {
            self.control.flags.set_flag(17, true);
            0x1ffff
        };

        let sx = i64::from(self.data.ir[0].0) + pf + i64::from(self.control.ofx);
        let sy = i64::from(self.data.ir[1].0) + pf + i64::from(self.control.ofy);

        check_mac0_overflow(&mut self.control.flags, sx);
        check_mac0_overflow(&mut self.control.flags, sy);

        self.sxy_push((sx >> 16) as i32, (sy >> 16) as i32);

        pf
    }

    #[inline]
    fn color_interp(&mut self, rgb: &[i64; 3], shift: u8, clamp: bool) {
        let v0 = i64::from(self.control.fc.x << 12) - rgb[0];
        let v1 = i64::from(self.control.fc.y << 12) - rgb[1];
        let v2 = i64::from(self.control.fc.z << 12) - rgb[2];

        self.set_ir_and_mac(0, shift, false, v0);
        self.set_ir_and_mac(1, shift, false, v1);
        self.set_ir_and_mac(2, shift, false, v2);

        let v0: i64 = (self.data.ir[0].0 as i32 * self.data.ir0 as i32).into();
        let v1: i64 = (self.data.ir[1].0 as i32 * self.data.ir0 as i32).into();
        let v2: i64 = (self.data.ir[2].0 as i32 * self.data.ir0 as i32).into();

        self.set_ir_and_mac(0, shift, clamp, v0 + rgb[0]);
        self.set_ir_and_mac(1, shift, clamp, v1 + rgb[1]);
        self.set_ir_and_mac(2, shift, clamp, v2 + rgb[2]);
    }

    #[inline]
    fn dpcs(&mut self, rgb: [u8; 3], shift: u8, clamp: bool) {
        self.set_mac(0, 0, i64::from(rgb[0]) << 16);
        self.set_mac(1, 0, i64::from(rgb[1]) << 16);
        self.set_mac(2, 0, i64::from(rgb[2]) << 16);

        let rgb = [
            self.data.mac[0] as i64,
            self.data.mac[1] as i64,
            self.data.mac[2] as i64,
        ];

        self.color_interp(&rgb, shift, clamp);
        self.rgb_push_from_mac();
    }

    #[inline]
    fn depth_queue(&mut self, pf: i64) {
        let depth = self.control.dqb as i64 + self.control.dqa as i64 * pf;

        self.set_mac0(depth);
        self.set_ir0((depth >> 12) as i32);
    }

    #[inline]
    fn ncd(&mut self, vec: Vec3<i16>, shift: u8, clamp: bool) {
        (_, self.data.ir) = mat_mul(
            &self.control.llm,
            &vec,
            &mut self.control.flags,
            shift,
            clamp,
        );
        (self.data.mac, self.data.ir) = mat_mul_add(
            &self.control.lcm,
            &self.control.bk,
            &self.ir_vec(),
            &mut self.control.flags,
            shift,
            clamp,
        );
        let rgb = [
            (i64::from(self.data.rgbc[0]) * i64::from(self.data.ir[0].0)) << 4,
            (i64::from(self.data.rgbc[1]) * i64::from(self.data.ir[1].0)) << 4,
            (i64::from(self.data.rgbc[2]) * i64::from(self.data.ir[2].0)) << 4,
        ];
        self.color_interp(&rgb, shift, clamp);
        self.rgb_push_from_mac();
    }

    #[inline]
    fn ncc(&mut self, vec: Vec3<i16>, shift: u8, clamp: bool) {
        (_, self.data.ir) = mat_mul(
            &self.control.llm,
            &vec,
            &mut self.control.flags,
            shift,
            clamp,
        );

        (self.data.mac, self.data.ir) = mat_mul_add(
            &self.control.lcm,
            &self.control.bk,
            &self.ir_vec(),
            &mut self.control.flags,
            shift,
            clamp,
        );

        let r = (i64::from(self.data.rgbc[0]) * i64::from(self.data.ir[0].0)) << 4;
        let g = (i64::from(self.data.rgbc[1]) * i64::from(self.data.ir[1].0)) << 4;
        let b = (i64::from(self.data.rgbc[2]) * i64::from(self.data.ir[2].0)) << 4;

        self.set_ir_and_mac(0, shift, clamp, r);
        self.set_ir_and_mac(1, shift, clamp, g);
        self.set_ir_and_mac(2, shift, clamp, b);

        self.rgb_push_from_mac();
    }

    #[inline]
    fn nc(&mut self, vec: Vec3<i16>, shift: u8, clamp: bool) {
        (_, self.data.ir) = mat_mul(
            &self.control.llm,
            &vec,
            &mut self.control.flags,
            shift,
            clamp,
        );

        (self.data.mac, self.data.ir) = mat_mul_add(
            &self.control.lcm,
            &self.control.bk,
            &self.ir_vec(),
            &mut self.control.flags,
            shift,
            clamp,
        );

        self.rgb_push_from_mac();
    }

    fn avsz(&mut self, val: i16) {
        let sz = i32::from(self.data.sz[1].0)
            + i32::from(self.data.sz[2].0)
            + i32::from(self.data.sz[3].0);
        let val = i64::from(val) * i64::from(sz);

        self.set_mac0(val);
        self.set_otz((val >> 12) as i32);
    }
}

impl Gte {
    fn cmd_rtps(&mut self, op: Opcode) {
        let pf = self.rtp(self.data.v0, op.shift(), op.clamp());
        self.depth_queue(pf);
    }

    fn cmd_nclip(&mut self) {
        let [x0, y0] = self.data.sxy0;
        let [x1, y1] = self.data.sxy1;
        let [x2, y2] = self.data.sxy2;

        let [x0, y0]: [i32; 2] = [x0.into(), y0.into()];
        let [x1, y1]: [i32; 2] = [x1.into(), y1.into()];
        let [x2, y2]: [i32; 2] = [x2.into(), y2.into()];

        let vals: [i64; 3] = [
            (x0 * (y1 - y2)).into(),
            (x1 * (y2 - y0)).into(),
            (x2 * (y0 - y1)).into(),
        ];

        let sum: i64 = vals.iter().sum();

        self.set_mac0(sum);
    }

    /// Outer product.
    fn cmd_op(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        let d0: i64 = self.control.rt.0[0].x.into();
        let d1: i64 = self.control.rt.0[1].y.into();
        let d2: i64 = self.control.rt.0[2].z.into();

        let ir0: i64 = self.data.ir[0].0.into();
        let ir1: i64 = self.data.ir[1].0.into();
        let ir2: i64 = self.data.ir[2].0.into();

        let v0 = (ir2 * d1) - (ir1 * d2);
        let v1 = (ir0 * d2) - (ir2 * d0);
        let v2 = (ir1 * d0) - (ir0 * d1);

        self.set_ir_and_mac(0, shift, clamp, v0);
        self.set_ir_and_mac(1, shift, clamp, v1);
        self.set_ir_and_mac(2, shift, clamp, v2);
    }

    fn cmd_dpcs(&mut self, op: Opcode) {
        let rgb = [self.data.rgbc[0], self.data.rgbc[1], self.data.rgbc[2]];
        self.dpcs(rgb, op.shift(), op.clamp());
    }

    fn cmd_intpl(&mut self, op: Opcode) {
        let rgb = [
            i64::from(self.data.ir[0].0) << 12,
            i64::from(self.data.ir[1].0) << 12,
            i64::from(self.data.ir[2].0) << 12,
        ];

        self.color_interp(&rgb, op.shift(), op.clamp());
        self.rgb_push_from_mac();
    }

    fn cmd_mvmva(&mut self, op: Opcode) {
        let mat = match op.mat() {
            0 => &self.control.rt,
            1 => &self.control.llm,
            2 => &self.control.lcm,
            _ => todo!("buggy matrix"),
        };
        let trans = match op.tr_vec() {
            0 => &self.control.tr,
            1 => &self.control.bk,
            2 => &self.control.fc,
            _ => &Vec3 { x: 0, y: 0, z: 0 },
        };
        let vec = match op.vec() {
            0 => self.data.v0,
            1 => self.data.v1,
            2 => self.data.v2,
            _ => self.ir_vec(),
        };

        (self.data.mac, self.data.ir) = mat_mul_add(
            mat,
            trans,
            &vec,
            &mut self.control.flags,
            op.shift(),
            op.clamp(),
        );
    }

    fn cmd_ncds(&mut self, op: Opcode) {
        self.ncd(self.data.v0, op.shift(), op.clamp());
    }

    fn cmd_ncdt(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        self.ncd(self.data.v0, shift, clamp);
        self.ncd(self.data.v1, shift, clamp);
        self.ncd(self.data.v2, shift, clamp);
    }

    fn cmd_nccs(&mut self, op: Opcode) {
        self.ncc(self.data.v0, op.shift(), op.clamp());
    }

    fn cmd_cc(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        (self.data.mac, self.data.ir) = mat_mul_add(
            &self.control.lcm,
            &self.control.bk,
            &self.ir_vec(),
            &mut self.control.flags,
            shift,
            clamp,
        );

        let r = (i64::from(self.data.rgbc[0]) * i64::from(self.data.ir[0].0)) << 4;
        let g = (i64::from(self.data.rgbc[1]) * i64::from(self.data.ir[1].0)) << 4;
        let b = (i64::from(self.data.rgbc[2]) * i64::from(self.data.ir[2].0)) << 4;

        self.set_ir_and_mac(0, shift, clamp, r);
        self.set_ir_and_mac(1, shift, clamp, g);
        self.set_ir_and_mac(2, shift, clamp, b);

        self.rgb_push_from_mac();
    }

    fn cmd_ncct(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        self.ncc(self.data.v0, shift, clamp);
        self.ncc(self.data.v1, shift, clamp);
        self.ncc(self.data.v2, shift, clamp);
    }

    fn cmd_ncs(&mut self, op: Opcode) {
        self.nc(self.data.v0, op.shift(), op.clamp());
    }

    fn cmd_nct(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        self.nc(self.data.v0, shift, clamp);
        self.nc(self.data.v1, shift, clamp);
        self.nc(self.data.v2, shift, clamp);
    }

    fn cmd_sqr(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        self.data.mac[0] = (i32::from(self.data.ir[0].0) * i32::from(self.data.ir[0].0)) >> shift;
        self.data.mac[1] = (i32::from(self.data.ir[1].0) * i32::from(self.data.ir[1].0)) >> shift;
        self.data.mac[2] = (i32::from(self.data.ir[2].0) * i32::from(self.data.ir[2].0)) >> shift;

        self.set_ir(0, clamp, self.data.mac[0]);
        self.set_ir(1, clamp, self.data.mac[1]);
        self.set_ir(1, clamp, self.data.mac[2]);
    }

    fn cmd_dcpl(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        let rgb = [
            (i64::from(self.data.rgbc[0]) * i64::from(self.data.ir[0].0)) << 4,
            (i64::from(self.data.rgbc[1]) * i64::from(self.data.ir[1].0)) << 4,
            (i64::from(self.data.rgbc[2]) * i64::from(self.data.ir[2].0)) << 4,
        ];

        self.color_interp(&rgb, shift, clamp);
        self.rgb_push_from_mac();
    }

    fn cmd_dpct(&mut self, op: Opcode) {
        for _ in 0..3 {
            let rgb = [self.data.rgb0.r, self.data.rgb0.g, self.data.rgb0.b];
            self.dpcs(rgb, op.shift(), op.clamp());
        }
    }

    fn cmd_avsz3(&mut self) {
        self.avsz(self.control.zsf3);
    }

    fn cmd_avsz4(&mut self) {
        self.avsz(self.control.zsf4);
    }

    fn cmd_rtpt(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        self.rtp(self.data.v0, shift, clamp);
        self.rtp(self.data.v1, shift, clamp);

        let pf = self.rtp(self.data.v2, shift, clamp);

        self.depth_queue(pf);
    }

    fn cmd_gpf(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        let ir1 = i64::from(self.data.ir[0].0) * i64::from(self.data.ir0);
        let ir2 = i64::from(self.data.ir[1].0) * i64::from(self.data.ir0);
        let ir3 = i64::from(self.data.ir[2].0) * i64::from(self.data.ir0);

        self.set_ir_and_mac(0, shift, clamp, ir1);
        self.set_ir_and_mac(1, shift, clamp, ir2);
        self.set_ir_and_mac(2, shift, clamp, ir3);

        self.rgb_push_from_mac();
    }

    fn cmd_gpl(&mut self, op: Opcode) {
        let shift = op.shift();
        let clamp = op.clamp();

        let ir1 = i64::from(self.data.ir[0].0) * i64::from(self.data.ir0);
        let ir2 = i64::from(self.data.ir[1].0) * i64::from(self.data.ir0);
        let ir3 = i64::from(self.data.ir[2].0) * i64::from(self.data.ir0);

        let ir1 = ir1 + (i64::from(self.data.mac[0]) << shift);
        let ir2 = ir2 + (i64::from(self.data.mac[1]) << shift);
        let ir3 = ir3 + (i64::from(self.data.mac[2]) << shift);

        self.set_ir_and_mac(0, shift, clamp, ir1);
        self.set_ir_and_mac(1, shift, clamp, ir2);
        self.set_ir_and_mac(2, shift, clamp, ir3);

        self.rgb_push_from_mac();
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Vec3<T: Copy> {
    x: T,
    y: T,
    z: T,
}

impl<T: fmt::Display + Copy> fmt::Display for Vec3<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
    _pad: u8,
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {}, {})", self.r, self.g, self.b)
    }
}

/// GTE data registers (0..=31).
#[repr(C)]
#[derive(Default)]
pub struct DataRegs {
    /// Vector 0.
    v0: Vec3<i16>,
    _pad0: u16,
    /// Vector 1.
    v1: Vec3<i16>,
    _pad1: u16,
    /// Vector 2.
    v2: Vec3<i16>,
    _pad2: u16,
    /// Color codes.
    rgbc: [u8; 4],
    /// Average z value.
    otz: u16,
    _pad3: u16,
    /// Interpolation accumulator.
    ir0: i16,
    _pad4: u16,
    /// 16 bit vector accumulator, each followed by u16 padding.
    ir: [(i16, u16); 3],
    /// Screen xy coordinate FIFO stage 0.
    sxy0: [i16; 2],
    /// Screen xy coordinate FIFO stage 1.
    sxy1: [i16; 2],
    /// Screen xy coordinate FIFO stage 2.
    sxy2: [i16; 2],
    /// Screen xy coordinate FIFO stage 3 (what does p stand for?).
    sxyp: [i16; 2],
    /// Screen z coordinate FIFO, each element followed by padding.
    sz: [(u16, u16); 4],
    /// Color FIFO stage 0.
    rgb0: Rgb,
    /// Color FIFO stage 1.
    rgb1: Rgb,
    /// Color FIFO stage 2.
    rgb2: Rgb,
    /// ??
    _res1: [u8; 4],
    /// 32 bit value accumulator.
    mac0: i32,
    /// 32 bit matrix accumulator.
    mac: [i32; 3],
    /// Input rgb values for conversion.
    irgb: u32,
    /// Output rgb values for conversion.
    _orgb: u32,
    /// Count leading zeros or ones. This is the input value.
    lzcs: i32,
    /// The amount of leading zeroes or ones (depending on sign) in `lzcs`.
    lzcr: u32,
}

impl DataRegs {
    pub fn dump(&self, d: &mut impl Dumper) {
        dump!(d, "vector 0", "{}", &self.v0);
        dump!(d, "vector 1", "{}", &self.v1);
        dump!(d, "vector 2", "{}", &self.v2);

        dump!(
            d,
            "rgbc",
            "{}, {}, {}, {}",
            self.rgbc[0],
            self.rgbc[1],
            self.rgbc[2],
            self.rgbc[3],
        );

        dump!(d, "ir0", "{}", self.ir0);
        dump!(d, "ir1", "{}", self.ir[0].0);
        dump!(d, "ir2", "{}", self.ir[1].0);
        dump!(d, "ir3", "{}", self.ir[2].0);

        dump!(d, "sxy0", "{}, {}", self.sxy0[0], self.sxy0[1]);
        dump!(d, "sxy1", "{}, {}", self.sxy1[0], self.sxy1[1]);
        dump!(d, "sxy2", "{}, {}", self.sxy2[0], self.sxy2[1]);
        dump!(d, "sxyp", "{}, {}", self.sxyp[0], self.sxyp[1]);

        dump!(
            d,
            "sz",
            "{}, {}, {}, {}",
            self.sz[0].0,
            self.sz[1].0,
            self.sz[2].0,
            self.sz[3].0,
        );

        dump!(d, "rgb 0", "{}", self.rgb0);
        dump!(d, "rgb 1", "{}", self.rgb1);
        dump!(d, "rgb 2", "{}", self.rgb2);

        dump!(d, "mac0", "{}", self.mac0);
        dump!(d, "mac1", "{}", self.mac[0]);
        dump!(d, "mac2", "{}", self.mac[1]);
        dump!(d, "mac3", "{}", self.mac[2]);

        // `irgb` and `orgb`.
        let (r, g, b) = (
            (self.ir[0].0 >> 7).clamp(0x0, 0x1f) as u8,
            (self.ir[1].0 >> 7).clamp(0x0, 0x1f) as u8,
            (self.ir[2].0 >> 7).clamp(0x0, 0x1f) as u8,
        );

        dump!(d, "irgb", "{r}, {g}, {b}");
        dump!(d, "orgb", "{r}, {g}, {b}");

        dump!(d, "lzcs", "{}", self.lzcs);
        dump!(d, "lzcr", "{}", self.lzcr);
    }
}

#[derive(Default)]
struct Matrix([Vec3<i16>; 3]);

impl fmt::Display for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\n{}\n{}", self.0[0], self.0[1], self.0[2])
    }
}

/// GTE control registers (32..=63).
#[repr(C)]
#[derive(Default)]
pub struct ControlRegs {
    /// Rotation matrix.
    rt: Matrix,
    _pad0: u16,
    /// Translation vector.
    tr: Vec3<i32>,
    /// Light source matrix.
    llm: Matrix,
    _pad1: u16,
    /// Background color.
    bk: Vec3<i32>,
    /// Light color matrix.
    lcm: Matrix,
    _pad2: u16,
    /// Far color.
    fc: Vec3<i32>,
    /// X screen offset.
    ofx: i32,
    /// Y screen offset.
    ofy: i32,
    /// Projection plane distance.
    h: u16,
    _pad3: u16,
    /// Depth queing parameter A.
    dqa: i16,
    _pad4: u16,
    /// Depth queing parameter B.
    dqb: i32,
    /// Average Z3 scale factor.
    zsf3: i16,
    _pad5: u16,
    /// Average Z4 scale factor.
    zsf4: i16,
    _pad6: u16,
    /// Flags register.
    flags: Flags,
}

impl ControlRegs {
    pub fn dump(&self, d: &mut impl Dumper) {
        dump!(d, "rotation", "{}", self.rt);
        dump!(d, "translation", "{}", self.tr);
        dump!(d, "light source", "{}", self.llm);
        dump!(d, "background color", "{}", self.bk);
        dump!(d, "light color", "{}", self.lcm);
        dump!(d, "far color", "{}", self.fc);
        dump!(d, "ofx", "{}", self.ofx);
        dump!(d, "ofy", "{}", self.ofy);
        dump!(d, "h", "{}", self.h);
        dump!(d, "dqa", "{}", self.dqa);
        dump!(d, "dqb", "{}", self.dqb);
        dump!(d, "zsf3", "{}", self.zsf3);
        dump!(d, "zsf4", "{}", self.zsf4);

        // TODO: Flags.
    }
}

macro_rules! impl_unsafe_io {
    ($t:ident) => {
        impl $t {
            #[inline]
            pub unsafe fn load_unchecked(&self, offset: u32) -> u32 {
                *(self as *const Self as *const u32).offset(offset as isize)
            }

            #[inline]
            pub unsafe fn store_unchecked(&mut self, offset: u32, val: u32) {
                *(self as *mut Self as *mut u32).offset(offset as isize) = val;
            }
        }
    };
}

impl_unsafe_io!(DataRegs);
impl_unsafe_io!(ControlRegs);

#[inline]
fn mat_mul_add(
    mat: &Matrix,
    trans: &Vec3<i32>,
    vec: &Vec3<i16>,
    flags: &mut Flags,
    shift: u8,
    clamp: bool,
) -> ([i32; 3], [(i16, u16); 3]) {
    let mut mul_row = |add: i32, row: usize| {
        let val = (i64::from(add) << 12) + i64::from(mat.0[row].x) * i64::from(vec.x);

        let dot = sign_extend_mac(flags, row, val)
            + i64::from(mat.0[row].y) * i64::from(vec.y)
            + i64::from(mat.0[row].z) * i64::from(vec.z);

        let val = sign_extend_mac(flags, row, dot);

        let mac = saturate_to_mac(flags, row, shift, val);
        let ir = saturate_to_ir(flags, row, clamp, mac);

        (mac, ir)
    };

    let (mac1, ir1) = mul_row(trans.x, 0);
    let (mac2, ir2) = mul_row(trans.y, 1);
    let (mac3, ir3) = mul_row(trans.z, 2);

    ([mac1, mac2, mac3], [(ir1, 0), (ir2, 0), (ir3, 0)])
}

#[inline]
fn mat_mul(
    mat: &Matrix,
    vec: &Vec3<i16>,
    flags: &mut Flags,
    shift: u8,
    clamp: bool,
) -> ([i32; 3], [(i16, u16); 3]) {
    let mut mul_row = |row: usize| {
        let val =
            i64::from(mat.0[row].x) * i64::from(vec.x) + i64::from(mat.0[row].y) * i64::from(vec.y);

        let dot = sign_extend_mac(flags, row, val) + i64::from(mat.0[row].z) * i64::from(vec.z);

        let val = sign_extend_mac(flags, row, dot);
        let mac = saturate_to_mac(flags, row, shift, val);
        let ir = saturate_to_ir(flags, row, clamp, mac);

        (mac, ir)
    };

    let (mac1, ir1) = mul_row(0);
    let (mac2, ir2) = mul_row(1);
    let (mac3, ir3) = mul_row(2);

    ([mac1, mac2, mac3], [(ir1, 0), (ir2, 0), (ir3, 0)])
}

/// Saturate `val` between `min` and `max`. Returns true of overflow occurs.
#[inline]
fn saturate<T: Ord + Copy>(min: T, max: T, val: T) -> (T, bool) {
    let of = !(min..=max).contains(&val);
    let val = val.clamp(min, max);
    (val, of)
}

/// Check for overflow (and underflow) for `mac` values, but not `mac0`.
///
/// `idx` is the index into `mac` and not for instance 1 for mac1.
#[inline]
fn check_mac_overflow(flags: &mut Flags, idx: usize, val: i64) {
    flags.set_flag(30 - idx, val > (1 << 43) - 1);
    flags.set_flag(27 - idx, val < -(1 << 43));
}

/// Check for overflow (and underflow) for `mac0` values.
#[inline]
fn check_mac0_overflow(flags: &mut Flags, val: i64) {
    flags.set_flag(16, val > (1 << 31) - 1);
    flags.set_flag(15, val < -(1 << 31));
}

/// Sign extend `mac` values, and check for overflow.
#[inline]
fn sign_extend_mac(flags: &mut Flags, idx: usize, val: i64) -> i64 {
    check_mac_overflow(flags, idx, val);
    // Sign extend at 43rd bit, which is basically truncating to only keep lowest 43 bits and
    // the sign bit.
    (val << 20) >> 20
}

/// Saturate `val` into rgb value, where `idx` is the index into rgb. Also checks overflow.
#[inline]
fn saturate_to_rgb(flags: &mut Flags, idx: usize, val: i32) -> u8 {
    let (val, of) = saturate(0x0, 0xff, val);
    flags.set_flag(21 - idx, of);

    val as u8
}

/// Truncate `val` and check for overflow.
#[inline]
fn saturate_to_mac(flags: &mut Flags, idx: usize, shift: u8, val: i64) -> i32 {
    check_mac_overflow(flags, idx, val);
    (val >> shift) as i32
}

/// Truncate `val` and check for overflow.
#[inline]
fn saturate_to_mac0(flags: &mut Flags, val: i64) -> i32 {
    check_mac0_overflow(flags, val);
    val as i32
}

/// Saturate `val` to `ir` registers.
#[inline]
fn saturate_to_ir(flags: &mut Flags, idx: usize, clamp: bool, val: i32) -> i16 {
    let min = if clamp { 0 } else { -(1 << 15) };
    let max = (1 << 15) - 1;

    let (val, of) = saturate(min, max, val);
    flags.set_flag(24 - idx, of);

    val as i16
}

/// Saturate `val` to the `ir0` register.
#[inline]
fn saturate_to_ir0(flags: &mut Flags, val: i32) -> i16 {
    let (val, of) = saturate(0x0, 0x1000, val);

    flags.set_flag(12, of);
    val as i16
}

/// Flags register.
#[derive(Default)]
struct Flags(u32);

impl Flags {
    fn set_flag(&mut self, flag: usize, val: bool) {
        self.0.set_bit(flag, val);
    }

    fn clear(&mut self) {
        self.0 = 0;
    }
}

#[derive(Clone, Copy)]
struct Opcode(u32);

impl Opcode {
    fn cmd(self) -> u32 {
        self.0.bit_range(0, 5)
    }

    fn clamp(self) -> bool {
        self.0.bit(10)
    }

    fn shift(self) -> u8 {
        self.0.bit(19) as u8 * 12
    }

    fn mat(self) -> u32 {
        self.0.bit_range(17, 18)
    }

    fn vec(self) -> u32 {
        self.0.bit_range(15, 16)
    }

    fn tr_vec(self) -> u32 {
        self.0.bit_range(13, 14)
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.cmd() {
            0x01 => f.write_str("rtps"),
            0x06 => f.write_str("nclip"),
            0x0c => f.write_str("op"),
            0x10 => f.write_str("dpcs"),
            0x11 => f.write_str("intp"),
            0x12 => f.write_str("mvmva"),
            0x13 => f.write_str("ncds"),
            0x16 => f.write_str("ncdt"),
            0x1b => f.write_str("nccs"),
            0x1c => f.write_str("cc"),
            0x1e => f.write_str("ncs"),
            0x20 => f.write_str("nct"),
            0x28 => f.write_str("sqr"),
            0x29 => f.write_str("dcpl"),
            0x2a => f.write_str("dpct"),
            0x2d => f.write_str("avsz3"),
            0x2e => f.write_str("avsz4"),
            0x30 => f.write_str("rtpt"),
            0x3d => f.write_str("gpf"),
            0x3e => f.write_str("gpl"),
            0x3f => f.write_str("ncct"),
            _ => f.write_str("invalid"),
        }
    }
}

/// Because the GTE uses fixed point integers, division has to be calculated manually using Newton-
/// Raphson. This is mostly taken from Mednafen.
///
/// [wiki]: https://en.wikipedia.org/wiki/Division_algorithm#Newton–Raphson_division
fn nr_divide(lhs: u16, rhs: u16) -> u32 {
    let shift = rhs.leading_zeros();

    let lhs = (lhs as u64) << shift;
    let rhs = rhs << shift;

    let reciprocal = {
        let idx = ((rhs & 0x7fff) + 0x40) >> 7;
        let factor = FACTOR_TABLE[idx as usize] as i32 + 0x101;

        let rhs = (rhs | 0x8000) as i32;
        let tmp = ((rhs * -factor) + 0x80) >> 8;

        (((factor * (0x20000 + tmp)) + 0x80) >> 8) as u64
    };

    ((lhs * reciprocal + 0x8000) >> 16).min(0x1ffff) as u32
}

const FACTOR_TABLE: [u8; 0x101] = [
    0xff, 0xfd, 0xfb, 0xf9, 0xf7, 0xf5, 0xf3, 0xf1, 0xef, 0xee, 0xec, 0xea, 0xe8, 0xe6, 0xe4, 0xe3,
    0xe1, 0xdf, 0xdd, 0xdc, 0xda, 0xd8, 0xd6, 0xd5, 0xd3, 0xd1, 0xd0, 0xce, 0xcd, 0xcb, 0xc9, 0xc8,
    0xc6, 0xc5, 0xc3, 0xc1, 0xc0, 0xbe, 0xbd, 0xbb, 0xba, 0xb8, 0xb7, 0xb5, 0xb4, 0xb2, 0xb1, 0xb0,
    0xae, 0xad, 0xab, 0xaa, 0xa9, 0xa7, 0xa6, 0xa4, 0xa3, 0xa2, 0xa0, 0x9f, 0x9e, 0x9c, 0x9b, 0x9a,
    0x99, 0x97, 0x96, 0x95, 0x94, 0x92, 0x91, 0x90, 0x8f, 0x8d, 0x8c, 0x8b, 0x8a, 0x89, 0x87, 0x86,
    0x85, 0x84, 0x83, 0x82, 0x81, 0x7f, 0x7e, 0x7d, 0x7c, 0x7b, 0x7a, 0x79, 0x78, 0x77, 0x75, 0x74,
    0x73, 0x72, 0x71, 0x70, 0x6f, 0x6e, 0x6d, 0x6c, 0x6b, 0x6a, 0x69, 0x68, 0x67, 0x66, 0x65, 0x64,
    0x63, 0x62, 0x61, 0x60, 0x5f, 0x5e, 0x5d, 0x5d, 0x5c, 0x5b, 0x5a, 0x59, 0x58, 0x57, 0x56, 0x55,
    0x54, 0x53, 0x53, 0x52, 0x51, 0x50, 0x4f, 0x4e, 0x4d, 0x4d, 0x4c, 0x4b, 0x4a, 0x49, 0x48, 0x48,
    0x47, 0x46, 0x45, 0x44, 0x43, 0x43, 0x42, 0x41, 0x40, 0x3f, 0x3f, 0x3e, 0x3d, 0x3c, 0x3c, 0x3b,
    0x3a, 0x39, 0x39, 0x38, 0x37, 0x36, 0x36, 0x35, 0x34, 0x33, 0x33, 0x32, 0x31, 0x31, 0x30, 0x2f,
    0x2e, 0x2e, 0x2d, 0x2c, 0x2c, 0x2b, 0x2a, 0x2a, 0x29, 0x28, 0x28, 0x27, 0x26, 0x26, 0x25, 0x24,
    0x24, 0x23, 0x22, 0x22, 0x21, 0x20, 0x20, 0x1f, 0x1e, 0x1e, 0x1d, 0x1d, 0x1c, 0x1b, 0x1b, 0x1a,
    0x19, 0x19, 0x18, 0x18, 0x17, 0x16, 0x16, 0x15, 0x15, 0x14, 0x14, 0x13, 0x12, 0x12, 0x11, 0x11,
    0x10, 0x0f, 0x0f, 0x0e, 0x0e, 0x0d, 0x0d, 0x0c, 0x0c, 0x0b, 0x0a, 0x0a, 0x09, 0x09, 0x08, 0x08,
    0x07, 0x07, 0x06, 0x06, 0x05, 0x05, 0x04, 0x04, 0x03, 0x03, 0x02, 0x02, 0x01, 0x01, 0x00, 0x00,
    0x00,
];

#[test]
fn test_reg_size() {
    assert_eq!(std::mem::size_of::<DataRegs>(), 128);
    assert_eq!(std::mem::size_of::<ControlRegs>(), 128);
}
