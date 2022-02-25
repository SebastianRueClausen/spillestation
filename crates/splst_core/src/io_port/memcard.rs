pub struct MemCard {

}

impl MemCard {
    pub fn transfer(&mut self, _val: u8) -> (u8, bool) {
        (0xff, false)
    }
}
