use byteorder::{BigEndian, ReadBytesExt as _};
use serde::Serialize;
use std::io::{Read, Seek};

use crate::mp4box::{
    box_start, mp4a::size_of_length, mp4a::EsdsBox, skip_bytes, skip_bytes_to, value_u32, BoxHeader,
    BoxType, Error, FixedPointU16, Mp4Box, ReadBox, Result, HEADER_EXT_SIZE, HEADER_SIZE,
};

/// MPEG-4 Part 2 / Visual (`mp4v`) visual sample entry.
///
/// Unlike `avc1`/`hvc1`/`av01`, the decoder configuration is not a self-contained
/// child box. It lives in the nested `esds` descriptor, whose `DecoderSpecificInfo`
/// holds the Video Object Layer header a decoder needs as extradata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mp4vBox {
    pub data_reference_index: u16,
    pub width: u16,
    pub height: u16,

    #[serde(with = "value_u32")]
    pub horizresolution: FixedPointU16,

    #[serde(with = "value_u32")]
    pub vertresolution: FixedPointU16,
    pub frame_count: u16,
    pub depth: u16,

    /// Object type indication from the `esds` `DecoderConfigDescriptor` (`0x20` for MPEG-4 Visual).
    pub object_type_indication: u8,

    /// Raw `DecoderSpecificInfo` bytes: the Video Object Layer header used as extradata.
    pub config_raw: Vec<u8>,
}

impl Default for Mp4vBox {
    fn default() -> Self {
        Self {
            data_reference_index: 0,
            width: 0,
            height: 0,
            horizresolution: FixedPointU16::new(0x48),
            vertresolution: FixedPointU16::new(0x48),
            frame_count: 1,
            depth: 0x0018,
            object_type_indication: 0x20,
            config_raw: Vec::new(),
        }
    }
}

impl Mp4vBox {
    pub fn get_type() -> BoxType {
        BoxType::Mp4vBox
    }

    pub fn get_size(&self) -> u64 {
        HEADER_SIZE + 8 + 70 + esds_box_size(&self.config_raw)
    }
}

impl Mp4Box for Mp4vBox {
    fn box_type(&self) -> BoxType {
        Self::get_type()
    }

    fn box_size(&self) -> u64 {
        self.get_size()
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).expect("Failed to convert to JSON"))
    }

    fn summary(&self) -> Result<String> {
        let s = format!(
            "data_reference_index={} width={} height={} frame_count={} object_type_indication={:#04x}",
            self.data_reference_index,
            self.width,
            self.height,
            self.frame_count,
            self.object_type_indication
        );
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for Mp4vBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;

        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        reader.read_u64::<BigEndian>()?; // pre-defined
        reader.read_u32::<BigEndian>()?; // pre-defined
        let width = reader.read_u16::<BigEndian>()?;
        let height = reader.read_u16::<BigEndian>()?;
        let horizresolution = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);
        let vertresolution = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);
        reader.read_u32::<BigEndian>()?; // reserved
        let frame_count = reader.read_u16::<BigEndian>()?;
        skip_bytes(reader, 32)?; // compressorname
        let depth = reader.read_u16::<BigEndian>()?;
        reader.read_i16::<BigEndian>()?; // pre-defined

        // A visual sample entry may carry other children (`pasp`, `btrt`, …), so
        // scan for `esds` rather than assuming it comes first. The nested descriptor
        // tree is the same one AAC uses, so reuse `EsdsBox` to parse it.
        let end = start + size;
        let mut esds = None;
        loop {
            let current = reader.stream_position()?;
            if current >= end {
                break;
            }
            let BoxHeader { name, size: s } = BoxHeader::read(reader)?;
            if s > size {
                return Err(Error::InvalidData(
                    "mp4v box contains a box with a larger size than it",
                ));
            }
            if name == BoxType::EsdsBox {
                esds = Some(EsdsBox::read_box(reader, s)?);
            }
            skip_bytes_to(reader, current + s)?;
        }

        let Some(esds) = esds else {
            return Err(Error::InvalidData("mp4v esds not found"));
        };
        let object_type_indication = esds.es_desc.dec_config.object_type_indication;
        let config_raw = esds.es_desc.dec_config.dec_specific.raw;
        if config_raw.is_empty() {
            return Err(Error::InvalidData("mp4v esds has no decoder config"));
        }

        skip_bytes_to(reader, start + size)?;

        Ok(Self {
            data_reference_index,
            width,
            height,
            horizresolution,
            vertresolution,
            frame_count,
            depth,
            object_type_indication,
            config_raw,
        })
    }
}

/// Size of the `esds` box that would wrap `config_raw`.
///
/// The crate has no writer, so this only satisfies the `Mp4Box::box_size` trait
/// method and is never serialized.
fn esds_box_size(config_raw: &[u8]) -> u64 {
    let ds = config_raw.len() as u32;
    let ds_desc = 1 + size_of_length(ds) as u64 + ds as u64; // DecoderSpecificInfo (0x05)
    let dc = 13 + ds_desc; // DecoderConfigDescriptor (0x04) payload
    let dc_desc = 1 + size_of_length(dc as u32) as u64 + dc;
    let sl_desc = 1 + 1 + 1; // SLConfigDescriptor (0x06): tag + len + 1 byte
    let es = 3 + dc_desc + sl_desc; // es_id(2) + flags(1) + children
    let es_desc = 1 + size_of_length(es as u32) as u64 + es;
    HEADER_SIZE + HEADER_EXT_SIZE + es_desc
}
