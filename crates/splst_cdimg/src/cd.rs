use splst_util::{Bcd, Msf};
use crate::index::{IndexLookup, Storage, Binary};
use crate::sector::{SectorDescriptor, Sector};
use crate::{Error, TrackFormat};

use std::path::{PathBuf, Path};

pub struct CdImage {
    indices: IndexLookup<Storage>,
    binaries: Vec<Binary>,
    name: String,
    path: PathBuf,
    toc: Toc,
}

impl CdImage {
    pub(super) fn new(
        path: &Path,
        indices: IndexLookup<Storage>,
        binaries: Vec<Binary>
    ) -> Self {
        let toc = indices.build_toc();
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .to_string();
        Self {
            indices,
            binaries,
            path: path.to_path_buf(),
            name,
            toc,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn toc(&self) -> &Toc {
        &self.toc
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
                self.indices
                    .get_from_track(idx.track, Bcd::ONE)
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

        let desc = SectorDescriptor {
            abs_msf: msf,
            track_msf,
            index: idx.index,
            track: idx.track,
            format: idx.format
        };

        Ok(Sector::new(&desc, data.into()))
    }
}

pub struct Track {
    pub number: Bcd,
    pub format: TrackFormat,
    pub start: Msf,
    pub length: Msf,
}

/// Table of content.
pub struct Toc {
    pub tracks: Vec<Track>,
}

impl Toc {
    pub fn tracks(&self) -> &Vec<Track> {
        &self.tracks
    }
}
