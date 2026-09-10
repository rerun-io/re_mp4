use byteorder::{BigEndian, ReadBytesExt as _};
use serde::Serialize;
use std::io::{Read, Seek};

use crate::mp4box::{
    box_start, read_box_header_ext, skip_bytes, skip_bytes_to, value_u32, AacConfig, BoxHeader,
    BoxType, Error, FixedPointU16, Mp4Box, ReadBox, Result, HEADER_EXT_SIZE, HEADER_SIZE,
};

/// Size of a QTFF `SoundDescriptionV2` struct, including the box header,
/// up to and including `numAudioChannels`.
const SOUND_DESCRIPTION_V2_MIN_SIZE: u64 = HEADER_SIZE + 28 + 4 + 8 + 4;

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
    pub fn new(config: &AacConfig) -> Self {
        Self {
            data_reference_index: 1,
            channelcount: config.chan_conf as u16,
            samplesize: 16,
            samplerate: FixedPointU16::new(config.freq_index.freq() as u16),
            esds: Some(EsdsBox::new(config)),
        }
    }

    pub fn get_type() -> BoxType {
        BoxType::Mp4aBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE + 8 + 20;
        if let Some(ref esds) = self.esds {
            size += esds.box_size();
        }
        size
    }
}

impl Mp4Box for Mp4aBox {
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
        reader.read_u16::<BigEndian>()?; // reserved
        reader.read_u32::<BigEndian>()?; // reserved
        let mut channelcount = reader.read_u16::<BigEndian>()?;
        let samplesize = reader.read_u16::<BigEndian>()?;
        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        let mut samplerate = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);

        let end = start + size;

        match version {
            1 => {
                // QTFF SoundDescriptionV1: skip samplesPerPacket, bytesPerPacket,
                // bytesPerFrame, bytesPerSample.
                reader.read_u64::<BigEndian>()?;
                reader.read_u64::<BigEndian>()?;
            }
            2 => {
                // QTFF SoundDescriptionV2. The v0 fields above hold placeholder values
                // (3 channels, 16 bit, 65536.0 Hz), and the real values follow.
                // Child atoms start `size_of_struct_only` bytes after the atom start.
                let size_of_struct_only = reader.read_u32::<BigEndian>()? as u64;
                let audio_sample_rate = reader.read_f64::<BigEndian>()?;
                let num_audio_channels = reader.read_u32::<BigEndian>()?;
                // Remaining v2 fields (always7F000000, constBitsPerChannel, formatSpecificFlags,
                // constBytesPerAudioPacket, constLPCMFramesPerAudioPacket) are skipped below.

                if size_of_struct_only < SOUND_DESCRIPTION_V2_MIN_SIZE
                    || end < start + size_of_struct_only
                {
                    return Err(Error::InvalidData(
                        "mp4a version 2 sample entry has an invalid sizeOfStructOnly",
                    ));
                }

                channelcount = u16::try_from(num_audio_channels).unwrap_or(u16::MAX);
                if audio_sample_rate.is_finite() && 0.0 <= audio_sample_rate {
                    // Saturates on overflow, i.e. for sample rates over 65535 Hz
                    samplerate = FixedPointU16::new_raw((audio_sample_rate * 65536.0) as u32);
                }

                skip_bytes_to(reader, start + size_of_struct_only)?;
            }
            _ => {}
        }

        // Find esds in mp4a or wave
        let mut esds = None;
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
            if s < HEADER_SIZE {
                // Malformed (or a zero-size "extends to end" box), which we cannot make
                // progress on. Bail out and use whatever we have found so far.
                break;
            }
            if name == BoxType::EsdsBox {
                esds = Some(EsdsBox::read_box(reader, s)?);
                break;
            } else if name == BoxType::WaveBox {
                // Typically contains frma, mp4a, esds, and a terminator atom.
                // We don't skip it, so the next loop iterations will walk into its children.
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

impl EsdsBox {
    pub fn new(config: &AacConfig) -> Self {
        Self {
            version: 0,
            flags: 0,
            es_desc: ESDescriptor::new(config),
        }
    }
}

impl Mp4Box for EsdsBox {
    fn box_type(&self) -> BoxType {
        BoxType::EsdsBox
    }

    fn box_size(&self) -> u64 {
        HEADER_SIZE
            + HEADER_EXT_SIZE
            + 1
            + size_of_length(ESDescriptor::desc_size()) as u64
            + ESDescriptor::desc_size() as u64
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

#[expect(dead_code)]
trait Descriptor: Sized {
    fn desc_tag() -> u8;
    fn desc_size() -> u32;
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

fn size_of_length(size: u32) -> u32 {
    match size {
        0x0..=0x7F => 1,
        0x80..=0x3FFF => 2,
        0x4000..=0x1FFFFF => 3,
        _ => 4,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ESDescriptor {
    pub es_id: u16,

    pub dec_config: DecoderConfigDescriptor,
    pub sl_config: SLConfigDescriptor,
}

impl ESDescriptor {
    pub fn new(config: &AacConfig) -> Self {
        Self {
            es_id: 1,
            dec_config: DecoderConfigDescriptor::new(config),
            sl_config: SLConfigDescriptor::new(),
        }
    }
}

impl Descriptor for ESDescriptor {
    fn desc_tag() -> u8 {
        0x03
    }

    fn desc_size() -> u32 {
        3 + 1
            + size_of_length(DecoderConfigDescriptor::desc_size())
            + DecoderConfigDescriptor::desc_size()
            + 1
            + size_of_length(SLConfigDescriptor::desc_size())
            + SLConfigDescriptor::desc_size()
    }
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

impl DecoderConfigDescriptor {
    pub fn new(config: &AacConfig) -> Self {
        Self {
            object_type_indication: 0x40, // XXX AAC
            stream_type: 0x05,            // XXX Audio
            up_stream: 0,
            buffer_size_db: 0,
            max_bitrate: config.bitrate, // XXX
            avg_bitrate: config.bitrate,
            dec_specific: DecoderSpecificDescriptor::new(config),
        }
    }
}

impl Descriptor for DecoderConfigDescriptor {
    fn desc_tag() -> u8 {
        0x04
    }

    fn desc_size() -> u32 {
        13 + 1
            + size_of_length(DecoderSpecificDescriptor::desc_size())
            + DecoderSpecificDescriptor::desc_size()
    }
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

impl DecoderSpecificDescriptor {
    pub fn new(config: &AacConfig) -> Self {
        Self {
            profile: config.profile as u8,
            freq_index: config.freq_index as u8,
            chan_conf: config.chan_conf as u8,
        }
    }
}

impl Descriptor for DecoderSpecificDescriptor {
    fn desc_tag() -> u8 {
        0x05
    }

    fn desc_size() -> u32 {
        2
    }
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

impl SLConfigDescriptor {
    pub fn new() -> Self {
        Self {}
    }
}

impl Descriptor for SLConfigDescriptor {
    fn desc_tag() -> u8 {
        0x06
    }

    fn desc_size() -> u32 {
        1
    }
}

impl<R: Read + Seek> ReadDesc<&mut R> for SLConfigDescriptor {
    fn read_desc(reader: &mut R, _size: u32) -> Result<Self> {
        reader.read_u8()?; // pre-defined

        Ok(Self {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn boxed(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(name);
        out.extend_from_slice(payload);
        out
    }

    /// AAC-LC, 48 kHz, stereo.
    fn esds() -> Vec<u8> {
        let mut payload = vec![0, 0, 0, 0]; // version + flags
        payload.extend_from_slice(&[0x03, 0x19, 0x00, 0x01, 0x00]); // ES_Descriptor
        payload.extend_from_slice(&[0x04, 0x11, 0x40, 0x15]); // DecoderConfigDescriptor
        payload.extend_from_slice(&[0, 0, 0]); // buffer_size_db
        payload.extend_from_slice(&[0, 0, 0, 0]); // max_bitrate
        payload.extend_from_slice(&[0, 0, 0, 0]); // avg_bitrate
        payload.extend_from_slice(&[0x05, 0x02, 0x11, 0x90]); // DecoderSpecificDescriptor
        payload.extend_from_slice(&[0x06, 0x01, 0x02]); // SLConfigDescriptor
        boxed(b"esds", &payload)
    }

    /// The `QuickTime` `wave` atom: `frma`, `mp4a`, `esds`, and a terminator atom.
    fn wave() -> Vec<u8> {
        let mut payload = boxed(b"frma", b"mp4a");
        payload.extend(boxed(b"mp4a", &[0, 0, 0, 0]));
        payload.extend(esds());
        payload.extend_from_slice(&[0, 0, 0, 8, 0, 0, 0, 0]);
        boxed(b"wave", &payload)
    }

    /// The fields shared by all versions of the QTFF sound description.
    fn v0_fields(version: u16, channels: u16, samplerate: u32) -> Vec<u8> {
        let mut out = vec![0, 0, 0, 0, 0, 0]; // reserved
        out.extend_from_slice(&1u16.to_be_bytes()); // data_reference_index
        out.extend_from_slice(&version.to_be_bytes());
        out.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // revision, vendor
        out.extend_from_slice(&channels.to_be_bytes());
        out.extend_from_slice(&16u16.to_be_bytes()); // samplesize
        out.extend_from_slice(&[0, 0, 0, 0]); // compression_id, packet_size
        out.extend_from_slice(&samplerate.to_be_bytes());
        out
    }

    fn parse(mp4a: &[u8]) -> Result<Mp4aBox> {
        let mut reader = Cursor::new(mp4a);
        let header = BoxHeader::read(&mut reader)?;
        Mp4aBox::read_box(&mut reader, header.size)
    }

    fn assert_aac_lc_48k_stereo(mp4a: &Mp4aBox) {
        assert_eq!(mp4a.data_reference_index, 1);
        assert_eq!(mp4a.channelcount, 2);
        assert_eq!(mp4a.samplesize, 16);
        assert_eq!(mp4a.samplerate.value(), 48000);
        let esds = mp4a.esds.as_ref().expect("esds not found");
        assert_eq!(esds.es_desc.dec_config.object_type_indication, 0x40);
        assert_eq!(esds.es_desc.dec_config.dec_specific.profile, 2);
        assert_eq!(esds.es_desc.dec_config.dec_specific.freq_index, 3);
        assert_eq!(esds.es_desc.dec_config.dec_specific.chan_conf, 2);
    }

    #[test]
    fn version_0() {
        let mut payload = v0_fields(0, 2, 48000 << 16);
        payload.extend(esds());
        let mp4a = parse(&boxed(b"mp4a", &payload)).expect("parse failed");
        assert_aac_lc_48k_stereo(&mp4a);
    }

    #[test]
    fn version_1_with_wave() {
        let mut payload = v0_fields(1, 2, 48000 << 16);
        payload.extend_from_slice(&[0; 16]); // samples_per_packet, bytes_per_packet, bytes_per_frame, bytes_per_sample
        payload.extend(wave());
        let mp4a = parse(&boxed(b"mp4a", &payload)).expect("parse failed");
        assert_aac_lc_48k_stereo(&mp4a);
    }

    #[test]
    fn version_2_with_wave() {
        let mut payload = v0_fields(2, 3, 0x0001_0000);
        payload.extend_from_slice(&72u32.to_be_bytes()); // size_of_struct_only
        payload.extend_from_slice(&48000.0f64.to_be_bytes()); // audio_sample_rate
        payload.extend_from_slice(&2u32.to_be_bytes()); // num_audio_channels
        payload.extend_from_slice(&0x7F00_0000u32.to_be_bytes()); // always_7f000000
        payload.extend_from_slice(&0u32.to_be_bytes()); // const_bits_per_channel
        payload.extend_from_slice(&0u32.to_be_bytes()); // format_specific_flags
        payload.extend_from_slice(&0u32.to_be_bytes()); // const_bytes_per_audio_packet
        payload.extend_from_slice(&1024u32.to_be_bytes()); // const_lpcm_frames_per_audio_packet
        assert_eq!(payload.len() + 8, 72);
        payload.extend(wave());
        let mp4a = parse(&boxed(b"mp4a", &payload)).expect("parse failed");
        assert_aac_lc_48k_stereo(&mp4a);
    }

    #[test]
    fn version_2_with_too_small_size_of_struct_only() {
        let mut payload = v0_fields(2, 3, 0x0001_0000);
        payload.extend_from_slice(&8u32.to_be_bytes()); // size_of_struct_only
        payload.extend_from_slice(&48000.0f64.to_be_bytes());
        payload.extend_from_slice(&2u32.to_be_bytes());
        payload.extend_from_slice(&[0; 20]);
        payload.extend(esds());
        let err = parse(&boxed(b"mp4a", &payload));
        assert!(matches!(err, Err(Error::InvalidData(_))));
    }

    #[test]
    fn zero_size_child_does_not_hang() {
        let mut payload = v0_fields(0, 2, 48000 << 16);
        payload.extend_from_slice(&[0, 0, 0, 0, b'f', b'r', b'e', b'e']);
        payload.extend_from_slice(&[0; 8]);
        let mp4a = parse(&boxed(b"mp4a", &payload)).expect("parse failed");
        assert!(mp4a.esds.is_none());
    }
}
