#![allow(dead_code)]
//! # TODO
//! 
//! - Change store and load to work like in the SPU, where a big struct with all the registers
//!   are cast into a byte array and simply assigned to.
//! 
//! - SIMD optimize.


use splst_util::{Bit, BitSet};

/// Each element is 16 bits. 1 bit sign, 3 bit integer, 12 bit fraction.
type Mat3 = [[i16; 3]; 3];
type Vec32 = [i32; 3];
type Vec16 = [i16; 3];
type Rgbx = [u8; 4];

pub struct Gte {
    /// Flags register.
    flags: u32,
    /// ['MulMat'] matrices.
    mul_mats: [Mat3; 3],
    /// ['TransVec'] vectors.
    trans_vecs: [Vec32; 3],
    /// ['MulVec'] vectors.
    vecs: [Vec16; 3],
    /// X screen offset.
    ofx: i32,
    /// Y screen offset.
    ofy: i32,
    /// Projection plane distance.
    h: u16,
    /// Depth queing parameter A.
    dqa: i16,
    /// Depth queing parameter B.
    dqb: i32,
    /// Z3 average scale factor.
    zsf3: i16,
    /// Z4 average scale factor.
    zsf4: i16,
    /// Average Z (depth ordering) value.
    otz: u16,
    /// Screen XY FIFO.
    sxy_fifo: [(i16, i16); 4],
    /// Screen Z FIFO.
    sz_fifo: [u16; 4],
    /// Color FIFO. RGB0, RGB1 and RGB2.
    rgb_fifo: [Rgbx; 3],
    /// Color register.
    rgb: [u8; 4],
    /// Interpolation factor.
    ir: [i16; 4],
    /// Intermediate results.
    mac: [i32; 4],
    /// Count leading zeros or ones. This is the input value.
    lzcs: u32,
    /// The amount of leading zeroes or ones (depending on sign) in 'lzcs'.
    lzcr: u8,
}

impl Gte {
    pub fn new() -> Self {
        Self {
            flags: 0,
            mul_mats: [[[0x0; 3]; 3]; 3],
            trans_vecs: [[0x0; 3]; 3],
            vecs: [[0x0; 3]; 3],
            ofx: 0,
            ofy: 0,
            h: 0,
            dqa: 0,
            dqb: 0,
            zsf3: 0,
            zsf4: 0,
            otz: 0,
            sxy_fifo: [(0x0, 0x0); 4],
            sz_fifo: [0x0; 4],
            rgb_fifo: [[0x0; 4]; 3],
            rgb: [0x0; 4],
            ir: [0x0; 4],
            mac: [0x0; 4],
            lzcs: 0,
            lzcr: 32,
        }
    }

    pub fn cmd(&mut self, cmd: u32) {
        let op = Opcode(cmd);

        match op.cmd() {
            0x06 => self.cmd_nclip(op),
            0x13 => self.cmd_ncds(op),
            0x30 => self.cmd_rtpt(op),
            _ => todo!("GTE Command: {:08x}", op.cmd()),
        }

        
    }

