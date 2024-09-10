use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::SeekFrom;
use std::io::{Read, Seek};

use crate::*;

#[derive(Debug)]
pub struct Mp4 {
    pub ftyp: FtypBox,
    pub moov: MoovBox,
    pub moofs: Vec<MoofBox>,
    pub emsgs: Vec<EmsgBox>,
    tracks: HashMap<u64, Track>,
}

impl Mp4 {
    pub fn read<R: Read + Seek>(mut reader: R, size: u64) -> Result<Self> {
        let start = reader.stream_position()?;

        let mut ftyp = None;
        let mut moov = None;
        let mut moofs = Vec::new();
        let mut moof_offsets = Vec::new();
        let mut emsgs = Vec::new();

        let mut current = start;
        while current < size {
            // Get box header.
            let header = BoxHeader::read(&mut reader)?;
            let BoxHeader { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "file contains a box with a larger size than it",
                ));
            }

            // Break if size zero BoxHeader, which can result in dead-loop.
            if s == 0 {
                break;
            }

            // Match and parse the atom boxes.
            match name {
                BoxType::FtypBox => {
                    ftyp = Some(FtypBox::read_box(&mut reader, s)?);
                }
                BoxType::FreeBox => {
                    skip_box(&mut reader, s)?;
                }
                BoxType::MdatBox => {
                    skip_box(&mut reader, s)?;
                }
                BoxType::MoovBox => {
                    moov = Some(MoovBox::read_box(&mut reader, s)?);
                }
                BoxType::MoofBox => {
                    let moof_offset = reader.stream_position()? - 8;
                    let moof = MoofBox::read_box(&mut reader, s)?;
                    moofs.push(moof);
                    moof_offsets.push(moof_offset);
                }
                BoxType::EmsgBox => {
                    let emsg = EmsgBox::read_box(&mut reader, s)?;
                    emsgs.push(emsg);
                }
                _ => {
                    // XXX warn!()
                    skip_box(&mut reader, s)?;
                }
            }
            current = reader.stream_position()?;
        }

        if ftyp.is_none() {
            return Err(Error::BoxNotFound(BoxType::FtypBox));
        }
        if moov.is_none() {
            return Err(Error::BoxNotFound(BoxType::MoovBox));
        }

        let mut this = Mp4 {
            ftyp: ftyp.unwrap(),
            moov: moov.unwrap(),
            moofs,
            emsgs,
            tracks: HashMap::new(),
        };

        let mut tracks = this.build_tracks();
        this.update_sample_list(&mut tracks);
        this.tracks = tracks;
        this.load_track_data(&mut reader)?;

        Ok(this)
    }

    pub fn tracks(&self) -> &HashMap<u64, Track> {
        &self.tracks
    }

    fn build_tracks(&mut self) -> HashMap<u64, Track> {
        let mut tracks = HashMap::new();

        // load samples from traks
        for trak in &self.moov.traks {
            let mut sample_n = 0usize;
            let mut chunk_index = 1u64;
            let mut chunk_run_index = 0usize;
            let mut last_sample_in_chunk = 0u64;
            let mut offset_in_chunk = 0u64;
            let mut last_chunk_in_run = 0u64;
            let mut last_sample_in_stts_run = -1i64;
            let mut stts_run_index = -1i64;
            let mut last_stss_index = 0;

            let mut samples = Vec::<Sample>::new();

            fn get_sample_chunk_offset(stbl: &StblBox, chunk_index: u64) -> u64 {
                if let Some(stco) = &stbl.stco {
                    stco.entries[chunk_index as usize - 1] as u64
                } else if let Some(co64) = &stbl.co64 {
                    co64.entries[chunk_index as usize - 1]
                } else {
                    panic!()
                }
            }

            let stbl = &trak.mdia.minf.stbl;
            let stsc = &stbl.stsc;
            let stsz = &stbl.stsz;
            let stts = &stbl.stts;

            while sample_n < stsz.sample_sizes.len() {
                // compute offset
                if sample_n == 0 {
                    chunk_index = 1;
                    chunk_run_index = 0;
                    last_sample_in_chunk = stsc.entries[chunk_run_index].samples_per_chunk as u64;
                    offset_in_chunk = 0;

                    if chunk_run_index + 1 < stsc.entries.len() {
                        last_chunk_in_run =
                            stsc.entries[chunk_run_index + 1].first_chunk as u64 - 1;
                    } else {
                        last_chunk_in_run = u64::MAX;
                    }
                } else if sample_n < last_sample_in_chunk as usize {
                    /* ... */
                } else {
                    chunk_index += 1;
                    offset_in_chunk = 0;
                    if chunk_index > last_chunk_in_run {
                        chunk_run_index += 1;
                        if chunk_run_index + 1 < stsc.entries.len() {
                            last_chunk_in_run =
                                stsc.entries[chunk_run_index + 1].first_chunk as u64 - 1;
                        } else {
                            last_chunk_in_run = u64::MAX;
                        }
                    }

                    last_sample_in_chunk += stsc.entries[chunk_run_index].samples_per_chunk as u64;
                }

                // compute timestamp, duration, is_sync
                if sample_n as i64 > last_sample_in_stts_run {
                    stts_run_index += 1;
                    if last_sample_in_stts_run < 0 {
                        last_sample_in_stts_run = 0;
                    }
                    last_sample_in_stts_run +=
                        stts.entries[stts_run_index as usize].sample_count as i64;
                }

                let timescale = trak.mdia.mdhd.timescale as u64;
                let size = stsz.sample_sizes[sample_n] as u64;
                let offset = get_sample_chunk_offset(stbl, chunk_index) + offset_in_chunk;
                offset_in_chunk += size;

                let timestamp = if sample_n > 0 {
                    samples[sample_n - 1].duration =
                        stts.entries[stts_run_index as usize].sample_delta as u64;

                    samples[sample_n - 1].timestamp + samples[sample_n - 1].duration
                } else {
                    0
                };

                let is_sync = if let Some(stss) = &stbl.stss {
                    if last_stss_index < stss.entries.len()
                        && sample_n == stss.entries[last_stss_index] as usize - 1
                    {
                        last_stss_index += 1;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                };

                samples.push(Sample {
                    id: samples.len() as u32,
                    timescale,
                    size,
                    offset,
                    timestamp,
                    is_sync,
                    duration: 0, // filled once we know next sample timestamp
                });
                sample_n += 1;
            }

            if let Some(last_sample) = samples.last_mut() {
                last_sample.duration = trak.mdia.mdhd.duration - last_sample.timestamp;
            }

            tracks.insert(
                trak.tkhd.track_id as u64,
                Track {
                    track_id: trak.tkhd.track_id,
                    width: trak.tkhd.width.value(),
                    height: trak.tkhd.height.value(),
                    first_traf_merged: false,
                    timescale: trak.mdia.mdhd.timescale as u64,
                    duration: trak.mdia.mdhd.duration,
                    kind: trak.mdia.minf.stbl.stsd.kind(),
                    samples,
                    data: Vec::new(),
                },
            );
        }

        tracks
    }

    fn update_sample_list(&mut self, tracks: &mut HashMap<u64, Track>) {
        // if the input file is fragmented and fetched in multiple downloads, we need to update the list of samples
        for moof in &self.moofs {
            // process moof to update sample list
            for traf in &moof.trafs {
                let mut last_run_position = 0;

                let track = tracks.get_mut(&(traf.tfhd.track_id as u64)).unwrap();
                let trak = self
                    .moov
                    .traks
                    .iter()
                    .find(|trak| trak.tkhd.track_id == traf.tfhd.track_id)
                    .unwrap();
                let trex = self
                    .moov
                    .mvex
                    .as_ref()
                    .map(|mvex| {
                        mvex.trexs
                            .iter()
                            .find(|trex| trex.track_id == traf.tfhd.track_id)
                            .unwrap()
                            .clone()
                    })
                    .unwrap_or_default();

                let default_sample_duration = traf
                    .tfhd
                    .default_sample_duration
                    .unwrap_or(trex.default_sample_duration);
                let default_sample_size = traf
                    .tfhd
                    .default_sample_size
                    .unwrap_or(trex.default_sample_size);
                let default_sample_flags = traf
                    .tfhd
                    .default_sample_flags
                    .unwrap_or(trex.default_sample_flags);

                for (traf_idx, trun) in traf.truns.iter().enumerate() {
                    for sample_n in 0..trun.sample_count as usize {
                        let mut sample_flags = default_sample_flags;
                        if trun.flags & TrunBox::FLAG_SAMPLE_FLAGS != 0 {
                            sample_flags = trun
                                .sample_flags
                                .get(sample_n)
                                .copied()
                                .unwrap_or(sample_flags);
                        } else if sample_n == 0
                            && (trun.flags & TrunBox::FLAG_FIRST_SAMPLE_FLAGS != 0)
                        {
                            sample_flags = trun.first_sample_flags.unwrap_or(sample_flags);
                        }

                        let mut timestamp = 0;
                        if track.first_traf_merged || sample_n > 0 {
                            let prev = &track.samples[track.samples.len() - 1];
                            timestamp = prev.timestamp + prev.duration;
                        } else {
                            if let Some(tfdt) = &traf.tfdt {
                                timestamp = tfdt.base_media_decode_time;
                            }
                            track.first_traf_merged = true;
                        }
                        let duration = trun
                            .sample_durations
                            .get(sample_n)
                            .copied()
                            .unwrap_or(default_sample_duration)
                            as u64;

                        let bdop = traf.tfhd.flags & TfhdBox::FLAG_BASE_DATA_OFFSET != 0;
                        let dbim = traf.tfhd.flags & TfhdBox::FLAG_DEFAULT_BASE_IS_MOOF != 0;
                        let bdo = if !bdop {
                            if !dbim {
                                if sample_n == 0 {
                                    // the first sample in the track fragment
                                    moof.start // the position of the first byte of the enclosing Movie Fragment Box
                                } else {
                                    last_run_position // the offset of the previous sample
                                }
                            } else {
                                moof.start
                            }
                        } else {
                            traf.tfhd.base_data_offset.unwrap_or(moof.start)
                        };

                        let sample_size =
                            trun.sample_sizes
                                .get(sample_n)
                                .copied()
                                .unwrap_or(default_sample_size) as u64;

                        let sample_offset = if traf_idx == 0 && sample_n == 0 {
                            bdo + trun.data_offset.unwrap_or(0) as u64
                        } else {
                            last_run_position
                        };

                        last_run_position = sample_offset + sample_size;

                        track.samples.push(Sample {
                            id: track.samples.len() as u32,
                            is_sync: (sample_flags >> 16) & 0x1 != 0,
                            size: sample_size,
                            offset: sample_offset,
                            timescale: trak.mdia.mdhd.timescale as u64,
                            timestamp,
                            duration,
                        });
                    }
                }
            }
        }
    }

    fn load_track_data<R: Read + Seek>(&mut self, reader: &mut R) -> Result<()> {
        for track in self.tracks.values_mut() {
            let mut first_sample_offset = None;
            for sample in &mut track.samples {
                reader.seek(SeekFrom::Start(sample.offset))?;

                let start = track.data.len();
                track
                    .data
                    .extend(std::iter::repeat(0).take(sample.size as usize));
                reader.read_exact(&mut track.data[start..])?;

                let first_sample_offset = *first_sample_offset.get_or_insert(sample.offset);
                sample.offset -= first_sample_offset;
            }
        }

        Ok(())
    }
}

