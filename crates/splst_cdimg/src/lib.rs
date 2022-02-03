#![feature(let_else)]

mod bcd;
mod msf;
mod index;
mod cue;
pub mod sector;
pub mod cd;

use thiserror::Error;

use std::io::{self, Read};
use std::path::Path;
use std::fs::File;

pub use cd::CdImage;
pub use sector::Sector;

#[derive(Debug, Error)]
pub enum Error {
    #[error("[error]: {0}")]
    IoError(#[from] io::Error),

    #[error("[error]: {0}")]
    PathError(String),

    #[error("[error]:{line}: {msg}")]
    CueError {
        line: usize,
        msg: String,
    },

    #[error("Section read failure")]
    SectionReadError,
}

impl Error {
    pub fn cue_err(line: usize, msg: impl Into<String>) -> Self {
        Error::CueError { line, msg: msg.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackMode {
    Audio,
    Mode1,
    Mode1Raw,
    Mode2,
    Mode2Raw,
}

impl TrackMode {
    pub fn from_str(val: &str) -> Option<Self> {
        let out = match val {
            "AUDIO" => TrackMode::Audio,
            "MODE1/2048" => TrackMode::Mode1,
            "MODE1/2352" => TrackMode::Mode1Raw,
            "MODE2/2336" => TrackMode::Mode2,
            "MODE2/2352" => TrackMode::Mode2Raw,
            _ => return None,
        };
        Some(out)
    }

    pub fn sector_size(self) -> usize {
        match self {
            TrackMode::Audio | TrackMode::Mode2Raw | TrackMode::Mode1Raw => 2352,
            TrackMode::Mode1 => 2048,
            TrackMode::Mode2 => 2336,
        }
    }

    pub fn track_format(self) -> TrackFormat {
        match self {
            TrackMode::Audio => TrackFormat::Audio,
            TrackMode::Mode1 | TrackMode::Mode1Raw => TrackFormat::Mode1,
            TrackMode::Mode2 | TrackMode::Mode2Raw => TrackFormat::Mode2Xa,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TrackFormat {
    Audio,
    Mode1,
    Mode2Xa,
}

pub fn open_cd(cue: &Path) -> Result<CdImage, Error> {
    let folder = cue.parent().ok_or_else(|| {
        Error::PathError(format!("Can't find parent folder for cue file with path: {}", cue.display()))
    })?;
    let mut source = String::new();
    File::open(cue)?.read_to_string(&mut source)?;
    cue::parse_cue(&source, &folder)
}