    pub fn ctrl_store(&mut self, reg: u32, val: u32) {
        let reg = reg as usize;
        trace!("GTE Control store to reg {:x}", reg);
        match reg {
            0 => {
                let mat = MulMat::Rotation as usize;

                self.mul_mats[mat][0][0] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][0][1] = val.bit_range(16, 31) as i16;
            }
            1 => {
                let mat = MulMat::Rotation as usize;

                self.mul_mats[mat][0][2] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][1][0] = val.bit_range(16, 31) as i16;
            }
            2 => {
                let mat = MulMat::Rotation as usize;

                self.mul_mats[mat][1][1] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][1][2] = val.bit_range(16, 31) as i16;
            }
            3 => {
                let mat = MulMat::Rotation as usize;

                self.mul_mats[mat][2][0] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][2][1] = val.bit_range(16, 31) as i16;
            }
            4 => {
                let mat = MulMat::Rotation as usize;

                self.mul_mats[mat][2][2] = val.bit_range(00, 15) as i16;
            }
            5..=7 => {
                let vec = TransVec::Translation as usize;

                self.trans_vecs[vec][reg - 5] = val as i32;
            }
            8 => {
                let mat = MulMat::Light as usize;

                self.mul_mats[mat][0][0] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][0][1] = val.bit_range(16, 31) as i16;
            }
            9 => {
                let mat = MulMat::Light as usize;

                self.mul_mats[mat][0][2] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][1][0] = val.bit_range(16, 31) as i16;
            }
            10 => {
                let mat = MulMat::Light as usize;

                self.mul_mats[mat][1][1] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][1][2] = val.bit_range(16, 31) as i16;
            }
            11 => {
                let mat = MulMat::Light as usize;

                self.mul_mats[mat][2][0] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][2][1] = val.bit_range(16, 31) as i16;
            }
            12 => {
                let mat = MulMat::Light as usize;

                self.mul_mats[mat][2][2] = val.bit_range(00, 15) as i16;
            }
            13..=15 => {
                let vec = TransVec::BackgroundColor as usize;

                self.trans_vecs[vec][reg - 13] = val as i32;
            }
            16 => {
                let mat = MulMat::LightColor as usize;

                self.mul_mats[mat][0][0] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][0][1] = val.bit_range(16, 31) as i16;
            }
            17 => {
                let mat = MulMat::LightColor as usize;

                self.mul_mats[mat][0][2] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][1][0] = val.bit_range(16, 31) as i16;
            }
            18 => {
                let mat = MulMat::LightColor as usize;

                self.mul_mats[mat][1][1] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][1][2] = val.bit_range(16, 31) as i16;
            }
            19 => {
                let mat = MulMat::LightColor as usize;

                self.mul_mats[mat][2][0] = val.bit_range(00, 15) as i16;
                self.mul_mats[mat][2][1] = val.bit_range(16, 31) as i16;
            }
            20 => {
                let mat = MulMat::LightColor as usize;

                self.mul_mats[mat][2][2] = val.bit_range(00, 15) as i16;
            }
            21..=23 => {
                let vec = TransVec::FarColor as usize;

                self.trans_vecs[vec][reg - 21] = val as i32;
            }
            24 => self.ofx = val as i32,
            25 => self.ofy = val as i32,
            26 => self.h = val as u16,
            27 => self.dqa = val as i16,
            28 => self.dqb = val as i32,
            29 => self.zsf3 = val as i16,
            30 => self.zsf4 = val as i16,
            _ => todo!("GTE Control store: {}", reg),
        }
    }

    pub fn data_store(&mut self, reg: u32, val: u32) {
        match reg {
            0 => {
                self.vecs[0][0] = val.bit_range(0, 16) as i16;
                self.vecs[0][1] = val.bit_range(0, 16) as i16;
            }
            1 => {
                self.vecs[0][2] = val.bit_range(0, 16) as i16;
            }
            2 => {
                self.vecs[1][0] = val.bit_range(0, 16) as i16;
                self.vecs[1][1] = val.bit_range(0, 16) as i16;
            }
            3 => {
                self.vecs[1][2] = val.bit_range(0, 16) as i16;
            }
            4 => {
                self.vecs[2][0] = val.bit_range(0, 16) as i16;
                self.vecs[2][1] = val.bit_range(0, 16) as i16;
            }
            5 => {
                self.vecs[2][2] = val.bit_range(0, 16) as i16;
            }
            6 => self.rgb = u32_to_rgbx(val),
            n @ 08..=11 => self.ir[n as usize - 8] = val as i16,
            n @ 24..=27 => self.mac[n as usize - 24] = val as i32,
            30 => {
                self.lzcs = val;
                // 'val' is in this case a signed 32 bit int. If bit 31 is set, it's a negative
                // value, in which case the 'lzcr' counts the leading ones instead of leading
                // zeroes.
                self.lzcr = if val.bit(31) {
                    val.leading_ones() as u8
                } else {
                    val.leading_zeros() as u8
                };
            }
            _ => todo!("GTE data store in reg: {}", reg),
        }
    }

    pub fn ctrl_load(&mut self, reg: u32) -> u32 {
        match reg {
            31 => self.flags,
            _ => todo!("GTE control store in reg: {}", reg),
        }
    }

    pub fn data_load(&mut self, reg: u32) -> u32 {
        match reg {
            n @ 20..=22 => rgbx_to_u32(self.rgb_fifo[n as usize - 20]),
            n @ 24..=27 => self.mac[n as usize - 24] as u32,
            30 => self.lzcs,
            31 => self.lzcr.into(),
            _ => todo!("GTE data load in reg: {}", reg),
        }
    }
   
    /// Perform Rotation, translation and perspective on a given vector.
    fn rtp_vec(&mut self, op: Opcode, vec: MulVec) -> u32 {
        let vec = &mut self.vecs[vec as usize];

        let rt = &self.mul_mats[MulMat::Rotation as usize];
        let tr = &self.trans_vecs[TransVec::Translation as usize];

        // Perform translation and rotation on 'vec', this is done with the
        // formula: tr + vec * rt. Save the z value, which is the last.
        let z_shift = (0..3)
            .map(|r| {
                let val = (0..3).fold((tr[r] as i64) << 12, |acc, c| {
                    let val = vec[c] as i64;
                    let rot = rt[r][c] as i64;
                    
                    let val = acc + val * rot;

                    self.flags = self.flags
                        .set_bit(30 - c, val > 0x7ff_ffff_ffff)
                        .set_bit(27 - c, val < -0x800_0000_0000);

                    (val << 20) >> 20
                });

                // Save value to MAC. Shift the value 12 bits to the left if 'ir_frac' flag is set
                // in the opcode.
                self.mac[r + 1] = (val >> op.ir_shift()) as i32;

                (val >> 12) as i32
            })
            .last()
            .unwrap();
      
        // Set the IR vector with the values in MAC truncated to 16 bit signed integer.
        self.ir[1] = self.ir_clamp(0, self.mac[1], op);
        self.ir[2] = self.ir_clamp(1, self.mac[2], op);
        self.ir[3] = self.ir_clamp(2, self.mac[3], op);
       
        // So apparently there is a hardware bug here when clamping the z into the IR registers.
        // Checking for under/overflow is done with z shifted 12 bits. I just override
        // the flag bits here.
        self.flags = self.flags
            .set_bit(22, z_shift > i16::MIN as i32)
            .set_bit(22, z_shift < i16::MAX as i32);
     
        // Clamp the 'z_shift' value and push it onto the z FIFO.
        let z = z_shift.clamp(0, u16::MAX as i32) as u16;

        // Set flag 18 if z_shift over/underflows.
        self.flags = self.flags
            .set_bit(18, z_shift < 0)
            .set_bit(18, z_shift > i16::MAX as i32);

        self.push_to_sz_fifo(z);

        // Calculate projection factor.
        let pf = if z > self.h / 2 {
           nr_divide(self.h, z) as i64
        } else {
            self.flags = self.flags.set_bit(17, true);
            0x1ffff
        };
        
        let sx = self.ir[1] as i64 + pf + self.ofx as i64;
        let sy = self.ir[2] as i64 + pf + self.ofy as i64;

        self.flags = self.flags
            .set_bit(15, sx.min(sy) < -0x8000_0000)
            .set_bit(16, sx.max(sy) > -0x7fff_ffff);

        let xy = (
            self.sxy_clamp(0, sx as i32),
            self.sxy_clamp(1, sy as i32)
        );

        self.push_to_sxy_fifo(xy);

        pf as u32
    }

    fn mat_vec_mul(
        &mut self,
        mat: &Mat3,
        tr: &Vec32,
        vec: &Vec16,
        op: Opcode,
    ) {
        for r in 0..3 {
            let val = (0..3).fold((tr[r] as i64) << 12, |acc, c| {
                let val = acc + (vec[c] as i32 * mat[r][c] as i32) as i64;

                self.flags = self.flags
                    .set_bit(30 - c, val > 0x7ff_ffff_ffff)
                    .set_bit(27 - c, val < -0x800_0000_0000);

                (val << 20) >> 20
            });

            self.mac[r + 1] = (val >> op.ir_shift()) as i32;
        }

        self.ir[1] = self.ir_clamp(0, self.mac[1], op); 
        self.ir[2] = self.ir_clamp(1, self.mac[2], op); 
        self.ir[3] = self.ir_clamp(2, self.mac[3], op); 
    }

    fn ncd(&mut self, vec: MulVec, op: Opcode) {
        // Seems kinda wasteful to copy here, hopefully the compiler optimises it away.
        let mat = self.mul_mats[MulMat::Light as usize];
        let vec = self.vecs[vec as usize];

        self.mat_vec_mul(&mat, &[0x0; 3], &vec, op);

        let vec: Vec16 = [
            self.ir[1],
            self.ir[2],
            self.ir[3],
        ];
       
        let mat = self.mul_mats[MulMat::Light as usize];
        let tr = self.trans_vecs[TransVec::BackgroundColor as usize];

        self.mat_vec_mul(&mat, &tr, &vec, op);

        self.dcpl(op);
    }

    fn dcpl(&mut self, op: Opcode) {
        let fc = self.trans_vecs[TransVec::FarColor as usize];
        let rgb = &self.rgb.clone()[0..3];

        for (i, col) in rgb.iter().enumerate() {
            let fc = (fc[i] as i64) << 12;
            let ir = self.ir[i + 1] as i32;

            let shade = (((*col as i32) << 4) * ir) as i64;

            let ir = self.i44_clamp(i, fc - shade) >> op.ir_shift();
            let ir = self.ir_clamp(i, ir as i32, Opcode(0));

            let val = self.i44_clamp(i, shade + self.ir[0] as i64 * ir as i64);

            self.mac[i + 1] = (val >> op.ir_shift()) as i32;
        }

        self.ir[1] = self.ir_clamp(0, self.mac[1], op); 
        self.ir[2] = self.ir_clamp(1, self.mac[2], op); 
        self.ir[3] = self.ir_clamp(2, self.mac[3], op); 

        self.mac_to_rgb_fifo();
    }

    fn depth_queue(&mut self, pf: u32) {
        let depth = self.dqb as i64 + self.dqa as i64 * pf as i64;

        self.flags = self.flags
            .set_bit(15, depth < -0x8000_0000)
            .set_bit(16, depth > -0x7fff_ffff);

        self.mac[0] = depth as i32;

        // Compute the IR value.
        let depth = depth >> 12;

        let of = !(0..4096).contains(&depth);

        self.flags = self.flags.set_bit(12, of);
        self.ir[0] = depth.clamp(0, 4096) as i16;
    }

    /// Clamp value going into the IR registers.
    fn ir_clamp(&mut self, which: usize, val: i32, op: Opcode) -> i16 {
        // The command may specify that the IR values should be clamped to 0.
        let min = if op.ir_clamp() { 0 } else { i16::MIN } as i32;
        let max = i16::MAX as i32;
  
        self.flags = self.flags
            .set_bit(24 - which, val > max)
            .set_bit(24 - which, val < min);
       
        val.clamp(min, max) as i16
    }

    /// Clamp value going into sxy FIFO.
    fn sxy_clamp(&mut self, which: usize, val: i32) -> i16 {
        self.flags = self.flags
            .set_bit(14 - which, val > 0x3ff)
            .set_bit(14 - which, val < -0x400);
      
        val.clamp(-0x400, 0x3ff) as i16
    }

    fn i44_clamp(&mut self, which: usize, val: i64) -> i64 {
        self.flags = self.flags
            .set_bit(30 - which, val > 0x7ff_ffff_ffff)
            .set_bit(27 - which, val < -0x800_0000_0000);
        (val << 20) >> 20
    }

    fn push_to_sz_fifo(&mut self, val: u16) {
        self.sz_fifo[0] = self.sz_fifo[1];
        self.sz_fifo[1] = self.sz_fifo[2];
        self.sz_fifo[2] = self.sz_fifo[3];

        self.sz_fifo[3] = val;
    }

    fn push_to_sxy_fifo(&mut self, val: (i16, i16)) {
        self.sxy_fifo[0] = self.sxy_fifo[1];
        self.sxy_fifo[1] = self.sxy_fifo[2];
        self.sxy_fifo[2] = self.sxy_fifo[3];

        self.sxy_fifo[3] = val;
    }

    fn mac_to_rgb_fifo(&mut self) {
        let mut mac_to_color = |which: usize, mac: i32| {
            let color = mac >> 4;
            self.flags = self.flags
                .set_bit(21 - which, color < 0)
                .set_bit(21 - which, color > 0xff);
            color.clamp(0, 0xff) as u8
        };

        let m1 = self.mac[1];
        let m2 = self.mac[2];
        let m3 = self.mac[3];

        let color = [
            mac_to_color(0, m1),
            mac_to_color(1, m2),
            mac_to_color(2, m3),
            self.rgb[3],
        ];

        self.rgb_fifo[0] = self.rgb_fifo[1];
        self.rgb_fifo[1] = self.rgb_fifo[2];
        self.rgb_fifo[2] = color;
    }
}

