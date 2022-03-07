#![allow(dead_code)]

use splst_util::{Bcd, Msf};
use crate::index::{Index, IndexLookup, Storage, Binary};
use crate::{Error, TrackMode};
use crate::cd::CdImage;

use itertools::Itertools;
use memmap2::Mmap;

use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;

#[derive(Debug, Clone, Copy)]
struct TrackEntry {
    num: Bcd,
    binary: usize,
    mode: TrackMode,
    start: Msf,
}

#[derive(Debug, Clone, Copy)]
struct IndexEntry {
    /// The index of the track.
    track: usize,
    /// The line of source the index is defined on.
    line: usize,
    num: Bcd,
    start: Msf,
}

fn parse(input: &str, folder: &Path) -> Result<(Vec<PathBuf>, Vec<TrackEntry>, Vec<IndexEntry>), Error> {
    let (mut indices, mut binaries, mut tracks) = (
        Vec::<IndexEntry>::new(), Vec::<PathBuf>::new(), Vec::<TrackEntry>::new()
    );
    let lines = input.lines();
    for (i, line) in lines.enumerate() {
        let mut tokens = Tokens::new(line, i + 1);
        // Skip the line if it's empty.
        let Some(first) = tokens.next() else {
            continue;
        };
        match first? {
            "FILE" => {
                let path = folder
                    .to_path_buf()
                    .join(tokens.expect("filename")?);

                let format = tokens.expect("file format")?;

                if format != "BINARY" {
                    return Err(Error::cue_err(
                        tokens.line_num, &format!("Unsupported file format '{format}'")
                    ));
                }

                binaries.push(path);
            }
            "TRACK" => {
                let Some(binary) = binaries.len().checked_sub(1) else {
                    return Err(Error::cue_err(
                        tokens.line_num, "'TRACK' command before 'FILE' command"
                    ));
                };
                let (num, format) = (
                    tokens.expect_bcd("track numer")?,
                    tokens.expect("track format")?,
                );
                let Some(mode) = TrackMode::from_str(format) else {
                    return Err(Error::cue_err(
                        tokens.line_num, &format!("Invalid track format '{format}'")
                    ));
                };
                tracks.push(TrackEntry {
                    mode, num, binary, start: Msf::ZERO
                });
            }
            "INDEX" => {
                let (num, start) = (
                    tokens.expect_bcd("index number")?,
                    tokens.expect_msf("index start")?,
                );
                let Some(track) = tracks.len().checked_sub(1) else {
                    return Err(Error::cue_err(
                        tokens.line_num, "'Index' command before 'Track' command"
                    ))
                };
                if indices.iter()
                    .rev()
                    .take_while(|idx| idx.track == track)
                    .any(|idx| idx.num == num)
                {
                    return Err(Error::cue_err(
                        tokens.line_num, &format!("'Duplicate index '{num}'")
                    ));
                }
                indices.push(IndexEntry {
                    num, start, line: tokens.line_num, track: tracks.len() - 1
                });
            }
            "REM" => {
                // 'REM' means it's a comment. Comments can contain metadata, but that is not
                // supported for now.
                continue;
            }
            cmd => {
                return Err(Error::cue_err(
                    tokens.line_num, &format!("Invalid command '{cmd}'")
                ));
            }
        }
    }
    Ok((binaries, tracks, indices))
}

struct Tokens<'a> {
    line: &'a str,
    line_num: usize,
}

impl<'a> Tokens<'a> {
    fn new(line: &'a str, line_num: usize) -> Self {
        Tokens { line, line_num }
    }

    fn expect(&mut self, what: &str) -> Result<&'a str, Error> {
        self.next().unwrap_or_else(|| {
            Err(Error::cue_err(self.line_num, &format!("Expected '{what}'")))
        })
    }

    fn expect_bcd(&mut self, what: &str) -> Result<Bcd, Error> {
        token_to_bcd(self.line_num, self.expect(what)?)
    }

    fn expect_msf(&mut self, what: &str) -> Result<Msf, Error> {
        let mut mfs = self
            .expect(what)?
            .split(':')
            .map(|token| {
                token_to_bcd(self.line_num, token)
            });
        let mut expect = || mfs.next().unwrap_or(
            Err(Error::cue_err(self.line_num, "MFS should have format 'xx:xx:xx'")
        ));
        Ok(Msf::from_bcd(expect()?, expect()?, expect()?))
    }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Result<&'a str, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.line = self.line.trim_start();
        if self.line.is_empty() {
            return None;
        }
        // The next 'token' is a string.
        let arg = if let Some(new) = self.line.strip_prefix('"') {
            if let Some((arg, rest)) = new.split_once("\"") {
                self.line = rest; 
                Ok(arg)
            } else {
                Err(Error::cue_err(self.line_num, "Unterminated string"))
            }
        } else {
            let new = self.line.split_once(|c: char| c.is_ascii_whitespace());
            if let Some((arg, rest)) = new {
                self.line = rest;
                Ok(arg)
            } else {
                let line = self.line;
                self.line = "";
                Ok(line)
            }
        };
        Some(arg)
    }
}

