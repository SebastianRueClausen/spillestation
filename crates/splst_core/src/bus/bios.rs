use splst_asm::{InsTy, Register, assemble_ins};
use super::{AddrUnit, BusMap};
use crate::exe::Exe;

use thiserror::Error;

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Error, Debug)]
pub enum BiosError {
    #[error("failed to load BIOS: {0}")]
    IoError(#[from] io::Error),

    #[error("invalid BIOS file: must be 512 kb, is {0} bytes")]
    InvalidSize(usize),
}

pub struct Bios {
    data: Box<[u8]>,
    path: PathBuf,
    name: String, 
}

impl Bios {
    pub const SIZE: usize = 1024 * 512;

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn from_file(path: &Path) -> Result<Self, BiosError> {
        let mut file = File::open(path)?;
        let mut data = Vec::<u8>::with_capacity(Self::SIZE);

        file.read_to_end(&mut data)?;

        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .to_string();

        if data.len() != Self::SIZE {
            Err(BiosError::InvalidSize(data.len()))
        } else {
            Ok(Self::new(data.into_boxed_slice(), path, name))
        }
    }

    #[cfg(test)]
    pub fn from_code(base: u32, code: &[u8]) -> Self {
        let base = super::regioned_addr(base);
        debug_assert!(Self::contains(base));

        let base = (base - Self::BUS_BEGIN) as usize;

        debug_assert!(base + code.len() <= Self::SIZE);

        let mut data = [0x0; Self::SIZE];
        for (i, byte) in code.iter().enumerate() {
            data[i + base] = *byte;
        }

        Self::new(Box::from(data), &Path::new(""), "custom".to_string())
    }

    pub fn new(data: Box<[u8]>, path: &Path, name: String) -> Self {
        Self {
            data,
            name,
            path: path.to_path_buf(),
        }
    }

    /// Load value from bios.
    pub fn load<T: AddrUnit>(&self, addr: u32) -> T {
        let val: u32 = (0..T::WIDTH as usize).fold(0, |val, byte| {
            val | (self.data[addr as usize + byte] as u32) << (8 * byte)
        });
        T::from_u32(val)
    }

    pub fn patch_for_exe(&mut self, exe: &Exe) {
        let mut ins = vec![
            InsTy::Label("main"),
            InsTy::Li(Register::T0, exe.pc),
            InsTy::Li(Register::GP, exe.gp),
        ]; 

        if exe.sp != 0 {
            ins.extend_from_slice(&[
                InsTy::Li(Register::SP, exe.sp),
                InsTy::Lui(Register::FP, exe.sp >> 16),
                InsTy::Jr(Register::T0),
                InsTy::Ori(Register::FP, Register::FP, exe.sp & 0xffff),
            ])
        } else {
            ins.extend_from_slice(&[
                InsTy::Nop,
                InsTy::Nop,
                InsTy::Nop,
                InsTy::Jr(Register::T0),
                InsTy::Nop,
            ])
        };

        let (code, base) = assemble_ins(0xbfc06ff0, ins.into_iter()).unwrap();

        self.patch(&code, base);
    }

    fn patch(&mut self, code: &[u8], base: u32) {
        let base = super::regioned_addr(base);
        let offset = Self::offset(base + code.len() as u32)
            .and(Self::offset(base))
            .expect("trying to path at address outside BIOS") as usize;

        for (i, byte) in code.iter().enumerate() {
            self.data[offset + i] = *byte;
        }
    }
}

impl BusMap for Bios {
    const BUS_BEGIN: u32 = 0x1fc00000;
    const BUS_END: u32 = Self::BUS_BEGIN + Self::SIZE as u32 - 1;
}
