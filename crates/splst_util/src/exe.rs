use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

#[derive(thiserror::Error, Debug)]
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
    /// Stack pointer. Set to `None` if the stack pointer shouldn't change.
    pub sp: Option<u32>,
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

        if regioned_addr(header.text_base + header.text_size) > 1024 * 1024 * 2 {
            return Err(ExeError::InvalidHeader(
                String::from("text segment not contained in RAM")
            ));
        }

        if regioned_addr(header.bss_base + header.bss_size) > 1024 * 1024 * 2 {
            return Err(ExeError::InvalidHeader(
                String::from("bss segment not contained in RAM")
            ));
        }

        let sp = if header.sp_base == 0 { 
            None
        } else {
            Some(header.sp_base + header.sp_offset)
        };

        Ok(Self {
            sp,
            pc: header.pc,
            gp: header.gp,
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

unsafe impl bytemuck::Zeroable for Header {}

unsafe impl bytemuck::AnyBitPattern for Header {}

fn regioned_addr(addr: u32) -> u32 {
    const REGION_MAP: [u32; 8] = [
        0xffff_ffff,
        0xffff_ffff,
        0xffff_ffff,
        0xffff_ffff,
        0x7fff_ffff,
        0x1fff_ffff,
        0xffff_ffff,
        0xffff_ffff,
    ];
    addr & REGION_MAP[(addr >> 29) as usize]
}
