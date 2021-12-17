use super::AddrUnit;
use std::fs::File;
use std::io::{Read, Error as IoError};
use std::fmt;
use std::path::Path;

/// Bios always takes up 512 kilobytes.
pub const BIOS_SIZE: usize = 1024 * 512;

pub enum BiosError {
    IoError(IoError),
    InvalidSize(usize),
}

impl From<IoError> for BiosError {
    fn from(err: IoError) -> Self {
        BiosError::IoError(err)
    }
}

impl fmt::Display for BiosError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BiosError::IoError(ref err) => {
                write!(f, "Failed to load BIOS: {}", err)
            },
            BiosError::InvalidSize(size) => {
                write!(f, "BIOS must be {} bytes, not {}", BIOS_SIZE, size)
            },
        }
    }
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

    pub fn new(bytes: Box<[u8]>) -> Self {
        Self { data: bytes }
    }

    pub fn load<T: AddrUnit>(&self, offset: u32) -> u32 {
        let mut value: u32 = 0;
        for i in 0..T::width() {
            value |= (self.data[offset as usize + i] as u32) << (8 * i);
        }
        value
    }
}
