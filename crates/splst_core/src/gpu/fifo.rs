//! This module emulates the Playstations GPU command buffer.

use splst_util::Bit;

use std::ops::Index;

pub struct Fifo {
    data: [u32; Self::SIZE],
    head: u32,
    tail: u32,
}

impl Fifo {
    const SIZE: usize = 16;

    pub fn new() -> Self {
        Self {
            data: [0x0; Self::SIZE],
            head: 0,
            tail: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.head.wrapping_sub(self.tail) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    pub fn is_full(&self) -> bool {
        self.len() == Self::SIZE
    }

    pub fn clear(&mut self) {
        self.tail = self.head;
    }

    pub fn push(&mut self, value: u32) {
        if  self.is_full() {
            warn!("Pushing to a full GPU FIFO");
            panic!();
        }
        self.data[self.head as usize & (Self::SIZE - 1)] = value;
        self.head = self.head.wrapping_add(1);
    }

    pub fn pop(&mut self) -> u32 {
        if self.is_empty() {
            warn!("Poping from an empty GPU FIFO");
        }
        let value = self[0];
        self.tail = self.tail.wrapping_add(1);
        value
    }

    pub fn next_cmd(&self) -> Option<u32> {
        if !self.is_empty() {
            Some(self[0].bit_range(24, 31))
        } else {
            None
        }
    }

    pub fn next_cmd_len(&self) -> Option<usize> {
        self.next_cmd().map(|cmd| usize::from(CMD_LEN[cmd as usize]))
    }

    pub fn has_full_cmd(&self) -> bool {
        self.next_cmd_len().map_or(false, |len| len <= self.len())
    }
}

impl Index<usize> for Fifo {
    type Output = u32;

    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < self.len());
        &self.data[self.tail.wrapping_add(index as u32) as usize & (Self::SIZE - 1)]
    }
}

/// Number of words in each GP0 command.
const CMD_LEN: [u8; 0x100] = [
    1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    4, 4, 4, 4, 7, 7, 7, 7, 5, 5, 5, 5, 9, 9, 9, 9,
    6, 6, 6, 6, 9, 9, 9, 9, 8, 8, 8, 8, 12, 12, 12, 12,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    3, 3, 3, 3, 4, 4, 4, 4, 2, 2, 2, 2, 3, 3, 3, 3,
    2, 2, 2, 2, 3, 3, 3, 3, 2, 2, 2, 2, 3, 3, 3, 3,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];
