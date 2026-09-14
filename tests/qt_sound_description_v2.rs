//! Regression for QuickTime Sound Description version 2 (`mp4a`).
//!
//! Full source movie (not checked in — too large):
//! https://download.blender.org/peach/bigbuckbunny_movies/big_buck_bunny_1080p_h264.mov.zip
//!
//! `MP4A_SOUND_DESC_V2` is the AAC `mp4a` sample-entry atom from that file
//! (`version == 2`, nested `wave`/`esds`).

use std::io::{Cursor, Seek, SeekFrom};

use re_mp4::{Mp4aBox, ReadBox};

/// `mp4a` atom from Blender Big Buck Bunny 1080p H.264 `.mov` (audio track).
const MP4A_SOUND_DESC_V2: &[u8] = &[
    0, 0, 0, 207, 109, 112, 52, 97, 0, 0, 0, 0, 0, 0, 0, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 16,
    255, 254, 0, 0, 0, 1, 0, 0, 0, 0, 0, 72, 64, 231, 112, 0, 0, 0, 0, 0, 0, 0, 0, 6, 127, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 91, 119, 97, 118, 101, 0, 0, 0, 12,
    102, 114, 109, 97, 109, 112, 52, 97, 0, 0, 0, 12, 109, 112, 52, 97, 0, 0, 0, 0, 0, 0, 0, 51,
    101, 115, 100, 115, 0, 0, 0, 0, 3, 128, 128, 128, 34, 0, 0, 0, 4, 128, 128, 128, 20, 64, 21, 0,
    24, 0, 0, 6, 214, 0, 0, 6, 214, 0, 5, 128, 128, 128, 2, 17, 176, 6, 128, 128, 128, 1, 2, 0, 0,
    0, 8, 0, 0, 0, 0, 0, 0, 0, 44, 99, 104, 97, 110, 0, 0, 0, 0, 0, 124, 0, 6, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 4, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[test]
fn parse_qt_sound_description_v2_mp4a_atom() {
    assert_eq!(&MP4A_SOUND_DESC_V2[4..8], b"mp4a");
    let size = u32::from_be_bytes(MP4A_SOUND_DESC_V2[0..4].try_into().unwrap()) as u64;
    assert_eq!(size as usize, MP4A_SOUND_DESC_V2.len());

    let mut cursor = Cursor::new(MP4A_SOUND_DESC_V2);
    cursor
        .seek(SeekFrom::Start(8))
        .expect("seek past box header");
    let mp4a = Mp4aBox::read_box(&mut cursor, size).expect("parse SoundDescriptionV2 mp4a");

    assert!(mp4a.esds.is_some(), "expected esds nested under QT wave");
    assert_eq!(mp4a.samplerate.value(), 48000);
    assert_eq!(mp4a.channelcount, 6);
}

/// Optional full-file check. Set `RE_MP4_BBB_MOV` to the unzipped Blender
/// `big_buck_bunny_1080p_h264.mov` path. Slow: the file is ~700 MB.
#[test]
fn parse_qt_sound_description_v2_full_movie_if_present() {
    let Ok(path) = std::env::var("RE_MP4_BBB_MOV") else {
        eprintln!("skipping full-movie test (set RE_MP4_BBB_MOV to enable)");
        return;
    };
    let path = std::path::Path::new(&path);
    assert!(
        path.is_file(),
        "RE_MP4_BBB_MOV is not a file: {}",
        path.display()
    );

    let (mp4, bytes) = re_mp4::Mp4::read_file(path).expect("parse full Big Buck Bunny MOV");
    let video = mp4
        .tracks()
        .values()
        .find(|t| t.kind == Some(re_mp4::TrackKind::Video))
        .expect("video track");
    let audio = mp4
        .tracks()
        .values()
        .find(|t| t.kind == Some(re_mp4::TrackKind::Audio))
        .expect("audio track");

    assert!(!video.samples.is_empty());
    assert!(!audio.samples.is_empty());
    let range = video.samples[0].byte_range();
    assert!(range.end <= bytes.len());

    let stsd = &audio.trak(&mp4).mdia.minf.stbl.stsd;
    match &stsd.contents {
        re_mp4::StsdBoxContent::Mp4a(mp4a) => {
            assert!(mp4a.esds.is_some());
            assert_eq!(mp4a.samplerate.value(), 48000);
            assert_eq!(mp4a.channelcount, 6);
        }
        other => panic!("expected Mp4a, got {other:?}"),
    }

    println!(
        "full MOV ok: video_samples={} audio_samples={} bytes={}",
        video.samples.len(),
        audio.samples.len(),
        bytes.len()
    );
}
