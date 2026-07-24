use byteorder::{BigEndian, ReadBytesExt as _};
use serde::Serialize;
use std::io::{Read, Seek};

use crate::mp4box::{
    box_start, skip_bytes, skip_bytes_to, value_u32, BoxHeader, BoxType, Error, FixedPointU16,
    Mp4Box, ReadBox, Result, HEADER_EXT_SIZE, HEADER_SIZE,
};

/// MPEG-4 Part 2 / Visual (`mp4v`) visual sample entry.
///
/// Unlike `avc1`/`hvc1`/`av01`, the decoder configuration is not a self-contained
/// child box. It lives in the nested `esds` elementary-stream descriptor: the raw
/// bytes of the `DecoderSpecificInfo` (the Video Object Layer header) are what a
/// decoder needs as "extradata". `object_type_indication` is `0x20` for MPEG-4
/// Visual.
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

    /// Object type indication from the `esds` `DecoderConfigDescriptor`.
    ///
    /// `0x20` for MPEG-4 Visual (MPEG-4 Part 2).
    pub object_type_indication: u8,

    /// Raw bytes of the `DecoderSpecificInfo` descriptor (the VOL/VOS header).
    ///
    /// This is the codec configuration ("extradata") a decoder needs.
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
        // scan for `esds` rather than assuming it comes first.
        let end = start + size;
        let mut object_type_indication = 0;
        let mut config_raw = Vec::new();
        let mut found_esds = false;
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
                let (oti, cfg) = read_esds_config(reader, s)?;
                object_type_indication = oti;
                config_raw = cfg;
                found_esds = true;
            }
            skip_bytes_to(reader, current + s)?;
        }

        if !found_esds {
            return Err(Error::InvalidData("mp4v esds not found"));
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

/// Parse the `esds` box body, returning the object type indication and the raw
/// `DecoderSpecificInfo` bytes (the VOL/VOS header).
///
/// `reader` must be positioned just after the 8-byte box header; `size` is the
/// full box size. Descends `ES_Descriptor (0x03)` → `DecoderConfigDescriptor
/// (0x04)` → `DecoderSpecificInfo (0x05)`, reading only what we need.
fn read_esds_config<R: Read + Seek>(reader: &mut R, size: u64) -> Result<(u8, Vec<u8>)> {
    let start = box_start(reader)?;
    reader.read_u32::<BigEndian>()?; // version + flags

    let end = start + size;
    let mut object_type_indication = 0u8;
    let mut config_raw = Vec::new();

    if reader.stream_position()? >= end {
        return Ok((object_type_indication, config_raw));
    }

    let (tag, es_size) = read_descriptor_header(reader)?;
    if tag != 0x03 {
        return Ok((object_type_indication, config_raw));
    }
    let es_end = reader.stream_position()? + es_size as u64;
    reader.read_u16::<BigEndian>()?; // es_id
    reader.read_u8()?; // flags

    while reader.stream_position()? < es_end {
        let (tag, desc_size) = read_descriptor_header(reader)?;
        let desc_end = reader.stream_position()? + desc_size as u64;
        if tag == 0x04 {
            // DecoderConfigDescriptor: 13 fixed bytes, then nested descriptors.
            object_type_indication = reader.read_u8()?;
            reader.read_u8()?; // stream_type / up_stream / reserved
            reader.read_u24::<BigEndian>()?; // buffer_size_db
            reader.read_u32::<BigEndian>()?; // max_bitrate
            reader.read_u32::<BigEndian>()?; // avg_bitrate
            while reader.stream_position()? < desc_end {
                let (tag, ds_size) = read_descriptor_header(reader)?;
                if tag == 0x05 {
                    config_raw = vec![0u8; ds_size as usize];
                    reader.read_exact(&mut config_raw)?;
                } else {
                    skip_bytes(reader, ds_size as u64)?;
                }
            }
        } else {
            skip_bytes(reader, desc_size as u64)?;
        }
        skip_bytes_to(reader, desc_end)?;
    }

    Ok((object_type_indication, config_raw))
}

/// Read an MPEG-4 descriptor header: a 1-byte tag and a variable-length size
/// encoded 7 bits per byte (high bit continues).
fn read_descriptor_header<R: Read>(reader: &mut R) -> Result<(u8, u32)> {
    let tag = reader.read_u8()?;
    let mut size: u32 = 0;
    for _ in 0..4 {
        let b = reader.read_u8()?;
        size = (size << 7) | (b & 0x7F) as u32;
        if b & 0x80 == 0 {
            break;
        }
    }
    Ok((tag, size))
}

/// Number of bytes a descriptor's variable-length size field occupies.
fn descriptor_len_bytes(len: u32) -> u64 {
    match len {
        0..=0x7F => 1,
        0x80..=0x3FFF => 2,
        0x4000..=0x1F_FFFF => 3,
        _ => 4,
    }
}

/// Best-effort size of the `esds` box wrapping `config_raw`.
///
/// Parsing is exact; `mp4v` write support is approximate and unused by Rerun.
fn esds_box_size(config_raw: &[u8]) -> u64 {
    let ds = config_raw.len() as u32;
    let ds_desc = 1 + descriptor_len_bytes(ds) + ds as u64; // DecoderSpecificInfo (0x05)
    let dc = 13 + ds_desc; // DecoderConfigDescriptor (0x04) payload
    let dc_desc = 1 + descriptor_len_bytes(dc as u32) + dc;
    let sl_desc = 1 + 1 + 1; // SLConfigDescriptor (0x06): tag + len + 1 byte
    let es = 3 + dc_desc + sl_desc; // es_id(2) + flags(1) + children
    let es_desc = 1 + descriptor_len_bytes(es as u32) + es;
    HEADER_SIZE + HEADER_EXT_SIZE + es_desc
}
