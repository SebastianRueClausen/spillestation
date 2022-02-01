use crate::msf::Msf;
use crate::bcd::Bcd;
use crate::TrackFormat;

pub enum SectorMode {
    Mode1,
    Mode2,
}

pub struct SectorHeader {
    pub msf: Msf,
    pub mode: SectorMode,
}

pub struct Sector {
    pub abs_msf: Msf,
    pub track_msf: Msf,
    pub index: Bcd,
    pub track: Bcd,
    pub format: TrackFormat,
    pub data: Box<[u8]>,
}

impl Sector {
    pub fn generate_cdrom_header(&mut self) {
        self.data[0] = 0x0;
        self.data[11] = 0x0;

        for i in 1..11 {
            self.data[i] = 0xff;
        }
        
        self.data[12] = self.abs_msf.min.raw();
        self.data[13] = self.abs_msf.sec.raw();
        self.data[14] = self.abs_msf.frame.raw();

        self.data[15] = match self.format {
            TrackFormat::Audio => todo!(),
            TrackFormat::Mode1 => 1,
            TrackFormat::Mode2Xa => 2,
        };
    }
}
