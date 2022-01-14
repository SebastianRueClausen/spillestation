use super::{AddrUnit, BusMap};
use std::{fs::File, io::{self, Read}, path::Path};
use thiserror::Error;

/// Bios always takes up 512 kilobytes.
pub const BIOS_SIZE: usize = 1024 * 512;

#[derive(Error, Debug)]
pub enum BiosError {
    #[error("Failed to load BIOS: {0}")]
    IoError(#[from] io::Error),
    #[error("Invalid BIOS file: must be 512 kb, is {0} bytes")]
    InvalidSize(usize),
}

pub struct Bios {
    data: Box<[u8]>,
}

impl Bios {
    pub fn from_file(path: &Path) -> Result<Self, BiosError> {
        let mut file = File::open(path)?;
        let mut data = Vec::<u8>::with_capacity(BIOS_SIZE);
        file.read_to_end(&mut data)?;
        if data.len() != BIOS_SIZE {
            Err(BiosError::InvalidSize(data.len()))
        } else {
            Ok(Self::new(data.into_boxed_slice()))
        }
    }

    pub fn new(data: Box<[u8]>) -> Self {
        Self { data }
    }

    pub fn load<T: AddrUnit>(&mut self, addr: u32) -> u32 {
        (0..T::WIDTH).fold(0, |value, byte| {
            value | (self.data[addr as usize + byte] as u32) << (8 * byte)
        })
    }
}

impl BusMap for Bios {
    const BUS_BEGIN: u32 = 0x1fc00000;
    const BUS_END: u32 = Self::BUS_BEGIN + BIOS_SIZE as u32 - 1;
}
