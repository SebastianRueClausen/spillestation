use splst_util::Bit;

use std::ops::Index;

const FIFO_SIZE: usize = 16;

/// FIFO used by the SPU and CD-ROM. The head and tail is 4 bits with one carry bit.
pub struct Fifo<T: Copy + Clone + Default, const SIZE: usize> {
   data: [T; SIZE],
   head: u8,
   tail: u8,
}

impl<T: Clone + Copy + Default, const SIZE: usize> Default for Fifo<T, SIZE> {
    fn default() -> Self {
        Self {
            data: [T::default(); SIZE],
            head: 0,
            tail: 0,
        }
    }
}

impl<T: Clone + Copy + Default, const SIZE: usize> Fifo<T, SIZE> {
    pub fn new() -> Self {
        Self {
            data: [T::default(); SIZE],
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

    pub fn push(&mut self, val: T) {
        debug_assert!(!self.is_full());
        let idx: usize = self.head
            .bit_range(0, 3)
            .into();
        self.head = self.head
            .wrapping_add(1)
            .bit_range(0, 3);
        self.data[idx] = val;
    }

    pub fn push_slice(&mut self, values: &[T]) {
        for val in values {
            self.push(*val);
        }
    }

    pub fn pop(&mut self) -> T {
        debug_assert!(!self.is_empty());
        let idx: usize = self.tail
            .bit_range(0, 3)
            .into();
        self.tail = self.tail
            .wrapping_add(1)
            .bit_range(0, 3);
        self.data[idx]
    }

    pub fn pop_n(&mut self, n: usize) {
        debug_assert!(self.len() >= n);
        self.tail = self.tail.wrapping_add(n as u8).bit_range(0, 3);
    }

    pub fn try_pop(&mut self) -> Option<T> {
        self.is_empty().then(|| self.pop())
    }

    pub fn peek(&self) -> Option<T> {
        self.is_empty().then(|| {
            self.data[self.tail.bit_range(0, 3) as usize]
        })
    }
}

impl<T: Clone + Copy + Default, const SIZE: usize> Index<usize> for Fifo<T, SIZE> {
    type Output = T;

    fn index(&self, idx: usize) -> &Self::Output {
        debug_assert!(idx < self.len());
        let idx = self.tail.wrapping_add(idx as u8) as usize % SIZE;
        &self.data[idx]
    }
}
