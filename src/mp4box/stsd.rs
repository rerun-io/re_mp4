use byteorder::{BigEndian, ReadBytesExt};
use serde::Serialize;
use std::io::{Read, Seek};

use crate::mp4box::{
    box_start, read_box_header_ext, skip_bytes_to, Av01Box, Avc1Box, BoxHeader, BoxType, Error,
    FourCC, Hvc1Box, Mp4Box, Mp4aBox, ReadBox, Result, TrackKind, Tx3gBox, Vp08Box, Vp09Box,
    HEADER_EXT_SIZE, HEADER_SIZE,
};

/// Codec dependent contents of the stsd box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum StsdBoxContent {
    /// AV1 video codec
    Av01(Av01Box),

    /// AVC video codec (h.264)
    Avc1(Avc1Box),

    /// HVC1 video codec (h.265)
    ///
    /// h.265 comes in two flavors: hev1 and hvc1.
    ///
    /// hvc1 parameter sets are stored out-of-band in the sample entry
    /// (i.e. below the Sample Description Box ( stsd ) box)
    ///
    /// hev1 parameter sets are stored out-of-band in the sample entry and/or in-band in the samples
    /// (i.e. SPS/PPS/VPS NAL units in the bitstream/ mdat box)
    Hvc1(Hvc1Box),

    /// VP8 video codec
    Vp08(Vp08Box),

    /// VP9 video codec
    Vp09(Vp09Box),

    /// AAC audio codec
    Mp4a(Mp4aBox),

    /// TTXT subtitle codec
    Tx3g(Tx3gBox),

    /// Unrecognized codecs
    Unknown(FourCC),
}

impl Default for StsdBoxContent {
    fn default() -> Self {
        Self::Unknown(FourCC::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct StsdBox {
    pub version: u8,
    pub flags: u32,
    pub contents: StsdBoxContent,
}

impl StsdBox {
    pub fn kind(&self) -> Option<TrackKind> {
        match &self.contents {
            StsdBoxContent::Av01(_) => Some(TrackKind::Video),
            StsdBoxContent::Avc1(_) => Some(TrackKind::Video),
            StsdBoxContent::Hvc1(_) => Some(TrackKind::Video),
            StsdBoxContent::Vp08(_) => Some(TrackKind::Video),
            StsdBoxContent::Vp09(_) => Some(TrackKind::Video),
            StsdBoxContent::Mp4a(_) => Some(TrackKind::Audio),
            StsdBoxContent::Tx3g(_) => Some(TrackKind::Subtitle),
            StsdBoxContent::Unknown(_) => None,
        }
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::StsdBox
    }

    pub fn get_size(&self) -> u64 {
        HEADER_SIZE
            + HEADER_EXT_SIZE
            + 4
            + match &self.contents {
                StsdBoxContent::Av01(contents) => contents.box_size(),
                StsdBoxContent::Avc1(contents) => contents.box_size(),
                StsdBoxContent::Hvc1(contents) => contents.box_size(),
                StsdBoxContent::Vp08(contents) => contents.box_size(),
                StsdBoxContent::Vp09(contents) => contents.box_size(),
                StsdBoxContent::Mp4a(contents) => contents.box_size(),
                StsdBoxContent::Tx3g(contents) => contents.box_size(),
                StsdBoxContent::Unknown(four_cc) => 0,
            }
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
        Ok(serde_json::to_string(&self).expect("Failed to convert to JSON"))
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

        // Get box header.
        let header = BoxHeader::read(reader)?;
        let BoxHeader { name, size: s } = header;
        if s > size {
            return Err(Error::InvalidData(
                "stsd box contains a box with a larger size than it",
            ));
        }

        let contents = match name {
            BoxType::Av01Box => StsdBoxContent::Av01(Av01Box::read_box(reader, s)?),
            // According to MPEG-4 part 15, sections 5.4.2.1.2 and 5.4.4 (or the whole 5.4 section in general),
            // the Avc1Box and Avc3Box are identical, but the Avc3Box is used in some cases.
            BoxType::Avc1Box => StsdBoxContent::Avc1(Avc1Box::read_box(reader, s)?),
            BoxType::Hvc1Box => StsdBoxContent::Hvc1(Hvc1Box::read_box(reader, s)?),
            BoxType::Vp08Box => StsdBoxContent::Vp08(Vp08Box::read_box(reader, s)?),
            BoxType::Vp09Box => StsdBoxContent::Vp09(Vp09Box::read_box(reader, s)?),
            BoxType::Mp4aBox => StsdBoxContent::Mp4a(Mp4aBox::read_box(reader, s)?),
            BoxType::Tx3gBox => StsdBoxContent::Tx3g(Tx3gBox::read_box(reader, s)?),
            _ => StsdBoxContent::Unknown(name.into()),
        };

        skip_bytes_to(reader, start + size)?;

        Ok(Self {
            version,
            flags,
            contents,
        })
    }
}
