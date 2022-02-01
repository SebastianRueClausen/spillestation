use crate::msf::Msf;
use crate::bcd::Bcd;
use crate::index::{IndexLookup, Storage, Binary};
use crate::sector::Sector;
use crate::{Error, TrackFormat};

pub struct Cd {
    indices: IndexLookup<Storage>,
    binaries: Vec<Binary>,
    pub toc: Toc,
}

impl Cd {
    pub(super) fn new(indices: IndexLookup<Storage>, binaries: Vec<Binary>) -> Self {
        let toc = indices.build_toc();
        Self { indices, binaries, toc }
    }

    pub fn load_sector(&self, msf: Msf) -> Result<Sector, Error> {
        let Some((i, idx)) = self.indices.get_from_msf(msf) else {
            return Err(Error::SectionReadError);
        };

        let track_msf = if idx.is_pregap() {
            // There shouldn't be a gregap without any index following it.
            self.indices
                .get(i + 1)
                .expect("pregap is last index")
                .msf() - msf
        } else {
            let one = if idx.index == Bcd::ONE {
                idx 
            } else {
                self.indices.get_from_track(idx.track, Bcd::ONE)
                    .map(|(_, i)| i)
                    .expect("track without index one")
            };
            msf - one.msf()
        };

        let data = match idx.data {
            Storage::Binary { binary, offset, mode } => {
                let binary = &self.binaries[binary];
                let start = offset
                    + mode.sector_size()
                    * (msf.sector() - idx.sector);
                let end = start + mode.sector_size();
                &binary.data[start..end]
            }
            Storage::PreGap => &[0x0; 2352],
        };

        let mut sector = Sector {
            abs_msf: msf,
            track_msf,
            index: idx.index,
            track: idx.track,
            format: idx.format,
            data: data.into(),
        };

        sector.generate_cdrom_header();

        Ok(sector)
    }
}

pub struct Track {
    pub number: Bcd,
    pub format: TrackFormat,
    pub start: Msf,
    pub length: Msf,
}

pub struct Toc {
    pub tracks: Vec<Track>,
}
