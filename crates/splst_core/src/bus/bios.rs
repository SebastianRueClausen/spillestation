use super::{AddrUnit, BusMap};
use thiserror::Error;

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Error, Debug)]
pub enum BiosError {
    #[error("Failed to load BIOS: {0}")]
    IoError(#[from] io::Error),

    #[error("Invalid BIOS file: must be 512 kb, is {0} bytes")]
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
        debug_assert!((Self::BUS_BEGIN..=Self::BUS_END).contains(&base));

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

    pub fn load<T: AddrUnit>(&mut self, addr: u32) -> u32 {
        (0..T::WIDTH).fold(0, |value, byte| {
            value | (self.data[addr as usize + byte] as u32) << (8 * byte)
        })
    }
}

impl BusMap for Bios {
    const BUS_BEGIN: u32 = 0x1fc00000;
    const BUS_END: u32 = Self::BUS_BEGIN + Self::SIZE as u32 - 1;
}
