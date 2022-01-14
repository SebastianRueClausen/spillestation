#![allow(dead_code)]

use crate::util::BitExtract;

/// Each element is 16 bits. 1 bit sign, 3 bit integer, 12 bit fraction.
type Mat3 = [[i16; 3]; 3];

/// 32 bit vector.
type Vec32 = [i32; 3];

/// 16 bit vector.
type Vec16 = [i16; 3];

pub struct Gte {
    /// ['MulMat'] matrices.
    mul_mats: [Mat3; 3],
    /// ['TransVec'] vectors.
    trans_vecs: [Vec32; 3],
    /// ['MulVec'] vectors.
    vecs: [Vec16; 4],
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
    /// Average Z(depth ordering) value.
    otz: u16,
    /// Screen XY FIFO.
    sxy_fifo: [i16; 4],
    /// Screen Z FIFO.
    sz_fifo: [i16; 4],
    /// Color FIFO. RGB0, RGB1 and RGB2.
    rgb_fifo: [[u8; 4]; 3],
    /// Color register.
    rgb: [u8; 4],
    /// Interpolation factor.
    ir0: i16,
    /// Intermediate results.
    mac: [i32; 4],
}

impl Gte {
    pub fn new() -> Self {
        Self {
            mul_mats: [[[0x0; 3]; 3]; 3],
            trans_vecs: [[0x0; 3]; 3],
            vecs: [[0x0; 3]; 4],
            ofx: 0,
            ofy: 0,
            h: 0,
            dqa: 0,
            dqb: 0,
            zsf3: 0,
            zsf4: 0,
            otz: 0,
            sxy_fifo: [0x0; 4],
            sz_fifo: [0x0; 4],
            rgb_fifo: [[0x0; 4]; 3],
            rgb: [0x0; 4],
            ir0: 0x0,
            mac: [0x0; 4],
        }
    }

    pub fn cmd(&mut self, cmd: u32) {
        let op = Opcode(cmd);
        todo!("GTE Command: {:08x}", op.cmd());
    }

    pub fn ctrl_store(&mut self, reg: u32, val: u32) {
        trace!("GTE Control store to reg {:x}", reg);
        match reg {
            13..=15 => {
                let vec = TransVec::BackgroundColor as usize;
                self.trans_vecs[vec][reg as usize - 13] = val as i32;
            }
            21..=23 => {
                let vec = TransVec::FarColor as usize;
                self.trans_vecs[vec][reg as usize - 21] = val as i32;
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
    IR = 3,
}

#[derive(Clone, Copy)]
enum TransVec {
    Translation = 0,
    BackgroundColor = 1,
    FarColor = 2,
    Zero = 3,
}

#[derive(Clone, Copy)]
struct Opcode(u32);

impl Opcode {
    /// The command number itself.
    fn cmd(self) -> u32 {
        self.0.extract_bits(0, 5)
    }

    /// If IR1, IR2 and IR2 results should clamp between -0x8000..0x7fff or 0..0x7fff.
    fn ir_clamp(self) -> bool {
        self.0.extract_bit(10) == 1
    }

    fn ir_frac(self) -> bool {
        self.0.extract_bit(19) == 1
    }

    /// The matrix to be operated on.
    fn mul_mat(self) -> MulMat {
        match self.0.extract_bits(17, 18) {
            0 => MulMat::Reserved,
            1 => MulMat::Light,
            2 => MulMat::LightColor,
            3 => MulMat::Reserved,
            _ => unreachable!(),
        }
    }

    fn mul_vec(self) -> MulVec {
        match self.0.extract_bits(15, 16) {
            0 => MulVec::V0,
            1 => MulVec::V1,
            2 => MulVec::V2,
            3 => MulVec::IR,
            _ => unreachable!(),
        }
    }

    fn trans_vec(self) -> TransVec {
        match self.0.extract_bits(13, 14) {
            0 => TransVec::Translation,
            1 => TransVec::BackgroundColor,
            2 => TransVec::FarColor,
            3 => TransVec::Zero,
            _ => unreachable!(),
        }
    }
}