pub struct Track {
    first_traf_merged: bool,

    pub width: u16,
    pub height: u16,

    pub track_id: u32,
    timescale: u64,
    duration: u64,
    pub kind: TrackKind,
    pub samples: Vec<Sample>,
    pub data: Vec<u8>,
}

impl Track {
    pub fn duration_ms(&self) -> f64 {
        (self.duration as f64 * 1e3) / self.timescale as f64
    }

    pub fn trak<'a>(&self, mp4: &'a Mp4) -> &'a TrakBox {
        mp4.moov
            .traks
            .iter()
            .find(|trak| trak.tkhd.track_id == self.track_id)
            .unwrap()
    }

    pub fn read_sample(&self, sample_id: u32) -> &[u8] {
        let sample = &self.samples[sample_id as usize];
        &self.data[sample.offset as usize..(sample.offset + sample.size) as usize]
    }

    pub fn raw_codec_config(&self, mp4: &Mp4) -> Option<Vec<u8>> {
        let sample_description = &self.trak(mp4).mdia.minf.stbl.stsd;

        if let Some(Av01Box { av1c, .. }) = &sample_description.av01 {
            Some(av1c.raw.clone())
        } else if let Some(Avc1Box { avcc, .. }) = &sample_description.avc1 {
            Some(avcc.raw.clone())
        } else if let Some(Hvc1Box { hvcc, .. }) = &sample_description.hvc1 {
            Some(hvcc.raw.clone())
        } else if let Some(Vp08Box { vpcc, .. }) = &sample_description.vp08 {
            Some(vpcc.raw.clone())
        } else if let Some(Vp09Box { vpcc, .. }) = &sample_description.vp09 {
            Some(vpcc.raw.clone())
        } else {
            None
        }
    }

    pub fn codec_string(&self, mp4: &Mp4) -> Option<String> {
        let sample_description = &self.trak(mp4).mdia.minf.stbl.stsd;

        let s = if let Some(Av01Box { av1c, .. }) = &sample_description.av01 {
            let profile = av1c.profile;
            let level = av1c.level;
            let tier = if av1c.tier == 0 { "M" } else { "H" };
            let bit_depth = av1c.bit_depth;

            format!("av01.{profile}.{level:02}{tier}.{bit_depth:02}")
        } else if let Some(Avc1Box { avcc, .. }) = &sample_description.avc1 {
            let profile = avcc.avc_profile_indication;
            let constraint = avcc.profile_compatibility;
            let level = avcc.avc_level_indication;

            format!("avc1.{profile:02X}{constraint:02X}{level:02X}")
        } else if let Some(Hvc1Box { hvcc, .. }) = &sample_description.hvc1 {
            let mut codec = "hvc1".to_string();
            match hvcc.general_profile_space {
                0 => {}
                1 => codec.push_str(".A"),
                2 => codec.push_str(".B"),
                3 => codec.push_str(".C"),
                _ => {}
            }
            write!(&mut codec, ".{}", hvcc.general_profile_idc).ok();

            let mut val = hvcc.general_profile_compatibility_flags;
            let mut reversed = 0;
            for i in 0..32 {
                reversed |= val & 1;
                if i == 31 {
                    break;
                }
                reversed <<= 1;
                val >>= 1;
            }
            write!(&mut codec, ".{:X}", reversed).ok();

            if hvcc.general_tier_flag {
                codec.push_str(".H");
            } else {
                codec.push_str(".L");
            }
            write!(&mut codec, "{}", hvcc.general_level_idc).ok();

            let mut constraint = [0u8; 6];
            constraint.copy_from_slice(&hvcc.general_constraint_indicator_flag.to_be_bytes()[2..]);
            let mut has_byte = false;
            let mut i = 5isize;
            while i >= 0 {
                let v = constraint[i as usize];
                if v > 0 || has_byte {
                    write!(&mut codec, ".{v:00X}").ok();
                    has_byte = true;
                }
                i -= 1;
            }

            codec
        } else if let Some(Vp08Box { vpcc, .. }) = &sample_description.vp08 {
            let profile = vpcc.profile;
            let level = vpcc.level;
            let bit_depth = vpcc.bit_depth;

            format!("vp08.{profile:02}.{level:02}.{bit_depth:02}")
        } else if let Some(Vp09Box { vpcc, .. }) = &sample_description.vp09 {
            let profile = vpcc.profile;
            let level = vpcc.level;
            let bit_depth = vpcc.bit_depth;

            format!("vp09.{profile:02}.{level:02}.{bit_depth:02}")
        } else {
            return None;
        };

        Some(s)
    }
}

