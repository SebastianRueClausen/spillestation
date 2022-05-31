#![feature(let_else)]

mod index;
mod cue;
pub mod sector;
pub mod cd;

use thiserror::Error;

use std::{fmt, io};
use std::path::Path;

pub use cd::CdImage;
pub use sector::Sector;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    IoError(#[from] io::Error),

    #[error("{0}")]
    PathError(String),

    #[error("cue error:{line}: {msg}")]
    CueError {
        line: usize,
        msg: String,
    },

    #[error("section read failure")]
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
        use TrackMode::*;
        match self {
            Audio => TrackFormat::Audio,
            Mode1 | Mode1Raw => TrackFormat::Mode1,
            Mode2 | Mode2Raw => TrackFormat::Mode2Xa,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TrackFormat {
    Audio,
    Mode1,
    Mode2Xa,
}

impl fmt::Display for TrackFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrackFormat::Audio => f.write_str("audio"),
            TrackFormat::Mode1 => f.write_str("mode 1"),
            TrackFormat::Mode2Xa => f.write_str("mode 2 xa"),
        }
    }
}

pub fn open_cd(path: &Path) -> Result<CdImage, Error> {
    cue::parse_cue(&path)
}