impl Gte {
    /// Do rotation, translation and transformation on v0, v1 and v2.
    fn cmd_rtpt(&mut self, op: Opcode) {
        self.rtp_vec(op, MulVec::V0);
        self.rtp_vec(op, MulVec::V1);

        let pf = self.rtp_vec(op, MulVec::V2);

        self.depth_queue(pf);
    }

    fn cmd_ncds(&mut self, op: Opcode) {
        self.ncd(MulVec::V0, op);
    }

    /// Do normale clipping.
    fn cmd_nclip(&mut self, _: Opcode) {
        let (x0, y0) = (self.sxy_fifo[0].0 as i32, self.sxy_fifo[0].1 as i32);
        let (x1, y1) = (self.sxy_fifo[1].0 as i32, self.sxy_fifo[1].1 as i32);
        let (x2, y2) = (self.sxy_fifo[2].0 as i32, self.sxy_fifo[2].1 as i32);

        let v1 = x0 * (y1 - y2);
        let v2 = x1 * (y2 - y0);
        let v3 = x2 * (y0 - y1);

        let res = v1 as i64 + v2 as i64 + v3 as i64;

        // Check overflow.
        self.flags = self.flags
            .set_bit(15, res < -0x8000_0000)
            .set_bit(16, res > -0x7fff_ffff);
        
        self.mac[0] = res as i32;
    }
}

