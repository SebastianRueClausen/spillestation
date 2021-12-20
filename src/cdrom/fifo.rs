use crate::util::bits::BitExtract;

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
        self.head.wrapping_sub(self.tail).extract_bits(0, 4) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_full(&self) -> bool {
        self.len() == FIFO_SIZE
    }

    pub fn push(&mut self, value: u8) {
        self.data[self.head.extract_bits(0, 4) as usize] = value;
        self.head = self.head.wrapping_add(1).extract_bits(0, 4);
    }

    pub fn push_slice(&mut self, values: &[u8]) {
        for value in values {
            self.push(*value);
        }
    }

    pub fn pop(&mut self) -> u8 {
        let value = self.data[self.tail.extract_bits(0, 4) as usize];
        self.tail = self.tail.wrapping_sub(1).extract_bits(0, 4);
        value
    }
}
