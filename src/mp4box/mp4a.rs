use byteorder::{BigEndian, ReadBytesExt as _};
use serde::Serialize;
use std::io::{Read, Seek};

use crate::mp4box::{
    box_start, read_box_header_ext, skip_bytes, skip_bytes_to, value_u32, BoxHeader, BoxType,
    Error, FixedPointU16, Mp4Box, ReadBox, Result,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mp4aBox {
    pub data_reference_index: u16,
    pub channelcount: u16,
    pub samplesize: u16,

    #[serde(with = "value_u32")]
    pub samplerate: FixedPointU16,
    pub esds: Option<EsdsBox>,
}

impl Default for Mp4aBox {
    fn default() -> Self {
        Self {
            data_reference_index: 0,
            channelcount: 2,
            samplesize: 16,
            samplerate: FixedPointU16::new(48000),
            esds: Some(EsdsBox::default()),
        }
    }
}

impl Mp4aBox {
    pub fn get_type() -> BoxType {
        BoxType::Mp4aBox
    }
}

impl Mp4Box for Mp4aBox {
    fn box_type(&self) -> BoxType {
        Self::get_type()
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).expect("Failed to convert to JSON"))
    }

    fn summary(&self) -> Result<String> {
        let s = format!(
            "channel_count={} sample_size={} sample_rate={}",
            self.channelcount,
            self.samplesize,
            self.samplerate.value()
        );
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for Mp4aBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;
        let version = reader.read_u16::<BigEndian>()?;
        reader.read_u16::<BigEndian>()?; // reserved / revision
        reader.read_u32::<BigEndian>()?; // reserved / vendor
        let mut channelcount = reader.read_u16::<BigEndian>()?;
        let mut samplesize = reader.read_u16::<BigEndian>()?;
        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        let mut samplerate = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);

        if version == 1 {
            // Skip QTFF SoundDescriptionV1 extension (16 bytes).
            reader.read_u64::<BigEndian>()?;
            reader.read_u64::<BigEndian>()?;
        } else if version == 2 {
            // QTFF SoundDescriptionV2: the ISO-style fields above land on
            // `sizeOfStructOnly`. Child atoms (usually `wave`) start at that
            // absolute offset from the atom start. Without this skip, the
            // float64 sample rate is misread as a box header (see #32).
            let size_of_struct_only = u64::from(reader.read_u32::<BigEndian>()?);
            if size_of_struct_only < HEADER_SIZE || size_of_struct_only > size {
                return Err(Error::InvalidData("invalid mp4a sizeOfStructOnly"));
            }
            let struct_end = start + size_of_struct_only;
            // Remaining V2 fields before extensions: rate, channels, …
            if reader.stream_position()? + 8 + 4 <= struct_end {
                let rate = f64::from_bits(reader.read_u64::<BigEndian>()?);
                let channels = reader.read_u32::<BigEndian>()?;
                channelcount = u16::try_from(channels).unwrap_or(u16::MAX);
                if rate.is_finite() && rate > 0.0 && rate <= f64::from(u16::MAX) {
                    samplerate = FixedPointU16::new(rate as u16);
                }
                // Prefer bits-per-channel when present (after always0x7F000000).
                if reader.stream_position()? + 8 <= struct_end {
                    reader.read_u32::<BigEndian>()?; // always 0x7F000000
                    let bits = reader.read_u32::<BigEndian>()?;
                    if bits > 0 {
                        samplesize = u16::try_from(bits).unwrap_or(samplesize);
                    }
                }
            }
            if struct_end < reader.stream_position()? {
                return Err(Error::InvalidData("mp4a sizeOfStructOnly before cursor"));
            }
            skip_bytes_to(reader, struct_end)?;
        }

        // Find esds in mp4a or wave
        let mut esds = None;
        let end = start + size;
        loop {
            let current = reader.stream_position()?;
            if current >= end {
                break;
            }
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "mp4a box contains a box with a larger size than it",
                ));
            }
            if name == BoxType::EsdsBox {
                esds = Some(EsdsBox::read_box(reader, s)?);
                break;
            } else if name == BoxType::WaveBox {
                // QT: frma / mp4a / esds / terminator nested inside wave.
                let wave_end = current + s;
                while reader.stream_position()? < wave_end {
                    let inner_pos = reader.stream_position()?;
                    let inner = BoxHeader::read(reader)?;
                    if inner.size > s {
                        return Err(Error::InvalidData(
                            "wave box contains a box with a larger size than it",
                        ));
                    }
                    if inner.name == BoxType::EsdsBox {
                        esds = Some(EsdsBox::read_box(reader, inner.size)?);
                        break;
                    }
                    skip_bytes_to(reader, inner_pos + inner.size)?;
                }
                skip_bytes_to(reader, wave_end)?;
                if esds.is_some() {
                    break;
                }
            } else {
                // Skip boxes
                let skip_to = current + s;
                skip_bytes_to(reader, skip_to)?;
            }
        }

        skip_bytes_to(reader, end)?;

        Ok(Self {
            data_reference_index,
            channelcount,
            samplesize,
            samplerate,
            esds,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct EsdsBox {
    pub version: u8,
    pub flags: u32,
    pub es_desc: ESDescriptor,
}

impl Mp4Box for EsdsBox {
    fn box_type(&self) -> BoxType {
        BoxType::EsdsBox
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).expect("Failed to convert to JSON"))
    }

    fn summary(&self) -> Result<String> {
        Ok(String::new())
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for EsdsBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = read_box_header_ext(reader)?;

        let mut es_desc = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            let (desc_tag, desc_size) = read_desc(reader)?;
            match desc_tag {
                0x03 => {
                    es_desc = Some(ESDescriptor::read_desc(reader, desc_size)?);
                }
                _ => break,
            }
            current = reader.stream_position()?;
        }

        let Some(es_desc) = es_desc else {
            return Err(Error::InvalidData("ESDescriptor not found"));
        };

        skip_bytes_to(reader, start + size)?;

        Ok(Self {
            version,
            flags,
            es_desc,
        })
    }
}

