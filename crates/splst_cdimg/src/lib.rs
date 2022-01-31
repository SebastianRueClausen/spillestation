#![feature(let_else)]

mod cue;
mod bcd;
mod msf;

use bcd::Bcd;
use thiserror::Error;

use std::io;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    IoError(#[from] io::Error),

    #[error("[error]:{line}: {msg}")]
    CueError {
        line: usize,
        msg: String,
    },
}

impl Error {
    pub fn cue_err(line: usize, msg: impl Into<String>) -> Self {
        Error::CueError { line, msg: msg.into() }
    }
}

#[derive(Clone, Copy)]
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
            "Mode2/2352" => TrackMode::Mode2Raw,
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


enum TrackFormat {
    Audio,
    Mode1,
    Mode2Xa,
}

pub enum Storage {
    Binary {
        index: usize,
        offset: usize,
        mode: TrackMode, 
    },
    PreGap,
}

pub struct Index<T> {
    sector: usize,
    index: Bcd,
    track: Bcd,
    format: TrackFormat,
    data: T 
}
