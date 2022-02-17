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

    #[cfg(test)]
    pub fn push(&mut self, val: u32) {
        if !self.is_full() {
            self.data[self.head as usize & (Self::SIZE - 1)] = val;
            self.head = self.head.wrapping_add(1);
        }
    }

    pub fn try_push(&mut self, val: u32) -> bool {
        if self.is_full() {
            warn!("Push to full GPU FIFO");
            return false
        }

        self.data[self.head as usize & (Self::SIZE - 1)] = val;
        self.head = self.head.wrapping_add(1);

        true
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
fn command() {
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
