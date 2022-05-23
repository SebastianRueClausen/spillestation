use super::AddrUnit;

pub struct RawMem<const SIZE: usize> {
    data: Box<[u8; SIZE]>,
}

impl<const SIZE: usize> RawMem<SIZE> {
    pub fn new() -> Self {
        Self {
            data: Box::new([0xff; SIZE]),
        }
    }

    #[inline]
    pub unsafe fn load_unchecked<T: AddrUnit>(&self, addr: u32) -> T {
        let val: u32 = (0..T::WIDTH as usize).fold(0, |val, byte| {
            let get = *self.data.get_unchecked(addr as usize + byte) as u32;
            val | get << (8 * byte)
        });
        T::from_u32(val)
    }

    #[inline]
    pub unsafe fn store_unchecked<T: AddrUnit>(&mut self, offset: u32, val: T) {
        let val = val.as_u32();
        let offset = offset as usize;
        for byte in 0..T::WIDTH as usize {
            *self.data.get_unchecked_mut(offset + byte) = (val >> (8 * byte)) as u8;
        }
    }

    #[inline]
    pub fn load<T: AddrUnit>(&self, offset: u32) -> T {
        let offset = offset as usize;
        let val: u32 = (0..T::WIDTH as usize).fold(0, |val, byte| {
            val | (self.data[offset + byte] as u32) << (8 * byte)
        });
        T::from_u32(val)
    }

    #[inline]
    pub fn store<T: AddrUnit>(&mut self, offset: u32, val: T) {
        let val = val.as_u32();
        let offset = offset as usize;
        for byte in 0..T::WIDTH as usize {
            self.data[offset + byte] = (val >> (8 * byte)) as u8;
        }
    }
}
