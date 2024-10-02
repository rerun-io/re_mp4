use re_mp4::{StsdBox, StsdBoxContent};

mod paths;

fn test_codec_parsing(
    video_path: &str,
    expected_codec_starts_with: &str,
    stsd_box_check: impl Fn(&StsdBox),
) {
    let mp4_file = std::path::Path::new(paths::SAMPLE_BASE_PATH).join(video_path);
    let video = re_mp4::Mp4::read_file(mp4_file).expect("Failed parsing mp4");

    let track = video.tracks().get(&1);
    let track = track.expect("Expected a video track with id 1");
    assert_eq!(
        track.kind,
        Some(re_mp4::TrackKind::Video),
        "Expected a video track but got {:?}",
        track.kind
    );

    let codec_string = track
        .codec_string(&video)
        .expect("Failed to read codec string");
    assert!(
        codec_string.starts_with(expected_codec_starts_with),
        "unexpected codec string: {codec_string}"
    );

    let stsd_box = &track.trak(&video).mdia.minf.stbl.stsd;
    stsd_box_check(stsd_box);
}

#[test]
fn parse_av1() {
    test_codec_parsing("bigbuckbunny/av1.mp4", "av01", |stsd_box| {
        assert!(matches!(stsd_box.contents, StsdBoxContent::Av01(_)));
    });
}

#[test]
fn parse_avc() {
    test_codec_parsing("bigbuckbunny/avc.mp4", "avc", |stsd_box| {
        assert!(matches!(stsd_box.contents, StsdBoxContent::Avc1(_)));
    });
}

#[test]
fn parse_hvc1() {
    test_codec_parsing("bigbuckbunny/hvc1.mp4", "hvc1", |stsd_box: &StsdBox| {
        assert!(matches!(stsd_box.contents, StsdBoxContent::Hvc1(_)));
    });
}

#[test]
fn parse_hev1() {
    test_codec_parsing("bigbuckbunny/hev1.mp4", "hev1", |stsd_box: &StsdBox| {
        assert!(matches!(stsd_box.contents, StsdBoxContent::Hev1(_)));
    });
}

#[test]
fn parse_vp8() {
    test_codec_parsing("bigbuckbunny/vp8.mp4", "vp08", |stsd_box: &StsdBox| {
        assert!(matches!(stsd_box.contents, StsdBoxContent::Vp08(_)));
    });
}

#[test]
fn parse_vp9() {
    test_codec_parsing("bigbuckbunny/vp9.mp4", "vp09", |stsd_box: &StsdBox| {
        assert!(matches!(stsd_box.contents, StsdBoxContent::Vp09(_)));
    });
}
