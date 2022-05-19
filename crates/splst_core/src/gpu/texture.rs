use super::primitive::{Point, Texel};
use super::vram::Vram;
use super::TexelDepth;

pub struct ClutCache {
    data: [u16; 256],
    status: Option<(Point, TexelDepth)>,
}

impl ClutCache {
    // Maybe fetch a new cacheline. Should only be called when 'depth' is either 4 or 8.
    pub fn maybe_fetch(&mut self, pos: Point, depth: TexelDepth, vram: &Vram) {
        // The cache is already loaded.
        if let Some((prev_pos, prev_depth)) = self.status {
            // If the depth is lower or the same, and if the position matches, the cacheline is
            // intact.
            if depth as usize <= prev_depth as usize && pos == prev_pos {
                return;
            }
        }
        
        let load = match depth {
            TexelDepth::B4 => 16,
            TexelDepth::B8 => 256,
            TexelDepth::B15 => {
                unreachable!("trying to load clut cache when texel depth is 15 bits");
            }
        };
        
        self.status = Some((pos, depth));

        for (i, val) in self.data[..load].iter_mut().enumerate() {
            *val = vram.load_16(pos.x + i as i32, pos.y);
        }
    }
    
    pub fn clear(&mut self) {
        self.status = None;
    }
    
    pub fn get(&self, offset: i32) -> Texel {
        Texel::new(self.data[offset as usize])
    }
}

impl Default for ClutCache {
    fn default() -> Self {
        Self {
            status: None,
            data: [0x0; 256],
        }
    }
}
