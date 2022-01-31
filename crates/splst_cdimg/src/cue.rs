use crate::bcd::Bcd;
use crate::msf::Msf;
use crate::{Error, Index, TrackMode, Storage};

use std::path::PathBuf;
use std::fs::File;
use std::io::Read;

struct Track {
    num: Bcd,
    binary: usize,
    mode: TrackMode,
    start: Msf,
}

struct Binary {
    data: Box<[u8]>,
}

impl Binary {
    fn size_left(&self) -> usize {
        self.data.len() - self.consumed
    }
}

struct CueIndex {
    track: usize,
    line: usize,
    num: Bcd,
    start: Msf,
}

struct Parser<'a> {
    input: &'a str,
    indices: Vec<CueIndex>,
    binaries: Vec<Binary>,
    tracks: Vec<Track>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            indices: Vec::new(),
            binaries: Vec::new(),
            tracks: Vec::new(),
        }
    }

    fn parse(&mut self) -> Result<(), Error> {
        let lines = self.input.lines();
        for (i, line) in lines.enumerate() {
            let mut tokens = Tokens::new(line, i + 1);
            // Skip the line if it's empty.
            let Some(first) = tokens.next() else {
                continue;
            };
            match first? {
                "FILE" => {
                    let path = PathBuf::from(tokens.expect("filename")?);
                    let format = tokens.expect("file format")?;
                    if format != "Binary" {
                        return Err(Error::cue_err(
                            tokens.line_num, &format!("Unsupported file format '{format}'")
                        ));
                    }
                    let mut file = File::open(path)?;
                    let mut data = Vec::new();
                    file.read_to_end(&mut data)?;
                    self.binaries.push(Binary {
                        data: data.into_boxed_slice(),
                        consumed: 0,
                    });
                }
                "TRACK" => {
                    let Some(binary) = self.binaries.len().checked_sub(1) else {
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
                    self.tracks.push(Track {
                        mode, num, binary, start: Msf::ZERO
                    });
                }
                "INDEX" => {
                    let (num, start) = (
                        tokens.expect_bcd("index number")?,
                        tokens.expect_msf("index start")?,
                    );
                    let Some(track_idx) = self.tracks.len().checked_sub(1) else {
                        return Err(Error::cue_err(
                            tokens.line_num, "'Index' command before 'Track' command"
                        ))
                    };
                    if self.indices.iter().rev()
                        .take_while(|idx| idx.track == track_idx)
                        .any(|idx| idx.num == num)
                    {
                        return Err(Error::cue_err(
                            tokens.line_num, &format!("'Duplicate index '{num}'")
                        ))
                    }
                    self.indices.push(CueIndex {
                        num, start, line: tokens.line_num, track: self.tracks.len() - 1
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
        Ok(Default::default())
    }
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
            .map(|token| token_to_bcd(self.line_num, token));
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
        let arg = if let Some(new) = self.line.strip_suffix('"') {
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
    Bcd::from_bcd(num).ok_or_else(|| {
        Error::cue_err(line, &format!(
            "Invalid Track number '{num}' (Doesn't fit BCD format)"
        ))
    })
}

pub fn parse_cue(input: &str) -> Result<(Vec<Binary>, Vec<Index>), Error> {
    let mut parser = Parser::new(input);
    parser.parse()?;

    // The absolute index for all files.
    let mut abs = Msf::ZERO; 

    let mut indices = Vec::with_capacity(parser.indices.len());
    let mut idx_iter = parser.indices.iter().map(|idx| {
        (idx, &parser.tracks[idx.track])
    });

    for (i, bin) in &mut parser.binaries.enumerate() {
        // Offset into 'bin'.
        let mut offset = 0;

        // The previous index.
        let mut prev = None;

        for (idx, track) in idx_iter.take_while(|(_, t)| t.binary == i) {
            let size = (idx.start - prev).sector() * track.mode.sector_size();

            if size > bin.size_left() {
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
                index: track.binary, mode: bin.mode, offset
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

            let bytes_left = bin.len() - offset;
            let sectors_left = bytes_left / sector_size;
            
            // Check that the remaining space is aligned with the sector size.
            if bytes_left % sector_size != 0 {
                return Err(Error::cue_err(
                    idx.line, "Sector alignment error following index"
                ));
            }

            abs = Msf::from_sector(sectors_left).and_then(|m| m.checked_add(abs)).ok_or(
                Error::cue_err(idx.line, "Absolute MFS more than maximum of '99:99:99'")
            )?;
        }
    }

    Ok((parser.binaries, indices))
}
