use crate::util::Bit;

const FIFO_SIZE: usize = 16;

pub struct Fifo {
   data: [u8; FIFO_SIZE],
   /// The head pointer is 4 bits and a carry bit.
   head: u8,
   /// The tail pointer is 4 bits and a carry bit.
   tail: u8,
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
        self.head.wrapping_sub(self.tail).bit_range(0, 3) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_full(&self) -> bool {
        self.len() == FIFO_SIZE
    }

    pub fn clear(&mut self) {
        self.head = self.tail;
    }

    pub fn push(&mut self, value: u8) {
        self.data[self.head.bit_range(0, 3) as usize] = value;
        self.head = self.head.wrapping_add(1).bit_range(0, 3);
    }

    pub fn push_slice(&mut self, values: &[u8]) {
        debug_assert!(!self.is_full());
        for value in values {
            self.push(*value);
        }
    }

    pub fn pop(&mut self) -> u8 {
        let value = self.data[self.tail.bit_range(0, 3) as usize];
        self.tail = self.tail.wrapping_add(1).bit_range(0, 3);
        value
    }

    pub fn try_pop(&mut self) -> Option<u8> {
        if self.is_empty() {
            None
        } else {
            Some(self.pop())
        }
    }
}