trait ReadDesc<T>: Sized {
    fn read_desc(_: T, size: u32) -> Result<Self>;
}

fn read_desc<R: Read>(reader: &mut R) -> Result<(u8, u32)> {
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ESDescriptor {
    pub es_id: u16,

    pub dec_config: DecoderConfigDescriptor,
    pub sl_config: SLConfigDescriptor,
}

impl<R: Read + Seek> ReadDesc<&mut R> for ESDescriptor {
    fn read_desc(reader: &mut R, size: u32) -> Result<Self> {
        let start = reader.stream_position()?;

        let es_id = reader.read_u16::<BigEndian>()?;
        reader.read_u8()?; // XXX flags must be 0

        let mut dec_config = None;
        let mut sl_config = None;

        let mut current = reader.stream_position()?;
        let end = start + size as u64;
        while current < end {
            let (desc_tag, desc_size) = read_desc(reader)?;
            match desc_tag {
                0x04 => {
                    dec_config = Some(DecoderConfigDescriptor::read_desc(reader, desc_size)?);
                }
                0x06 => {
                    sl_config = Some(SLConfigDescriptor::read_desc(reader, desc_size)?);
                }
                _ => {
                    skip_bytes(reader, desc_size as u64)?;
                }
            }
            current = reader.stream_position()?;
        }

        Ok(Self {
            es_id,
            dec_config: dec_config.unwrap_or_default(),
            sl_config: sl_config.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct DecoderConfigDescriptor {
    pub object_type_indication: u8,
    pub stream_type: u8,
    pub up_stream: u8,
    pub buffer_size_db: u32,
    pub max_bitrate: u32,
    pub avg_bitrate: u32,

    pub dec_specific: DecoderSpecificDescriptor,
}

impl<R: Read + Seek> ReadDesc<&mut R> for DecoderConfigDescriptor {
    fn read_desc(reader: &mut R, size: u32) -> Result<Self> {
        let start = reader.stream_position()?;

        let object_type_indication = reader.read_u8()?;
        let byte_a = reader.read_u8()?;
        let stream_type = (byte_a & 0xFC) >> 2;
        let up_stream = byte_a & 0x02;
        let buffer_size_db = reader.read_u24::<BigEndian>()?;
        let max_bitrate = reader.read_u32::<BigEndian>()?;
        let avg_bitrate = reader.read_u32::<BigEndian>()?;

        let mut dec_specific = None;

        let mut current = reader.stream_position()?;
        let end = start + size as u64;
        while current < end {
            let (desc_tag, desc_size) = read_desc(reader)?;
            match desc_tag {
                0x05 => {
                    dec_specific = Some(DecoderSpecificDescriptor::read_desc(reader, desc_size)?);
                }
                _ => {
                    skip_bytes(reader, desc_size as u64)?;
                }
            }
            current = reader.stream_position()?;
        }

        Ok(Self {
            object_type_indication,
            stream_type,
            up_stream,
            buffer_size_db,
            max_bitrate,
            avg_bitrate,
            dec_specific: dec_specific.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct DecoderSpecificDescriptor {
    pub profile: u8,
    pub freq_index: u8,
    pub chan_conf: u8,
}

fn get_audio_object_type(byte_a: u8, byte_b: u8) -> u8 {
    let mut profile = byte_a >> 3;
    if profile == 31 {
        profile = 32 + ((byte_a & 7) | (byte_b >> 5));
    }

    profile
}

fn get_chan_conf<R: Read + Seek>(
    reader: &mut R,
    byte_b: u8,
    freq_index: u8,
    extended_profile: bool,
) -> Result<u8> {
    let chan_conf;
    if freq_index == 15 {
        // Skip the 24 bit sample rate
        let sample_rate = reader.read_u24::<BigEndian>()?;
        chan_conf = ((sample_rate >> 4) & 0x0F) as u8;
    } else if extended_profile {
        let byte_c = reader.read_u8()?;
        chan_conf = (byte_b & 1) | (byte_c & 0xE0);
    } else {
        chan_conf = (byte_b >> 3) & 0x0F;
    }

    Ok(chan_conf)
}

impl<R: Read + Seek> ReadDesc<&mut R> for DecoderSpecificDescriptor {
    fn read_desc(reader: &mut R, _size: u32) -> Result<Self> {
        let byte_a = reader.read_u8()?;
        let byte_b = reader.read_u8()?;
        let profile = get_audio_object_type(byte_a, byte_b);
        let (freq_index, chan_conf) = if profile > 31 {
            let freq_index = (byte_b >> 1) & 0x0F;
            let chan_conf = get_chan_conf(reader, byte_b, freq_index, true)?;
            (freq_index, chan_conf)
        } else {
            let freq_index = ((byte_a & 0x07) << 1) + (byte_b >> 7);
            let chan_conf = get_chan_conf(reader, byte_b, freq_index, false)?;
            (freq_index, chan_conf)
        };

        Ok(Self {
            profile,
            freq_index,
            chan_conf,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SLConfigDescriptor {}

impl<R: Read + Seek> ReadDesc<&mut R> for SLConfigDescriptor {
    fn read_desc(reader: &mut R, _size: u32) -> Result<Self> {
        reader.read_u8()?; // pre-defined

        Ok(Self {})
    }
}
