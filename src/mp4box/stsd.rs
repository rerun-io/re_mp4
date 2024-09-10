use byteorder::{BigEndian, ReadBytesExt};
use serde::Serialize;
use std::io::{Read, Seek};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct StsdBox {
    pub version: u8,
    pub flags: u32,

    /// AV1 video codec
    #[serde(skip_serializing_if = "Option::is_none")]
    pub av01: Option<Av01Box>,

    /// AVC video codec (h.264)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avc1: Option<Avc1Box>,

    /// HEVC video codec (h.265)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hvc1: Option<Hvc1Box>,

    /// VP8 video codec
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vp08: Option<Vp08Box>,

    /// VP9 video codec
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vp09: Option<Vp09Box>,

    /// AAC audio codec
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mp4a: Option<Mp4aBox>,

    /// TTXT subtitle codec
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx3g: Option<Tx3gBox>,
}

impl StsdBox {
    pub fn kind(&self) -> TrackKind {
        if self.av01.is_some()
            || self.avc1.is_some()
            || self.hvc1.is_some()
            || self.vp08.is_some()
            || self.vp09.is_some()
        {
            TrackKind::Video
        } else if self.mp4a.is_some() {
            TrackKind::Audio
        } else if self.tx3g.is_some() {
            TrackKind::Subtitle
        } else {
            TrackKind::Video
        }
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::StsdBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE + HEADER_EXT_SIZE + 4;
        if let Some(ref av01) = self.av01 {
            size += av01.box_size();
        } else if let Some(ref avc1) = self.avc1 {
            size += avc1.box_size();
        } else if let Some(ref hvc1) = self.hvc1 {
            size += hvc1.box_size();
        } else if let Some(ref vp09) = self.vp09 {
            size += vp09.box_size();
        } else if let Some(ref mp4a) = self.mp4a {
            size += mp4a.box_size();
        } else if let Some(ref tx3g) = self.tx3g {
            size += tx3g.box_size();
        }
        size
    }
}

impl Mp4Box for StsdBox {
    fn box_type(&self) -> BoxType {
        self.get_type()
    }

    fn box_size(&self) -> u64 {
        self.get_size()
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        let s = String::new();
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for StsdBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = read_box_header_ext(reader)?;

        reader.read_u32::<BigEndian>()?; // XXX entry_count

        let mut av01 = None;
        let mut avc1 = None;
        let mut hvc1 = None;
        let mut vp08 = None;
        let mut vp09 = None;
        let mut mp4a = None;
        let mut tx3g = None;

        // Get box header.
        let header = BoxHeader::read(reader)?;
        let BoxHeader { name, size: s } = header;
        if s > size {
            return Err(Error::InvalidData(
                "stsd box contains a box with a larger size than it",
            ));
        }

        match name {
            BoxType::Av01Box => {
                av01 = Some(Av01Box::read_box(reader, s)?);
            }
            // According to MPEG-4 part 15, sections 5.4.2.1.2 and 5.4.4 (or the whole 5.4 section in general),
            // the Avc1Box and Avc3Box are identical, but the Avc3Box is used in some cases.
            BoxType::Avc1Box => {
                avc1 = Some(Avc1Box::read_box(reader, s)?);
            }
            BoxType::Hvc1Box => {
                hvc1 = Some(Hvc1Box::read_box(reader, s)?);
            }
            BoxType::Vp08Box => {
                vp08 = Some(Vp08Box::read_box(reader, s)?);
            }
            BoxType::Vp09Box => {
                vp09 = Some(Vp09Box::read_box(reader, s)?);
            }
            BoxType::Mp4aBox => {
                mp4a = Some(Mp4aBox::read_box(reader, s)?);
            }
            BoxType::Tx3gBox => {
                tx3g = Some(Tx3gBox::read_box(reader, s)?);
            }
            _ => {}
        }

        skip_bytes_to(reader, start + size)?;

        Ok(StsdBox {
            version,
            flags,
            av01,
            avc1,
            hvc1,
            vp08,
            vp09,
            mp4a,
            tx3g,
        })
    }
}