fn u32_to_rgbx(val: u32) -> Rgbx {
    let r = val.bit_range(0, 7) as u8;
    let g = val.bit_range(8, 15) as u8;
    let b = val.bit_range(16, 23) as u8;
    let x = val.bit_range(24, 31) as u8;
    [r, g, b, x]
}

fn rgbx_to_u32(val: Rgbx) -> u32 {
    0_u32
        .set_bit_range(00, 07, val[0] as u32)
        .set_bit_range(08, 15, val[1] as u32)
        .set_bit_range(16, 23, val[2] as u32)
        .set_bit_range(24, 31, val[3] as u32)
}

#[derive(Clone, Copy)]
enum MulMat {
   Rotation = 0,
   Light = 1,
   LightColor = 2,
   Reserved = 3,
}

#[derive(Clone, Copy)]
enum MulVec {
    V0 = 0,
    V1 = 1,
    V2 = 2,
}

#[derive(Clone, Copy)]
enum TransVec {
    Translation = 0,
    BackgroundColor = 1,
    FarColor = 2,
}

#[derive(Clone, Copy)]
struct Opcode(u32);

impl Opcode {
    /// The command number itself.
    fn cmd(self) -> u32 {
        self.0.bit_range(0, 5)
    }

    /// If IR1, IR2 and IR2 results should clamp between -0x8000..0x7fff or 0..0x7fff.
    fn ir_clamp(self) -> bool {
        self.0.bit(10)
    }

