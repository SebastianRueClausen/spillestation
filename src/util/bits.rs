/// Trait to extract a value between two given bit positions.
pub trait BitExtract {
    /// Extract a single bit.
    fn extract_bit(self, n: u32) -> Self;

    /// Extract a range of bits. Both are inclusive.
    fn extract_bits(self, a: u32, b: u32) -> Self;
}

impl BitExtract for u32 {
    fn extract_bit(self, n: u32) -> u32 {
        (self >> n) & 1
    }

    fn extract_bits(self, a: u32, b: u32) -> u32 {
        let mask = ((1 << (b - a + 1)) - 1) << a;
        (self & mask) >> a
    }
}

impl BitExtract for u16 {
    fn extract_bit(self, n: u32) -> u16 {
        (self >> n) & 1
    }

    fn extract_bits(self, a: u32, b: u32) -> u16 {
        let mask = ((1 << (b - a + 1)) - 1) << a;
        (self & mask) >> a
    }
}
