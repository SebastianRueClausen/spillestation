pub struct MemCard {

}

impl MemCard {
    pub fn transfer(&mut self, _val: u8) -> (Option<u8>, bool) {
        (None, false)
    }
}
