use splst_util::{Msf, Bcd, Bit};
use crate::TrackFormat;

pub enum SectorMode {
    Mode1,
    Mode2,
}

pub struct SectorHeader {
    pub msf: Msf,
    pub mode: SectorMode,
}

pub struct SectorDescriptor {
    pub abs_msf: Msf,
    pub track_msf: Msf,
    pub index: Bcd,
    pub track: Bcd,
    pub format: TrackFormat,
}

pub struct Sector {
    pub abs_msf: Msf,
    pub track_msf: Msf,
    pub index: Bcd,
    pub track: Bcd,
    pub format: TrackFormat,
    data: Box<[u8]>,
}

impl Sector {
    pub fn start_value() -> Self {
        Self {
            abs_msf: Msf::ZERO,
            track_msf: Msf::ZERO,
            index: Bcd::ZERO,
            track: Bcd::ZERO,
            format: TrackFormat::Audio,
            data: Box::new([0x0; 2352]),
        }
    }

    pub fn new(desc: &SectorDescriptor, mut data: Box<[u8]>) -> Self {
        let SectorDescriptor {
            abs_msf, track_msf, index, track, format
        } = *desc;

        // Genrate sector header.
        data[0] = 0x0;
        data[11] = 0x0;

        for i in 1..11 {
            data[i] = 0xff;
        }
        
        data[12] = abs_msf.min.raw();
        data[13] = abs_msf.sec.raw();
        data[14] = abs_msf.frame.raw();

        data[15] = match format {
            TrackFormat::Audio => todo!(),
            TrackFormat::Mode1 => 1,
            TrackFormat::Mode2Xa => 2,
        };

        Self {
            abs_msf, track_msf, index, track, format, data
        }
    }

    pub fn header(&self) -> Option<SectorHeader> {
        let header = &self.data[0..16];

        // TODO: Should perhaps validate here.
       
        let m = Bcd::from_bcd(self.data[12])?;
        let s = Bcd::from_bcd(self.data[13])?;
        let f = Bcd::from_bcd(self.data[14])?;

        let msf = Msf::from_bcd(m, s, f);
        let mode = match header[15] {
            1 => SectorMode::Mode1,
            2 => SectorMode::Mode2,
            _ => return None,
        };

        Some(SectorHeader { msf, mode })
    }

    pub fn xa_header(&self) -> Option<XaHeader> {
        match self.format {
            TrackFormat::Mode1 | TrackFormat::Audio => None,
            TrackFormat::Mode2Xa => {
                Some(XaHeader::new(&self.data[8..16]))
            }
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn xa_data(&self) -> Option<&[u8]> {
        match self.xa_header()?.mode().form() {
            XaForm::Form1 => Some(&self.data[24..2072]),
            XaForm::Form2 => Some(&self.data[24..2348]),
        }
    }
}

pub enum XaForm {
    Form1,
    Form2,
}

#[derive(Clone, Copy)]
pub struct XaMode(u8);

impl XaMode {
    pub fn eor(self) -> bool {
        self.0.bit(0)
    }

    pub fn video(self) -> bool {
        self.0.bit(1)
    }

    pub fn audio(self) -> bool {
        self.0.bit(2)
    }

    pub fn data(self) -> bool {
        self.0.bit(3)
    }

    pub fn trigger(self) -> bool {
        self.0.bit(4)
    }

    pub fn form(self) -> XaForm {
        match self.0.bit(5) {
            false => XaForm::Form1,
            true => XaForm::Form2,
        }
    }

    pub fn real_time(self) -> bool {
        self.0.bit(6)
    }

    pub fn eof(self) -> bool {
        self.0.bit(7)
    }
}

#[derive(Clone, Copy)]
pub enum XaSampleHz {
    Hz37800,
    Hz18900,
}

#[derive(Clone, Copy)]
pub enum XaSampleSize {
    Bit4 = 4,
    Bit8 = 8,
}

#[derive(Clone, Copy)]
pub struct XaVideoEncoding(u8);

#[derive(Clone, Copy)]
pub struct XaAudioEncoding(u8);

impl XaAudioEncoding {
    pub fn stereo(self) -> bool {
        self.0.bit(0)
    }

    pub fn sample_hz(self) -> XaSampleHz {
        match self.0.bit(2) {
            false => XaSampleHz::Hz37800,
            true => XaSampleHz::Hz18900,
        }
    }

    pub fn sample_size(self) -> XaSampleSize {
        match self.0.bit(4) {
            false => XaSampleSize::Bit4,
            true => XaSampleSize::Bit8,
        }
    }

    pub fn emphasis(self) -> bool {
        self.0.bit(6)
    }
}

pub enum XaEncoding {
   Video(XaVideoEncoding), 
   Audio(XaAudioEncoding), 
   Invalid,
}

pub struct XaHeader([u8; 8]);

impl XaHeader {
    fn new(header: &[u8]) -> Self {
        debug_assert_eq!(header.len(), 8);
        let mut data = [0; 8];
        data.clone_from_slice(&header);
        XaHeader(data)
    }

    pub fn file_num(&self) -> u8 {
        self.0[0]
    }

    pub fn channel_num(&self) -> u8 {
        self.0[1]
    }

    pub fn mode(&self) -> XaMode {
        XaMode(self.0[2])
    }

    pub fn encoding(&self) -> XaEncoding {
        let mode = self.mode();
        let encoding = self.0[3];
        if mode.video() {
            XaEncoding::Video(XaVideoEncoding(encoding))
        } else if mode.audio() {
            XaEncoding::Audio(XaAudioEncoding(encoding))
        } else {
            XaEncoding::Invalid
        }
    }
}
