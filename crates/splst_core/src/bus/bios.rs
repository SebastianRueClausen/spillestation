use splst_asm::{InsTy, Register, assemble_ins};
use splst_util::Exe;
use super::{AddrUnit, BusMap};

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
    #[inline]
    pub fn load<T: AddrUnit>(&self, addr: u32) -> T {
        let val: u32 = (0..T::WIDTH as usize).fold(0, |val, byte| {
            val | (self.data[addr as usize + byte] as u32) << (8 * byte)
        });
        T::from_u32(val)
    }

    #[inline]
    pub unsafe fn load_unchecked<T: AddrUnit>(&self, addr: u32) -> T {
        let val: u32 = (0..T::WIDTH as usize).fold(0, |val, byte| {
            let get = *self.data.get_unchecked(addr as usize + byte) as u32;
            val | get << (8 * byte)
        });
        T::from_u32(val)
    }

    pub fn patch_for_exe(&mut self, exe: &Exe) {
        let mut ins = vec![
            InsTy::Label("main"),
            InsTy::Li(Register::T0, exe.pc),
            InsTy::Li(Register::GP, exe.gp),
        ]; 

        if let Some(sp) = exe.sp {
            ins.extend_from_slice(&[
                InsTy::Li(Register::SP, sp),
                InsTy::Lui(Register::FP, sp >> 16),
                InsTy::Jr(Register::T0),
                InsTy::Ori(Register::FP, Register::FP, sp & 0xffff),
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

        let (code, base) = assemble_ins(0xbfc06ff0, &ins).unwrap();

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

mod fns {
    #![allow(dead_code)]

    pub fn get_a0_func(id: u32) -> Option<&'static Func> {
        A0_FUNCS
            .as_slice()
            .binary_search_by(|(num, _)| u32::from(*num).cmp(&id))
            .map(|idx| A0_FUNCS[idx].1)
            .ok()
    }

    pub enum ArgType {
        Str,
        Char,
        Int,
        Ptr,
    }

    pub struct Arg {
        pub name: &'static str,
        pub kind: ArgType,
    }

    impl Arg {
        const fn new(name: &'static str, kind: ArgType) -> Self {
            Self { name, kind }
        }
    }

    pub struct Func {
        pub name: &'static str,
        pub args: &'static [Arg],
    }

    const A0_FUNCS: [(u8, &'static Func); 0x39] = [
        (0x00, &FILE_OPEN),
        (0x01, &FILE_SEEK),
        (0x02, &FILE_READ),
        (0x03, &FILE_WRITE),
        (0x04, &FILE_CLOSE),
        (0x05, &FILE_IOCTL),
        (0x06, &EXIT),
        (0x07, &FILE_GET_DEVICE_FLAG),

        (0x08, &FILE_GETC),
        (0x09, &FILE_PUTC),
        (0x0a, &TODIGIT),
        (0x0b, &ATOF),
        (0x0c, &STRTOUL),
        (0x0d, &STRTOL),
        (0x0e, &ABS),
        (0x0f, &LABS),

        (0x10, &ATOI),
        (0x11, &ATOL),
        (0x12, &ATOB),
        (0x13, &SAVE_STATE),
        (0x14, &RESTORE_STATE),
        (0x15, &STRCAT),
        (0x16, &STRNCAT),
        (0x17, &STRCMP),

        (0x18, &STRNCMP),
        (0x19, &STRCPY),
        (0x1a, &STRNCPY),
        (0x1b, &STRLEN),
        (0x1c, &INDEX),
        (0x1d, &RINDEX),
        (0x1e, &STRCHR),
        (0x1f, &STRRCHR),

        (0x20, &STRPBRK),
        (0x21, &STRSPN),
        (0x22, &STRCSPN),
        (0x23, &STRTOK),
        (0x24, &STRSTR),
        (0x25, &TOUPPER),
        (0x26, &TOLOWER),
        (0x27, &BCOPY),

        (0x28, &BZERO),
        (0x29, &BCMP),
        (0x2a, &MEMCPY),
        (0x2b, &MEMSET),
        (0x2c, &MEMMOVE),
        (0x2d, &MEMCMP),
        (0x2e, &MEMCHR),
        (0x2f, &RAND),

        (0x30, &SRAND),
        (0x31, &QSORT),
        (0x32, &STROD),
        (0x33, &MALLOC),
        (0x34, &FREE),
        (0x35, &LSEARCH),
        (0x36, &BSEARCH),
        (0x37, &CALLOC),
        (0x38, &REALLOC),
    ];

    const FILE_OPEN: Func = Func {
        name: "FileOpen",
        args: &[Arg::new("filename", ArgType::Str), Arg::new("accessmode", ArgType::Int)],
    };

    const FILE_SEEK: Func = Func {
        name: "FileSeek",
        args: &[
            Arg::new("file", ArgType::Ptr),
            Arg::new("offset", ArgType::Int),
            Arg::new("origin", ArgType::Int),
        ],
    };
     
    const FILE_READ: Func = Func {
        name: "FileRead",
        args: &[
            Arg::new("file", ArgType::Ptr),
            Arg::new("dst", ArgType::Ptr),
            Arg::new("length", ArgType::Int),
        ],
    };

    const FILE_WRITE: Func = Func {
        name: "FileWrite",
        args: &[
            Arg::new("file", ArgType::Ptr),
            Arg::new("src", ArgType::Ptr),
            Arg::new("length", ArgType::Int),
        ],
    };

    const FILE_CLOSE: Func = Func {
        name: "FileClose",
        args: &[Arg::new("file", ArgType::Ptr)],
    };

    const FILE_IOCTL: Func = Func {
        name: "FileIoctl",
        args: &[
            Arg::new("file", ArgType::Ptr),
            Arg::new("cmd", ArgType::Int),
            Arg::new("arg", ArgType::Int),
        ],
    };

    const EXIT: Func = Func {
        name: "exit",
        args: &[Arg::new("exitcode", ArgType::Int)],
    };

    const FILE_GET_DEVICE_FLAG: Func = Func {
        name: "FileGetDeviceFlag",
        args: &[Arg::new("file", ArgType::Ptr)],
    };


    const FILE_GETC: Func = Func {
        name: "FileGetc",
        args: &[Arg::new("file", ArgType::Ptr)],
    };

    const FILE_PUTC: Func = Func {
        name: "FilePutc",
        args: &[Arg::new("c", ArgType::Char), Arg::new("file", ArgType::Ptr)],
    };

    const TODIGIT: Func = Func {
        name: "todigit",
        args: &[Arg::new("c", ArgType::Char)],
    };

    const ATOF: Func = Func {
        name: "atof",
        args: &[Arg::new("src", ArgType::Int)],
    };

    const STRTOUL: Func = Func {
        name: "strtoul",
        args: &[
            Arg::new("src", ArgType::Str),
            Arg::new("src_end", ArgType::Int),
            Arg::new("base", ArgType::Int),
        ],
    };

    const STRTOL: Func = Func {
        name: "strtol",
        args: &[
            Arg::new("src", ArgType::Str),
            Arg::new("src_end", ArgType::Int),
            Arg::new("base", ArgType::Int),
        ],
    };

    const ABS: Func = Func {
        name: "abs",
        args: &[Arg::new("val", ArgType::Int)],
    };

    const LABS: Func = Func {
        name: "labs",
        args: &[Arg::new("val", ArgType::Int)],
    };

    const ATOI: Func = Func {
        name: "atoi",
        args: &[Arg::new("src", ArgType::Int)],
    };

    const ATOL: Func = Func {
        name: "atol",
        args: &[Arg::new("src", ArgType::Int)],
    };

    const ATOB: Func = Func {
        name: "atob",
        args: &[Arg::new("src", ArgType::Int), Arg::new("num_dst", ArgType::Ptr)],
    };

    const SAVE_STATE: Func = Func {
        name: "SaveState",
        args: &[Arg::new("buf", ArgType::Ptr)],
    };

    const RESTORE_STATE: Func = Func {
        name: "RestoreState",
        args: &[Arg::new("buf", ArgType::Ptr), Arg::new("param", ArgType::Int)],
    };

    const STRCAT: Func = Func {
        name: "strcat",
        args: &[Arg::new("dest", ArgType::Str), Arg::new("src", ArgType::Ptr)],
    };

    const STRNCAT: Func = Func {
        name: "strncat",
        args: &[
            Arg::new("dest", ArgType::Str),
            Arg::new("src", ArgType::Str),
            Arg::new("maxlen", ArgType::Int),
        ],
    };

    const STRCMP: Func = Func {
        name: "strcmp",
        args: &[Arg::new("str1", ArgType::Str), Arg::new("str2", ArgType::Str)],
    };

    const STRNCMP: Func = Func {
        name: "strncmp",
        args: &[
            Arg::new("str1", ArgType::Str),
            Arg::new("str2", ArgType::Str),
            Arg::new("maxlen", ArgType::Int),
        ],
    };

    const STRCPY: Func = Func {
        name: "strcpy",
        args: &[Arg::new("dst", ArgType::Str), Arg::new("src", ArgType::Str)],
    };

    const STRNCPY: Func = Func {
        name: "strncpy",
        args: &[
            Arg::new("dst", ArgType::Str),
            Arg::new("str", ArgType::Str),
            Arg::new("maxlen", ArgType::Int),
        ],
    };

    const STRLEN: Func = Func {
        name: "strlen",
        args: &[Arg::new("src", ArgType::Str)],
    };

    const INDEX: Func = Func {
        name: "index",
        args: &[Arg::new("src", ArgType::Str), Arg::new("c", ArgType::Char)],
    };

    const RINDEX: Func = Func {
        name: "rindex",
        args: &[Arg::new("src", ArgType::Str), Arg::new("c", ArgType::Char)],
    };

    const STRCHR: Func = Func {
        name: "strchr",
        args: &[Arg::new("src", ArgType::Str), Arg::new("c", ArgType::Char)],
    };

    const STRRCHR: Func = Func {
        name: "strrchr",
        args: &[Arg::new("src", ArgType::Str), Arg::new("c", ArgType::Char)],
    };

    const STRPBRK: Func = Func {
        name: "strpbrk",
        args: &[Arg::new("src", ArgType::Str), Arg::new("list", ArgType::Str)],
    };

    const STRSPN: Func = Func {
        name: "strspn",
        args: &[Arg::new("src", ArgType::Str), Arg::new("list", ArgType::Str)],
    };

    const STRCSPN: Func = Func {
        name: "strcspn",
        args: &[Arg::new("src", ArgType::Str), Arg::new("list", ArgType::Str)],
    };

    const STRTOK: Func = Func {
        name: "strtok",
        args: &[Arg::new("src", ArgType::Str), Arg::new("list", ArgType::Str)],
    };

    const STRSTR: Func = Func {
        name: "strstr",
        args: &[Arg::new("src", ArgType::Str), Arg::new("substr", ArgType::Str)],
    };

    const TOUPPER: Func = Func {
        name: "toupper",
        args: &[Arg::new("c", ArgType::Char)],
    };

    const TOLOWER: Func = Func {
        name: "tolower",
        args: &[Arg::new("c", ArgType::Char)],
    };

    const BCOPY: Func = Func {
        name: "bcopy",
        args: &[
            Arg::new("src", ArgType::Ptr),
            Arg::new("dst", ArgType::Ptr),
            Arg::new("len", ArgType::Int),
        ],
    };

    const BZERO: Func = Func {
        name: "bzero",
        args: &[Arg::new("dst", ArgType::Ptr), Arg::new("len", ArgType::Int)],
    };

    const BCMP: Func = Func {
        name: "bcmp",
        args: &[
            Arg::new("ptr1", ArgType::Ptr),
            Arg::new("ptr2", ArgType::Ptr),
            Arg::new("len", ArgType::Int)
        ],
    };

    const MEMCPY: Func = Func {
        name: "memcpy",
        args: &[
            Arg::new("dst", ArgType::Ptr),
            Arg::new("src", ArgType::Ptr),
            Arg::new("len", ArgType::Int)
        ],
    };

    const MEMSET: Func = Func {
        name: "memset",
        args: &[
            Arg::new("dst", ArgType::Ptr),
            Arg::new("src", ArgType::Ptr),
            Arg::new("num", ArgType::Int)
        ],
    };

    const MEMMOVE: Func = Func {
        name: "memmove",
        args: &[
            Arg::new("dst", ArgType::Ptr),
            Arg::new("src", ArgType::Ptr),
            Arg::new("len", ArgType::Int)
        ],
    };

    const MEMCMP: Func = Func {
        name: "memcmp",
        args: &[
            Arg::new("src1", ArgType::Ptr),
            Arg::new("src2", ArgType::Ptr),
            Arg::new("len", ArgType::Int)
        ],
    };

    const MEMCHR: Func = Func {
        name: "memchr",
        args: &[
            Arg::new("dst", ArgType::Ptr),
            Arg::new("value", ArgType::Char),
            Arg::new("num", ArgType::Int)
        ],
    };

    const RAND: Func = Func {
        name: "rand",
        args: &[],
    };

    const SRAND: Func = Func {
        name: "srand",
        args: &[Arg::new("seed", ArgType::Int)],
    };

    const QSORT: Func = Func {
        name: "qsort",
        args: &[
            Arg::new("base", ArgType::Ptr),
            Arg::new("num", ArgType::Int),
            Arg::new("size", ArgType::Int),
            Arg::new("callback", ArgType::Ptr)
        ],
    };

    const STROD: Func = Func {
        name: "strod",
        args: &[Arg::new("str", ArgType::Str), Arg::new("endptr", ArgType::Int)],
    };

    const MALLOC: Func = Func {
        name: "malloc",
        args: &[Arg::new("size", ArgType::Int)],
    };

    const FREE: Func = Func {
        name: "free",
        args: &[Arg::new("ptr", ArgType::Ptr)],
    };

    const LSEARCH: Func = Func {
        name: "lsearch",
        args: &[
            Arg::new("key", ArgType::Ptr),
            Arg::new("base", ArgType::Ptr),
            Arg::new("num", ArgType::Int),
            Arg::new("size", ArgType::Int),
            Arg::new("callback", ArgType::Ptr),
        ],
    };

    const BSEARCH: Func = Func {
        name: "bsearch",
        args: &[
            Arg::new("key", ArgType::Ptr),
            Arg::new("base", ArgType::Ptr),
            Arg::new("num", ArgType::Int),
            Arg::new("size", ArgType::Int),
            Arg::new("callback", ArgType::Ptr),
        ],
    };

    const CALLOC: Func = Func {
        name: "calloc",
        args: &[Arg::new("num", ArgType::Int), Arg::new("size", ArgType::Int)],
    };

    const REALLOC: Func = Func {
        name: "realloc",
        args: &[Arg::new("ptr", ArgType::Ptr), Arg::new("size", ArgType::Int)],
    };
}

impl BusMap for Bios {
    const BUS_BEGIN: u32 = 0x1fc00000;
    const BUS_END: u32 = Self::BUS_BEGIN + Self::SIZE as u32 - 1;
}