fn token_to_bcd(line: usize, token: &str) -> Result<Bcd, Error> {
    let num = u8::from_str_radix(token, 10).map_err(|err| {
        Error::cue_err(line, &format!("Invalid number: {err}"))
    });
    let num = num?;
    Bcd::from_binary(num).ok_or_else(|| {
        Error::cue_err(line, &format!(
            "Invalid Track number '{num}' (Doesn't fit BCD format)"
        ))
    })
}

pub fn parse_cue(path: &Path) -> Result<CdImage, Error> {
    let folder = path.parent().ok_or_else(|| {
        Error::PathError(format!(
            "Can't find parent folder for cue file with path: {}", path.display()
        ))
    })?;

    let mut source = String::new();
    File::open(path)?.read_to_string(&mut source)?;

    let (binaries, track_entries, index_entries) = parse(&source, folder)?;

    // The absolute index for all files. Cue always skips the first 2 seconds.
    let mut abs = Msf::from_sector(150).unwrap(); 

    let mut indices = Vec::with_capacity(index_entries.len() + 1);
    let mut idx_iter = index_entries.iter().map(|idx| {
        (idx, &track_entries[idx.track])
    });

    let binaries = binaries
        .iter()
        .map(|path| {
            let file = File::open(path)?;
            let mmap = unsafe {
                Mmap::map(&file)?
            };
            Ok(Binary {
                data: mmap,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;

    // Since cue files doesn't specify track 1's pregap, it get's added here if there is a track 1.
    if let Some(track) = &track_entries.iter().find(|t| t.num == Bcd::ONE) {
        indices.push(Index {
            index: Bcd::ZERO,
            track: Bcd::ZERO,
            sector: abs.sector(),
            format: track.mode.track_format(),
            data: Storage::PreGap,
        });
    }

    for (i, bin) in &mut binaries.iter().enumerate() {
        // Offset into 'bin'.
        let mut offset = 0;

        // The previous index.
        let mut prev: Option<(&IndexEntry, &TrackEntry)> = None;

        for (idx, track) in idx_iter.take_while_ref(|(_, t)| t.binary == i) {
            let size = {
                let prev_start = if let Some(prev) = prev {
                    prev.0.start
                } else {
                    Msf::ZERO
                };
                (idx.start - prev_start).sector() * track.mode.sector_size()
            };

            if size > (bin.data.len() - offset) {
                return Err(Error::cue_err(
                    idx.line, "Index goes past the end of the binary file"
                ));
            }

            abs = abs.checked_add(idx.start).ok_or(
                Error::cue_err(idx.line, "Absolute MFS more than maximum of '99:99:99'")
            )?;

            prev = Some((idx, track));
            offset += size;

            let data = Storage::Binary {
                binary: track.binary, mode: track.mode, offset
            };

            indices.push(Index {
                index: idx.num,
                track: track.num,
                sector: abs.sector(),
                format: track.mode.track_format(),
                data,
            });
        }

        // The last index in the file spans get rest of file.
        if let Some((idx, track)) = prev {
            let sector_size = track.mode.sector_size();

            let bytes_left = bin.data.len() - offset;
            let sectors_left = bytes_left / sector_size;
            
            // Check that the remaining space is aligned with the sector size.
            if bytes_left % sector_size != 0 {
                return Err(Error::cue_err(
                    idx.line, "Sector alignment error following index"
                ));
            }

            abs = Msf::from_sector(sectors_left)
                .and_then(|m| m.checked_add(abs))
                .ok_or(
                    Error::cue_err(idx.line, "Absolute MFS more than maximum of '99:99:99'")
                )?;
        }
    }

    Ok(CdImage::new(path, IndexLookup::new(indices, abs.sector()), binaries))
}

#[test]
fn parse_test() {
    let source = r#"
        FILE "crash_bandicoot.bin" BINARY
            TRACK 01 MODE2/2352
            INDEX 01 00:00:00        
    "#;
    let (binaries, tracks, indices) = parse(source, &PathBuf::new()).unwrap();

    assert_eq!(binaries.len(), 1);
    assert_eq!(tracks.len(), 1);
    assert_eq!(indices.len(), 1);

    let binary = &binaries[0];
    let track = &tracks[0];
    let index = &indices[0];

    assert_eq!(binary.to_str().unwrap(), "crash_bandicoot.bin");
    assert_eq!(track.mode, TrackMode::Mode2Raw);
    assert_eq!(track.num, Bcd::ONE);
    assert_eq!(index.num, Bcd::ONE);
    assert_eq!(index.start, Msf::ZERO);
}
