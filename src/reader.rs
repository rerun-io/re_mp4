use std::collections::BTreeMap;
use std::io::SeekFrom;
use std::io::{Read, Seek};

use crate::{
    skip_box, BoxHeader, BoxType, EmsgBox, Error, FtypBox, MoofBox, MoovBox, ReadBox, Result,
    StblBox, StsdBoxContent, TfhdBox, TrackId, TrackKind, TrakBox, TrunBox,
};

#[derive(Debug)]
pub struct Mp4 {
    pub ftyp: FtypBox,
    pub moov: MoovBox,
    pub moofs: Vec<MoofBox>,
    pub emsgs: Vec<EmsgBox>,
    tracks: BTreeMap<TrackId, Track>,
}

impl Mp4 {
    /// Parses the contents of a byte slice as MP4 data.
    pub fn read_bytes(bytes: &[u8]) -> Result<Self> {
        let mp4 = Self::read(std::io::Cursor::new(bytes), bytes.len() as u64)?;
        Ok(mp4)
    }

    /// Reads the contents of a file as MP4 data.
    pub fn read_file(file_path: impl AsRef<std::path::Path>) -> Result<Self> {
        let bytes = std::fs::read(file_path)?;
        Self::read_bytes(&bytes)
    }

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

        let Some(ftyp) = ftyp else {
            return Err(Error::BoxNotFound(BoxType::FtypBox));
        };
        let Some(moov) = moov else {
            return Err(Error::BoxNotFound(BoxType::MoovBox));
        };

        let mut this = Self {
            ftyp,
            moov,
            moofs,
            emsgs,
            tracks: Default::default(),
        };

        let mut tracks = this.build_tracks();
        this.update_sample_list(&mut tracks)?;
        this.tracks = tracks;
        this.load_track_data(&mut reader)?;