    fn ir_frac(self) -> bool {
        self.0.bit(19)
    }

    fn ir_shift(self) -> u32 {
        self.0.bit(19) as u32 * 12
    }

    /// The matrix to be operated on.
    fn mul_mat(self) -> MulMat {
        match self.0.bit_range(17, 18) {
            0 => MulMat::Reserved,
            1 => MulMat::Light,
            2 => MulMat::LightColor,
            3 => MulMat::Reserved,
            _ => unreachable!(),
        }
    }

    fn mul_vec(self) -> MulVec {
        match self.0.bit_range(15, 16) {
            0 => MulVec::V0,
            1 => MulVec::V1,
            2 => MulVec::V2,
            _ => unreachable!(),
        }
    }

    fn trans_vec(self) -> TransVec {
        match self.0.bit_range(13, 14) {
            0 => TransVec::Translation,
            1 => TransVec::BackgroundColor,
            2 => TransVec::FarColor,
            _ => unreachable!(),
        }
    }
}

/// Because the GTE uses fixed point integers, division has to be calculated manually using Newton-
/// Raphson. This is mostly taken from Mednafen.
///
/// https://en.wikipedia.org/wiki/Division_algorithm#Newton–Raphson_division
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
    0xff, 0xfd, 0xfb, 0xf9, 0xf7, 0xf5, 0xf3, 0xf1, 0xef, 0xee, 0xec, 0xea,
    0xe8, 0xe6, 0xe4, 0xe3, 0xe1, 0xdf, 0xdd, 0xdc, 0xda, 0xd8, 0xd6, 0xd5,
    0xd3, 0xd1, 0xd0, 0xce, 0xcd, 0xcb, 0xc9, 0xc8, 0xc6, 0xc5, 0xc3, 0xc1,
    0xc0, 0xbe, 0xbd, 0xbb, 0xba, 0xb8, 0xb7, 0xb5, 0xb4, 0xb2, 0xb1, 0xb0,
    0xae, 0xad, 0xab, 0xaa, 0xa9, 0xa7, 0xa6, 0xa4, 0xa3, 0xa2, 0xa0, 0x9f,
    0x9e, 0x9c, 0x9b, 0x9a, 0x99, 0x97, 0x96, 0x95, 0x94, 0x92, 0x91, 0x90,
    0x8f, 0x8d, 0x8c, 0x8b, 0x8a, 0x89, 0x87, 0x86, 0x85, 0x84, 0x83, 0x82,
    0x81, 0x7f, 0x7e, 0x7d, 0x7c, 0x7b, 0x7a, 0x79, 0x78, 0x77, 0x75, 0x74,
    0x73, 0x72, 0x71, 0x70, 0x6f, 0x6e, 0x6d, 0x6c, 0x6b, 0x6a, 0x69, 0x68,
    0x67, 0x66, 0x65, 0x64, 0x63, 0x62, 0x61, 0x60, 0x5f, 0x5e, 0x5d, 0x5d,
    0x5c, 0x5b, 0x5a, 0x59, 0x58, 0x57, 0x56, 0x55, 0x54, 0x53, 0x53, 0x52,
    0x51, 0x50, 0x4f, 0x4e, 0x4d, 0x4d, 0x4c, 0x4b, 0x4a, 0x49, 0x48, 0x48,
    0x47, 0x46, 0x45, 0x44, 0x43, 0x43, 0x42, 0x41, 0x40, 0x3f, 0x3f, 0x3e,
    0x3d, 0x3c, 0x3c, 0x3b, 0x3a, 0x39, 0x39, 0x38, 0x37, 0x36, 0x36, 0x35,
    0x34, 0x33, 0x33, 0x32, 0x31, 0x31, 0x30, 0x2f, 0x2e, 0x2e, 0x2d, 0x2c,
    0x2c, 0x2b, 0x2a, 0x2a, 0x29, 0x28, 0x28, 0x27, 0x26, 0x26, 0x25, 0x24,
    0x24, 0x23, 0x22, 0x22, 0x21, 0x20, 0x20, 0x1f, 0x1e, 0x1e, 0x1d, 0x1d,
    0x1c, 0x1b, 0x1b, 0x1a, 0x19, 0x19, 0x18, 0x18, 0x17, 0x16, 0x16, 0x15,
    0x15, 0x14, 0x14, 0x13, 0x12, 0x12, 0x11, 0x11, 0x10, 0x0f, 0x0f, 0x0e,
    0x0e, 0x0d, 0x0d, 0x0c, 0x0c, 0x0b, 0x0a, 0x0a, 0x09, 0x09, 0x08, 0x08,
    0x07, 0x07, 0x06, 0x06, 0x05, 0x05, 0x04, 0x04, 0x03, 0x03, 0x02, 0x02,
    0x01, 0x01, 0x00, 0x00, 0x00,
];


