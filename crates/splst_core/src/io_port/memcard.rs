
pub struct MemCard {
    flash: Box<[u8; FLASH_SIZE]>,
    write_counter: u32,
    dirty: bool,
    access: Access,
    sector: u16,
    
}

impl Default for MemCard {
    fn default() -> Self {
        Self::new(formatted())
    }
}

impl MemCard {
    pub fn new(flash: Box<[u8; FLASH_SIZE]>) -> Self {
        Self {
            flash,
            write_counter: 0,
            dirty: false,
            access: Access::Id,
            sector: 0,
        }
    }
    
    pub fn transfer(&mut self, _val: u8) -> (u8, bool) {
        (0xff, false)
    }
}

enum Access {
    Read = b'R' as isize,
    Write = b'W' as isize,
    Id = b'S' as isize,
}

fn formatted() -> Box<[u8; FLASH_SIZE]> {
    let mut flash = Box::new([0x0; FLASH_SIZE]);
    
    // Magic values in the first block.
    flash[0] = b'M';
    flash[1] = b'C';
    
    // The checksum of the first block.
    flash[127] = b'M' ^ b'C';
    
    // format the rest of the blocks.
    for block in 1..16 {
        let start = block * SECTOR_SIZE;
        let end = start + SECTOR_SIZE;
        
        let data = &mut flash[start..end];
        
        // Set status as free.
        data[0] = 0xa0;

        // Set next pointer to none.
        data[8] = 0xff;
        data[9] = 0xff;
        
        // Set checksum.
        data[127] = 0xa0 ^ 0xff ^ 0xff;
    }
    
    // Set broken sector list.
    for off in 16..36 {
        let start = off * SECTOR_SIZE;
        let end = (off + 1) * SECTOR_SIZE;

        let sector = &mut flash[start..end];

        // Set sector position to none.
        sector[0] = 0xff;
        sector[1] = 0xff;
        sector[2] = 0xff;
        sector[3] = 0xff;

        // Not sure what this is but the card I'm using has those two bytes set as well. It's
        // at the same position as the next block pointer in block entries but it doesn't make
        // a lot of sense here.
        sector[8] = 0xff;
        sector[9] = 0xff;
    }
    
    flash
}

fn checksum(mem: &[u8]) -> u8 {
    mem.iter().fold(0, |sum, val| sum ^ val)
}

const SECTOR_SIZE: usize = 128;
const BLOCK_SIZE: usize = 64 * SECTOR_SIZE;
const FLASH_SIZE: usize = 16 * BLOCK_SIZE;
    
