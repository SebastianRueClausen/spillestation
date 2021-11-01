
use std::ops::Index;

/// TODO: Test actual required size.
/// For now it's power of 2 for fast modulo.
const FIFO_SIZE: usize = 32;

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

    pub fn is_empty(&self) -> bool {
        self.len() == FIFO_SIZE
    }

    pub fn push(&mut self, value: u32) {
        // Fast modulo.
        self.data[self.head as usize & (FIFO_SIZE - 1)] = value;
        self.head = self.head.wrapping_add(1);
    }

    pub fn pop(&mut self) -> u32 {
        let index = self.tail as usize & (FIFO_SIZE - 1);
        self.tail = self.tail.wrapping_add(1);
        self.data[index]
    }
}

impl Index<usize> for Fifo {
    type Output = u32;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len());
        &self.data[self.tail.wrapping_add(index as u32) as usize & (FIFO_SIZE - 1)]
    }
}
