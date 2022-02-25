//! This module emulates the Playstations GPU command buffer.

use splst_util::Bit;
use super::gp0;

use std::ops::Index;

pub struct Fifo {
    data: [u32; Self::SIZE],
    head: u32,
    tail: u32,
    cmd_words_left: Option<u8>,
}

impl Fifo {
    const SIZE: usize = 16;

    pub fn new() -> Self {
        Self {
            data: [0x0; Self::SIZE],
            head: 0,
            tail: 0,
            cmd_words_left: None,
        }
    }

    pub fn len(&self) -> u8 {
        self.head.wrapping_sub(self.tail) as u8
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    pub fn is_full(&self) -> bool {
        self.len() as usize == Self::SIZE
    }

    pub fn clear(&mut self) {
        self.tail = self.head;
    }

    fn push_internal(&mut self, val: u32) {
        self.data[self.head as usize % Self::SIZE] = val;
        self.head = self.head.wrapping_add(1);
    }

    /// Push data to the FIFO. Should never be called if 'val' could be a command or argument to a
    /// command.
    pub fn push(&mut self, val: u32) {
        if !self.is_full() {
            self.push_internal(val);
        } else {
            warn!("Push data to full GPU FIFO, value {}", val);
        }
    }

    /// Push command data to the FIFO.
    pub fn push_cmd(&mut self, val: u32) -> Option<PushAction> {
        if self.is_full() {
            let cmd = val.bit_range(24, 31);

            if gp0::cmd_is_imm(cmd) {
                return Some(PushAction::ImmCmd);
            }

            warn!("Push command to full GPU FIFO, either argument or GP0({:0x})", cmd);

            if let Some(cmd) = self.next_cmd() {
                warn!("The next command is GP0({:0x}), which has len {}", cmd, gp0::cmd_fifo_len(cmd));
            }

            return None;
        }

        let words_left = match self.cmd_words_left.take() {
            Some(words_left) => words_left,
            None => {
                let cmd = val.bit_range(24, 31);

                if gp0::cmd_is_imm(cmd) {
                    return Some(PushAction::ImmCmd);
                }

                gp0::cmd_fifo_len(cmd)
            }
        };

        self.push_internal(val);

        match words_left - 1 {
            0 => Some(PushAction::FullCmd),
            words => {
                self.cmd_words_left = Some(words);
                None
            }
        }
    }

    pub fn pop(&mut self) -> u32 {
        if self.is_empty() {
            warn!("Poping from an empty GPU FIFO");
        }

        let val = self[0];
        self.tail = self.tail.wrapping_add(1);

        val
    }

    pub fn next_cmd(&self) -> Option<u32> {
        if !self.is_empty() {
            Some(self[0].bit_range(24, 31))
        } else {
            None
        }
    }

    pub fn next_cmd_len(&self) -> Option<u8> {
        self.next_cmd().map(|cmd| gp0::cmd_fifo_len(cmd))
    }

    pub fn has_full_cmd(&self) -> bool {
        self.next_cmd_len().map_or(false, |len| len <= self.len())
    }
}

impl Index<u8> for Fifo {
    type Output = u32;

    fn index(&self, index: u8) -> &Self::Output {
        debug_assert!(index < self.len());
        
        let idx = self.tail.wrapping_add(index as u32) as usize % Self::SIZE;

        &self.data[idx]
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PushAction {
    /// If the pushed value is an immediate command, in which case it won't be pushed to the FIFO.
    /// This can only be the case if the FIFO isn't expecting the argument to a previous command.
    ImmCmd,
    /// If the pushed value was the last argument to a command and this a command is ready to be
    /// executed.
    FullCmd,
}

#[test]
fn pop_and_push() {
    let mut fifo = Fifo::new();

    assert!(fifo.is_empty());

    fifo.push(1); 
    fifo.push(2); 
    fifo.push(3); 

    assert_eq!(fifo.len(), 3);

    fifo.push(4); 
    fifo.push(5); 
    fifo.push(6); 
    fifo.push(7); 
    fifo.push(8); 
    fifo.push(9); 
    fifo.push(10); 
    fifo.push(11); 
    fifo.push(12); 
    fifo.push(13); 
    fifo.push(14); 
    fifo.push(15);
    fifo.push(16);

    assert_eq!(fifo.len(), 16); 

    fifo.push(17);

    assert_eq!(fifo.len(), 16); 
    assert!(fifo.is_full());

    assert_eq!(fifo[0], 1);
    assert_eq!(fifo[1], 2);
    assert_eq!(fifo[2], 3);
    assert_eq!(fifo[15], 16);

    assert_eq!(fifo.pop(), 1);
    assert_eq!(fifo.pop(), 2);
    assert_eq!(fifo.pop(), 3);
    assert_eq!(fifo.pop(), 4);
    assert_eq!(fifo.pop(), 5);
    assert_eq!(fifo.pop(), 6);
    assert_eq!(fifo.pop(), 7);
    assert_eq!(fifo.pop(), 8);
    assert_eq!(fifo.pop(), 9);
    assert_eq!(fifo.pop(), 10);
    assert_eq!(fifo.pop(), 11);
    assert_eq!(fifo.pop(), 12);
    assert_eq!(fifo.pop(), 13);

    assert_eq!(fifo.len(), 3);

    assert_eq!(fifo.pop(), 14);
    assert_eq!(fifo.pop(), 15);
    assert_eq!(fifo.pop(), 16);

    assert!(fifo.is_empty());
}

#[test]
fn cmd() {
    use splst_util::BitSet;
    let mut fifo = Fifo::new();

    assert!(!fifo.has_full_cmd());

    fifo.push(0_u32.set_bit_range(24, 31, 0x3f));

    assert!(!fifo.has_full_cmd());
    assert_eq!(fifo.next_cmd_len().unwrap(), 12);

    fifo.push(1); 
    fifo.push(2); 
    fifo.push(3); 
    fifo.push(4); 
    fifo.push(5); 
    fifo.push(6); 
    fifo.push(7); 
    fifo.push(8); 
    fifo.push(9); 
    fifo.push(10); 

    assert!(!fifo.has_full_cmd());

    fifo.push(11); 

    assert!(fifo.has_full_cmd());
}

#[test]
fn cmd2() {
    use splst_util::BitSet;
    let mut fifo = Fifo::new();

    assert_eq!(fifo.push_cmd(0x0_u32.set_bit_range(24, 31, 0x30)), None);

    for _ in 0..4 {
        assert_eq!(fifo.push_cmd(0x0), None); 
    }

    assert!(matches!(fifo.push_cmd(0x0), Some(PushAction::FullCmd)));
    assert!(matches!(fifo.push_cmd(0x0), Some(PushAction::ImmCmd)));
}
