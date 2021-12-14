//! This module emulates the Playstations GPU command buffer.

use std::ops::Index;
use crate::util::bits::BitExtract;

/// Power of 2 for fast modulo.
const FIFO_SIZE: usize = 32;

/// Since the commands/instructions of the Playstations GPU aren't one word like the CPU, a buffer
/// is used to store the words until it has a full command. This is done using a queue/fifo. The
/// first word recives must be an instruction, since the instruction determines the length of
/// the command. It then checks after each push if the length of the buffer is equal to the length
/// of the instruction stored in the first slot. If it is, the command get's executed by the GPU.
///
/// Since we know the max size a command can have, this is implemented as a circular buffer.
pub struct Fifo {
    data: [u32; FIFO_SIZE], 
    head: u32,
    tail: u32,
}

impl Fifo {
    pub fn new() -> Self {
        Self {
            data: [0x0; FIFO_SIZE],
            head: 0,
            tail: 0,
        }
    }

    pub fn len(&self) -> usize {
        // Just to make sure we handle overflow, as there could be alot of writes to the fifo.
        self.head.wrapping_sub(self.tail) as usize 
    }

    pub fn clear(&mut self) {
        self.tail = self.head;
    }

    pub fn push(&mut self, value: u32) {
        self.data[self.head as usize & (FIFO_SIZE - 1)] = value;
        self.head = self.head.wrapping_add(1);
    }

    pub fn pop(&mut self) -> u32 {
        let value = self[0];
        self.tail = self.tail.wrapping_add(1);
        value
    }

    /// Checks if buffer holds a full command.
    pub fn has_full_cmd(&self) -> bool {
        let cmd = self[0].extract_bits(24, 31) as usize;
        CMD_LEN[cmd] as usize == self.len()
    }
}

impl Index<usize> for Fifo {
    type Output = u32;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.len());
        &self.data[self.tail.wrapping_add(index as u32) as usize & (FIFO_SIZE - 1)]
    }
}

/// Number of words in each GP0 command.
const CMD_LEN: [u8; 0x100] = [
    1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    4, 4, 4, 4, 7, 7, 7, 7, 5, 5, 5, 5, 9, 9, 9, 9, 6, 6, 6, 6, 9, 9, 9, 9, 8, 8, 8, 8, 12, 12, 12,
    12, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 3, 3, 3, 3, 4, 4, 4, 4, 2, 2, 2, 2, 3, 3, 3, 3, 2, 2, 2, 2, 3, 3, 3, 3, 2, 2, 2, 2, 3, 3,
    3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1,
];
