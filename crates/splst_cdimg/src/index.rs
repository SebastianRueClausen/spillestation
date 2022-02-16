use splst_util::Bcd;
use splst_util::Msf;
use crate::{TrackFormat, TrackMode};
use crate::cd::{Toc, Track};

use std::cmp::Ordering;

pub struct Index<T> {
    pub sector: usize,
    pub index: Bcd,
    pub track: Bcd,
    pub format: TrackFormat,
    pub data: T 
}

impl<T> Index<T> {
    pub fn is_pregap(&self) -> bool {
        self.index == Bcd::ZERO
    }

    pub fn msf(&self) -> Msf {
        Msf::from_sector(self.sector).unwrap()
    }
}

impl<T> PartialEq for Index<T> {
    fn eq(&self, other: &Index<T>) -> bool {
        self.sector == other.sector
    }
}

impl<T> Eq for Index<T> { }

impl<T> PartialOrd for Index<T> {
    fn partial_cmp(&self, other: &Index<T>) -> Option<Ordering> {
        self.sector.partial_cmp(&other.sector)
    }
}

impl<T> Ord for Index<T> {
    fn cmp(&self, other: &Index<T>) -> Ordering {
        self.sector.cmp(&other.sector)
    }
}

pub struct IndexLookup<T> {
    indices: Vec<Index<T>>,
    first_in_lead_out: usize,
}

impl<T> IndexLookup<T> {
    pub fn new(mut indices: Vec<Index<T>>, first_in_lead_out: usize) -> Self {
        indices.sort();
        Self { indices, first_in_lead_out }
    }

    pub fn get(&self, idx: usize) -> Option<&Index<T>> {
        self.indices.get(idx)
    }

    pub fn get_from_msf(&self, msf: Msf) -> Option<(usize, &Index<T>)> {
        let sector = msf.sector();  
        if sector > self.first_in_lead_out {
            None
        } else {
            let i = self.indices.binary_search_by(|idx| {
                idx.sector.cmp(&sector)
            });
            // Get the index of the index. If it's can't find it, it returns the index of the first
            // element greater than the one we are searching for.
            let i = i.unwrap_or_else(|i| i - 1);
            Some((i, &self.indices[i]))
        }
    }
    
    pub fn get_from_track(&self, track: Bcd, index: Bcd) -> Option<(usize, &Index<T>)> {
        self.indices
            .binary_search_by(|idx| {
                match idx.track.cmp(&track) {
                    Ordering::Equal => idx.index.cmp(&index),
                    or => or,
                }
            })
            .map(|i| (i, &self.indices[i]))
            .ok()
    }

    fn track_len(&self, track: Bcd) -> Option<(Msf, &Index<T>)> {
        let (i, index) = self.get_from_track(track, Bcd::ONE)?;
        let next = self.indices[(i + 1)..].iter().find(|index| {
            index.track != track
        });
        let end = if let Some(next) = next {
            next.sector
        } else {
            self.first_in_lead_out
        };
        let len = Msf::from_sector(end - index.sector)?;
        Some((len, index))
    }

    pub fn build_toc(&self) -> Toc {
        let len = self.indices.iter()
            .last()
            .map(|i| i.track)
            .unwrap_or(Bcd::ZERO);
        let mut tracks = Vec::with_capacity(usize::from(len.as_binary()));
        let track_iter = (1..=99).map_while(|i| {
            let number = Bcd::from_binary(i).unwrap();
            self.track_len(number).map(|(length, index)| {
                Track {
                    format: index.format,
                    start: index.msf(),
                    number,
                    length,
                }
            })
        });
        tracks.extend(track_iter);
        Toc { tracks }
    }
}

#[derive(Clone, Copy)]
pub enum Storage {
    Binary {
        binary: usize,
        offset: usize,
        mode: TrackMode, 
    },
    PreGap,
}

pub struct Binary {
    pub data: Box<[u8]>,
}