        Ok(this)
    }

    pub fn tracks(&self) -> &BTreeMap<TrackId, Track> {
        &self.tracks
    }

    /// Process each `trak` box to obtain a list of samples for each track.
    ///
    /// Note that the list will be incomplete if the file is fragmented.
    fn build_tracks(&mut self) -> BTreeMap<TrackId, Track> {
        let mut tracks = BTreeMap::new();

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
            let mut last_sample_in_ctts_run = -1i64;
            let mut ctts_run_index = -1i64;

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

                let decode_timestamp = if sample_n > 0 {
                    samples[sample_n - 1].duration =
                        stts.entries[stts_run_index as usize].sample_delta as u64;

                    samples[sample_n - 1].decode_timestamp + samples[sample_n - 1].duration
                } else {
                    0
                };

                let composition_timestamp = if let Some(ctts) = &stbl.ctts {
                    if sample_n as i64 >= last_sample_in_ctts_run {
                        ctts_run_index += 1;
                        if last_sample_in_ctts_run < 0 {
                            last_sample_in_ctts_run = 0;
                        }
                        last_sample_in_ctts_run +=
                            ctts.entries[ctts_run_index as usize].sample_count as i64;
                    }

                    decode_timestamp + ctts.entries[ctts_run_index as usize].sample_offset as u64
                } else {
                    decode_timestamp
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
                    decode_timestamp,
                    composition_timestamp,
                    is_sync,
                    duration: 0, // filled once we know next sample timestamp
                });
                sample_n += 1;
            }

            if let Some(last_sample) = samples.last_mut() {
                last_sample.duration = trak.mdia.mdhd.duration - last_sample.decode_timestamp;
            }

            tracks.insert(
                trak.tkhd.track_id,
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

    /// In case the input file is fragmented, it will contain one or more `moof` boxes,
    /// which must be processed to obtain the full list of samples for each track.
    fn update_sample_list(&mut self, tracks: &mut BTreeMap<TrackId, Track>) -> Result<()> {
        let mut last_run_position = 0;

        for moof in &self.moofs {
            // process moof to update sample list
            for traf in &moof.trafs {
                let track_id = traf.tfhd.track_id;
                let track = tracks
                    .get_mut(&track_id)
                    .ok_or(Error::TrakNotFound(track_id))?;
                let trak = self
                    .moov
                    .traks
                    .iter()
                    .find(|trak| trak.tkhd.track_id == track_id)
                    .ok_or(Error::TrakNotFound(track_id))?;
                let trex = if let Some(mvex) = &self.moov.mvex {
                    mvex.trexs
                        .iter()
                        .find(|trex| trex.track_id == track_id)
                        .ok_or(Error::BoxInTrafNotFound(track_id, BoxType::TrexBox))?
                        .clone()
                } else {
                    Default::default()
                };

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

                        let mut decode_timestamp = 0;
                        if track.first_traf_merged || sample_n > 0 {
                            let prev = &track.samples[track.samples.len() - 1];
                            decode_timestamp = prev.decode_timestamp + prev.duration;
                        } else {
                            if let Some(tfdt) = &traf.tfdt {
                                decode_timestamp = tfdt.base_media_decode_time;
                            }
                            track.first_traf_merged = true;
                        }

                        let composition_timestamp = if trun.flags & TrunBox::FLAG_SAMPLE_CTS != 0 {
                            decode_timestamp
                                + trun.sample_cts.get(sample_n).copied().unwrap_or(0) as u64
                        } else {
                            decode_timestamp
                        };

                        let duration = trun
                            .sample_durations
                            .get(sample_n)
                            .copied()
                            .unwrap_or(default_sample_duration)
                            as u64;

                        let base_data_offset_present =
                            traf.tfhd.flags & TfhdBox::FLAG_BASE_DATA_OFFSET != 0;
                        let default_base_is_moof =
                            traf.tfhd.flags & TfhdBox::FLAG_DEFAULT_BASE_IS_MOOF != 0;
                        let data_offset_present = trun.flags & TrunBox::FLAG_DATA_OFFSET != 0;
                        let base_data_offset = if !base_data_offset_present {
                            if !default_base_is_moof {
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
                            if data_offset_present {
                                base_data_offset + trun.data_offset.unwrap_or(0) as u64
                            } else {
                                base_data_offset
                            }
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
                            decode_timestamp,
                            composition_timestamp,
                            duration,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// For every track, combine its samples into a single contiguous buffer.
    ///
    /// This also updates sample offsets and the track duration if needed.
    ///
    /// After this function is called, each track's [`Track::data`] may only be indexed by one of its samples' [`Sample::offset`]s.
    fn load_track_data<R: Read + Seek>(&mut self, reader: &mut R) -> Result<()> {
        for track in self.tracks.values_mut() {
            for sample in &mut track.samples {
                let data_offset = track.data.len();

                track
                    .data
                    .resize(track.data.len() + sample.size as usize, 0);

                // at this point, `sample.offset` is the offset of the first byte of the sample in the file
                reader.seek(SeekFrom::Start(sample.offset))?;
                reader
                    .read_exact(&mut track.data[data_offset..data_offset + sample.size as usize])?;

                // we want it to be the offset of the sample in the combined track data
                sample.offset = data_offset as u64;
            }

            if track.duration == 0 {
                track.duration = track
                    .samples
                    .last()
                    .map(|v| v.decode_timestamp + v.duration)
                    .unwrap_or_default();
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
    pub timescale: u64,
    pub duration: u64,
    pub kind: Option<TrackKind>,
    pub samples: Vec<Sample>,
    pub data: Vec<u8>,
}

impl Track {
    pub fn trak<'a>(&self, mp4: &'a Mp4) -> &'a TrakBox {
        let Some(trak) = mp4
            .moov
            .traks
            .iter()
            .find(|trak| trak.tkhd.track_id == self.track_id)
        else {
            // `Track` structs are only constructed when we have `trak` boxes,
            // so unless the user removes the `trak` box from the `Mp4`, it
            // will always be present.
            unreachable!("track with id \"{}\" not found", self.track_id);
        };

        trak
    }

    pub fn read_sample(&self, sample_id: u32) -> &[u8] {
        let sample = &self.samples[sample_id as usize];
        &self.data[sample.offset as usize..(sample.offset + sample.size) as usize]
    }

    pub fn raw_codec_config(&self, mp4: &Mp4) -> Option<Vec<u8>> {
        let sample_description = &self.trak(mp4).mdia.minf.stbl.stsd;

        match &sample_description.contents {
            StsdBoxContent::Av01(content) => Some(content.av1c.raw.clone()),
            StsdBoxContent::Avc1(content) => Some(content.avcc.raw.clone()),
            StsdBoxContent::Hev1(content) | StsdBoxContent::Hvc1(content) => {
                Some(content.hvcc.raw.clone())
            }
            StsdBoxContent::Vp08(content) => Some(content.vpcc.raw.clone()),
            StsdBoxContent::Vp09(content) => Some(content.vpcc.raw.clone()),
            StsdBoxContent::Mp4a(_) | StsdBoxContent::Tx3g(_) | StsdBoxContent::Unknown(_) => None,
        }
    }

    pub fn codec_string(&self, mp4: &Mp4) -> Option<String> {
        self.trak(mp4).mdia.minf.stbl.stsd.contents.codec_string()
    }
}

#[derive(Default, Clone, Copy)]
pub struct Sample {
    pub id: u32,
    pub is_sync: bool,
    pub size: u64,
    pub offset: u64,
    pub timescale: u64,
    pub decode_timestamp: u64,
    pub composition_timestamp: u64,
    pub duration: u64,
}

impl std::fmt::Debug for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Track")
            .field("first_traf_merged", &self.first_traf_merged)
            .field("kind", &self.kind)
            .field("timescale", &self.timescale)
            .field("duration", &self.duration)
            .finish()
    }
}

impl std::fmt::Debug for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sample")
            .field("is_sync", &self.is_sync)
            .field("size", &self.size)
            .field("offset", &self.offset)
            .field("decode_timestamp", &self.decode_timestamp)
            .field("composition_timestamp", &self.composition_timestamp)
            .field("duration", &self.duration)
            .finish()
    }
}
