use splst_util::Bit;

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
        self.head
            .wrapping_sub(self.tail)
            .bit_range(0, 3)
            .into()
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

    pub fn push(&mut self, val: u8) {
        debug_assert!(!self.is_full());
        let idx: usize = self.head
            .bit_range(0, 3)
            .into();
        self.head = self.head
            .wrapping_add(1)
            .bit_range(0, 3);
        self.data[idx] = val;
    }

    pub fn push_slice(&mut self, values: &[u8]) {
        for val in values {
            self.push(*val);
        }
    }

    pub fn pop(&mut self) -> u8 {
        debug_assert!(!self.is_empty());
        let idx: usize = self.tail
            .bit_range(0, 3)
            .into();
        self.tail = self.tail
            .wrapping_add(1)
            .bit_range(0, 3);
        self.data[idx]
    }

    pub fn try_pop(&mut self) -> Option<u8> {
        self.is_empty().then(|| self.pop())
    }

    pub fn peek(&self) -> Option<u8> {
        self.is_empty().then(|| {
            self.data[self.tail.bit_range(0, 3) as usize]
        })
    }
}
