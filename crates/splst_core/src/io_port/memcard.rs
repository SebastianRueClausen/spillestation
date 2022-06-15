use splst_util::BitSet;
use super::IoSlot;

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum MemCardError {
    #[error("failed to access memory card file: {0}")] 
    IoError(#[from] io::Error),
    #[error("invalid memory card file. Must be at least 128 kb, is {0} bytes")]
    InvalidSize(usize),
}

#[derive(Default)]
pub struct MemCards([Option<MemCard>; 2]);

impl MemCards {
    pub fn get(&self, slot: IoSlot) -> Option<&MemCard> {
        self.0[slot as usize].as_ref()
    }

    pub fn get_mut(&mut self, slot: IoSlot) -> Option<&mut MemCard> {
        self.0[slot as usize].as_mut()
    }

    pub(super) fn reset_transfer_state(&mut self) {
        self.0.iter_mut().flatten().for_each(|card| {
            card.state = TransferState::Idle;
            card.addr = 0;
            card.last_byte = 0;
        });
    }
}

pub struct MemCard {
    flash: Box<[u8]>,

    /// The current transfer state.
    state: TransferState,

    /// Sector address used to indicate which sector should be read or written to.
    addr: u16,

    /// The byte recieved last transfer.
    last_byte: u8,

    /// Internal flag in the flags register which is set after the first successfull write to
    /// `flash`.
    has_written: bool,

    /// `true` if the content of `flash` has been changed since last saved.
    changed: bool,

    /// cache used to write into during the write sequence. This seems to be what the real hardware
    /// uses as well. It seems from the test i have done that the writes are only commited when the
    /// write sequence is at [`WriteState::End`], altrough im not 100% sure they are correct.
    ///
    /// It also means that we can serialize `flash` at any time since it's guarenteed to be in a
    /// valid state.
    write_sector: [u8; SECTOR_SIZE],

    save_path: PathBuf,

    error: Option<MemCardError>,
}

impl MemCard {
    fn new(save_path: PathBuf, flash: Box<[u8]>) -> Self {
        Self {
            flash,
            save_path,
            state: TransferState::Idle,
            addr: 0,
            last_byte: 0,
            has_written: false,
            changed: false,
            write_sector: [0x0; SECTOR_SIZE],
            error: None,
        }

    }

    /// Create new freshly formatted memory card, which will be saved to `path`.
    pub fn fresh_to(path: &Path) -> Self {
        let mut flash = Box::new([0x0; FLASH_SIZE]);
        format(&mut flash);

        Self::new(path.to_path_buf(), flash)
    }

    pub fn load_from(path: &Path) -> Result<Self, MemCardError> {
        let mut file = File::open(path)?;
        let mut data = Vec::<u8>::with_capacity(FLASH_SIZE);

        file.read_to_end(&mut data)?;

        if data.len() < FLASH_SIZE {
            Err(MemCardError::InvalidSize(data.len()))
        } else {
            Ok(Self::new(path.to_path_buf(), data.into_boxed_slice()))
        }
    }

    fn save(&mut self) {
        let mut file = match File::open(&self.save_path) {
            Ok(file) => file,
            Err(err) => {
                self.error = Some(err.into());
                return
            }
        };
        if let Err(err) = file.set_len(0).and_then(|()| file.write_all(&self.flash)) {
            self.error = Some(err.into());
        };
    }

    /// Return flags register.
    fn flags(&self) -> u8 {
        u8::from(!self.has_written) << 3
    }

    pub fn transfer(&mut self, val: u8) -> (u8, bool) {
        let out = match self.state {
            TransferState::Idle if val == 0x81 => {
                self.state = TransferState::Command; 

                (0xff, true)
            }
            // Do nothing. The first byte should always contain 0x81.
            TransferState::Idle => {
                warn!("weird first memory card byte recieved {val:0x}"); 

                (0xff, false)
            }
            TransferState::Command => {
                self.state = match val {
                    0x52 => TransferState::Read(ReadState::CardId1),
                    0x53 => TransferState::Id(IdState::CardId1),
                    0x57 => TransferState::Write(WriteState::CardId1),
                    _ => {
                        error!("invalid cd-rom command {val:0x}");

                        self.state = TransferState::Idle; 
                        return (self.flags(), false);
                    }
                };

                (self.flags(), true)
            }
            TransferState::Read(ref mut state) => {
                use ReadState::*;

                let (out, next) = match *state {
                    CardId1 => (0x5a, CardId2),
                    CardId2 => (0x5d, AddrHi),

                    AddrHi => {
                        self.addr = self.addr.set_bit_range(8, 15, u16::from(val));
                        (0x0, AddrLo) 
                    }
                    AddrLo => {
                        self.addr = self.addr.set_bit_range(0, 7, u16::from(val));
                        (self.last_byte, Ack1)
                    }

                    Ack1 => (0x5c, Ack2),
                    Ack2 => (0x5d, ConfirmAddrHi),

                    ConfirmAddrHi => ((self.addr >> 8) as u8, ConfirmAddrLo),
                    ConfirmAddrLo => (self.addr as u8, Data(0)),

                    Data(offset) if offset < 127 => {
                        (sector(&self.flash, self.addr)[offset], Data(offset - 1))
                    }
                    Data(_) => {
                        let val = sector(&self.flash, self.addr)[127];
                        (val, Checksum)
                    }
                    
                    Checksum => {
                        let sector = sector(&self.flash, self.addr.into());
                        let sum = checksum(&sector[..127])
                            ^ (self.addr >> 8) as u8
                            ^ self.addr as u8;

                        (sum, End)
                    }

                    End => {
                        self.state = TransferState::Idle;
                        return (b'G', true);
                    }
                };

                *state = next;

                (out, true)
            }
            TransferState::Write(ref mut state) => {
                use WriteState::*;

                let (out, next) = match *state {
                    CardId1 => (0x5a, CardId2),
                    CardId2 => (0x5d, AddrHi),

                    AddrHi => {
                        self.addr = self.addr.set_bit_range(8, 15, u16::from(val));

                        (0x0, AddrLo) 
                    }
                    AddrLo => {
                        self.addr = self.addr.set_bit_range(0, 7, u16::from(val));

                        (self.last_byte, Data(0))
                    }

                    Data(offset) if offset < 127 => {
                        self.has_written = true;
                        self.write_sector[offset] = val;

                        (self.last_byte, Data(offset + 1))
                    }
                    Data(_) => {
                        self.write_sector[127] = val;

                        (self.last_byte, Checksum)
                    }
                    
                    // We just wait calculating the checksum until we actually need it.
                    Checksum => (self.last_byte, Ack1),

                    Ack1 => (0x5c, Ack2),
                    Ack2 => (0x5d, End),

                    End => {
                        self.state = TransferState::Idle;

                        if self.addr <= 0x3ff {
                            self.changed |= sector_mut(&mut self.flash, self.addr)
                                .iter_mut()
                                .zip(self.write_sector.iter())
                                .fold(false, |changed, (to, from)| {
                                    let diff = to != from;
                                    *to = *from;

                                    changed | diff
                                });
                        } else {
                            // Invalid sector.
                            return (0xff,  true);
                        };

                        let sum = checksum(&self.write_sector)
                            ^ (self.addr >> 8) as u8
                            ^ self.addr as u8;

                        let out = if sum != 0 { b'N' } else { b'G' };

                        return (out, true);
                    }
                };

                *state = next;

                (out, true)
            }
            TransferState::Id(ref mut state) => {
                use IdState::*;

                let (out, next) = match *state {
                    CardId1 => (0x5a, CardId2),
                    CardId2 => (0x5d, Ack1),

                    Ack1 => (0x5c, Ack2),
                    Ack2 => (0x5d, Rest1),

                    Rest1 => (0x04, Rest2),
                    Rest2 => (0x00, Rest3),
                    Rest3 => (0x00, Rest4),
                    Rest4 =>  {
                        self.state = TransferState::Idle;
                        return (0x80, true);
                    }
                };

                *state = next;

                (out, true)
            }
        };

        self.last_byte = val;

        out
    }
}

enum WriteState {
    CardId1,
    CardId2,

    AddrHi,
    AddrLo,

    /// Holds the current offset into the sector being written to.
    Data(usize),
    Checksum,
  
    Ack1,
    Ack2,

    End,
}

enum ReadState {
    CardId1,
    CardId2,

    AddrHi,
    AddrLo,

    Ack1,
    Ack2,

    ConfirmAddrHi,
    ConfirmAddrLo,

    /// Holds the current offset into the sector being read.
    Data(usize),
    Checksum,

    End,
}

enum IdState {
    CardId1,
    CardId2,

    Ack1,
    Ack2,
    
    Rest1,
    Rest2,
    Rest3,
    Rest4,
}

/// The transfer state of the memory card.
enum TransferState {
    /// The memory card is not doing anything.
    Idle,

    /// The memory card is waiting for the next command, which is gonna start either a write
    /// sequence, read sequence or ID sequence.
    Command,

    /// One sector of data is going to be written to the memory card via transfers.
    Write(WriteState),

    /// One sector of data is going to be read from the memory card via transfers.
    Read(ReadState),

    /// Get the card ID.
    Id(IdState),
}

/// Calculate the checksum of `mem`.
fn checksum(mem: &[u8]) -> u8 {
    mem.iter().fold(0, |sum, val| sum ^ val)
}

fn sector_mut(flash: &mut [u8], index: u16) -> &mut [u8] {
    let index: usize = index.into();

    let start = index * SECTOR_SIZE;
    let end = start + SECTOR_SIZE;
   
    &mut flash[start..end]
}

fn sector(flash: &[u8], index: u16) -> &[u8] {
    let index: usize = index.into();

    let start = index * SECTOR_SIZE;
    let end = start + SECTOR_SIZE;
   
    &flash[start..end]
}

/// Format `flash` which erases everything from it.
fn format(flash: &mut [u8; FLASH_SIZE]) {
    *flash = [0x0; FLASH_SIZE];

    // Magic values for header.
    flash[0] = b'M';
    flash[1] = b'C';
    
    flash[127] = checksum(&flash[..127]);
    
    // format the the rest of files.
    for i in 1..16 {
        let data = sector_mut(flash, i);
        
        // Set status as free.
        data[0] = 0xa0;

        // Set next pointer to none.
        data[8] = 0xff;
        data[9] = 0xff;
        
        // Set checksum.
        data[127] = checksum(&data[..127]);
    }
    
    // Set broken sector list.
    for i in 16..36 {
        let data = sector_mut(flash, i);

        // Set sector position to none.
        data[0] = 0xff;
        data[1] = 0xff;
        data[2] = 0xff;
        data[3] = 0xff;
        data[8] = 0xff;
        data[9] = 0xff;

        data[127] = checksum(&data[..127]);
    }
}

/// Check that `flash` is formatted correctly.
fn check_format(flash: &[u8; FLASH_SIZE]) -> bool {
    if flash[0] != b'M' || flash[1] != b'C' {
        return false;
    }

    if checksum(&flash[..127]) != flash[127] {
        return false;
    }

    (1..16).all(|i| {
        let data = sector(flash, i);
        checksum(&data[..127]) == data[127]
    })
}


const SECTOR_SIZE: usize = 128;
const FLASH_SIZE: usize = 16 * 64 * SECTOR_SIZE;
    
