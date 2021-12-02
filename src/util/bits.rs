
pub trait BitExtract {
    fn extract_bit(self, n: u32) -> u32;
    fn extract_bits(self, a: u32, b: u32) -> u32;
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
