use crate::bus::{ram::Ram, regioned_addr, BusMap};

use thiserror::Error;
use bytemuck::{Zeroable, AnyBitPattern};

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

#[derive(Error, Debug)]
pub enum ExeError {
    #[error("failed to load exe: {0}")]
    IoError(#[from] io::Error),

    #[error("invalid header: {0}")]
    InvalidHeader(String),
}

/// Loaded PSX EXE file.
pub struct Exe {
    /// Text segment data.
    pub text: Box<[u8]>,
    /// Program counter.
    pub pc: u32,
    /// Global counter.
    pub gp: u32,
    /// Size of text segment.
    pub text_size: u32,
    /// Base address of text segment.
    pub text_base: u32,
    /// Size of bss segment.
    pub bss_size: u32,
    /// Base address of bss segment.
    pub bss_base: u32,
    /// Stack pointer.
    pub sp: u32,
}

impl Exe {
    /// Load and parse from file.
    pub fn load(path: &Path) -> Result<Self, ExeError> {
        let mut file = File::open(path)?;
        let mut data = Vec::<u8>::default();
        
        file.read_to_end(&mut data)?;

        let data = data.into_boxed_slice();

        if data.len() < 0x800 {
            return Err(ExeError::InvalidHeader(
                format!("must be at least 2 kilobytes, is {} bytes", data.len())
            ));
        }

        let header: &Header = bytemuck::from_bytes(&data[..std::mem::size_of::<Header>()]);

        if header.magic != b"PS-X EXE".as_slice() {
            return Err(ExeError::InvalidHeader(
                String::from("invalid magic value, must be 'PS-X EXE'")
            ));
        }

        let text_base = regioned_addr(header.text_base);
        if !Ram::contains(text_base) || !Ram::contains(text_base + header.text_size) {
            return Err(ExeError::InvalidHeader(
                String::from("text segment not contained in RAM")
            ));
        }

        let bss_base = regioned_addr(header.bss_base);
        if !Ram::contains(bss_base) || !Ram::contains(bss_base + header.bss_size) {
            return Err(ExeError::InvalidHeader(
                String::from("bss segment not contained in RAM")
            ));
        }

        Ok(Self {
            pc: header.pc,
            gp: header.gp,
            sp: header.sp_base + header.sp_offset,
            text_base: header.text_base,
            text_size: header.text_size.min(data.len() as u32 - 0x800),
            bss_size: header.bss_size,
            bss_base: header.bss_base,
            text: Box::from(&data[0x800..]),
        })
    }
}

/// Header for PSX EXE files. The actual header is 2048 bytes, but we only care about the first
/// part of it.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Header {
    /// Magic value, should be "PS-X EXE".
    magic: [u8; 8], 
    _pad0: [u8; 8],
    /// Program counter.
    pc: u32,
    /// Global pointer.
    gp: u32,
    /// Base address of the text segment.
    text_base: u32,
    /// Size of the the text segment. Should be the size of the file minus the 0x800 byte header.
    text_size: u32,
    _pad1: [u8; 8],
    /// Base address of bss segment (zero initialized).
    bss_base: u32,
    /// Size of the bss segment (zero initialized).
    bss_size: u32,
    /// Stack pointer base.
    sp_base: u32,
    /// Stack pointer offset.
    sp_offset: u32,
}

unsafe impl Zeroable for Header {}

unsafe impl AnyBitPattern for Header {}