#[derive(Default, Clone, Copy)]
pub struct Sample {
    pub id: u32,
    pub is_sync: bool,
    pub size: u64,
    pub offset: u64,
    timescale: u64,
    timestamp: u64,
    duration: u64,
}

impl Sample {
    pub fn timestamp_ms(&self) -> f64 {
        (self.timestamp as f64 * 1e3) / self.timescale as f64
    }

    pub fn duration_ms(&self) -> f64 {
        (self.duration as f64 * 1e3) / self.timescale as f64
    }
}

impl Mp4 {
    pub fn metadata(&self) -> impl Metadata<'_> {
        self.moov.udta.as_ref().and_then(|udta| {
            udta.meta.as_ref().and_then(|meta| match meta {
                MetaBox::Mdir { ilst } => ilst.as_ref(),
                _ => None,
            })
        })
    }
}

impl std::fmt::Debug for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Track")
            .field("first_traf_merged", &self.first_traf_merged)
            .field("kind", &self.kind)
            .field("samples", &self.samples)
            .field("data.len", &self.data.len())
            .finish()
    }
}

impl std::fmt::Debug for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sample")
            .field("is_sync", &self.is_sync)
            .field("size", &self.size)
            .field("offset", &self.offset)
            .field("timestamp", &self.timestamp_ms())
            .field("duration", &self.duration_ms())
            .finish()
    }
}
