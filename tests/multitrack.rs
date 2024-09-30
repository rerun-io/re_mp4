mod paths;

#[test]
fn multitrack_video_with_subtitles_and_sound() {
    let mp4_with_subtitles_and_sound = std::path::Path::new(paths::SAMPLE_BASE_PATH)
        .join("rerun404_avc_with_subtitles_and_sound.mp4");

    let bytes = std::fs::read(mp4_with_subtitles_and_sound).unwrap();
    let video = re_mp4::read(&bytes).unwrap();

    assert_eq!(video.tracks().len(), 3);
    assert_eq!(video.moov.mvhd.next_track_id, 4);

    // Video track.
    {
        let track = video.tracks().get(&1).unwrap();
        assert_eq!(track.kind, Some(re_mp4::TrackKind::Video));
        assert_eq!(track.codec_string(&video), Some("avc1.640028".to_owned()));
        assert_eq!(track.track_id, 1);
        assert_eq!(track.width, 600);
        assert_eq!(track.height, 600);
        assert!(!track.samples.is_empty());
        assert!(!track.data.is_empty());
    }

    // Audio track.
    {
        let track = video.tracks().get(&2).unwrap();
        assert_eq!(track.kind, Some(re_mp4::TrackKind::Audio));
        assert_eq!(track.codec_string(&video), None);
        assert_eq!(track.track_id, 2);
        assert_eq!(track.width, 0);
        assert_eq!(track.height, 0);
        assert!(!track.samples.is_empty());
        assert!(!track.data.is_empty());
    }

    // Subtitle track.
    {
        let track = video.tracks().get(&3).unwrap();
        assert_eq!(track.kind, Some(re_mp4::TrackKind::Subtitle));
        assert_eq!(track.codec_string(&video), None);
        assert_eq!(track.track_id, 3);
        assert_eq!(track.width, 0);
        assert_eq!(track.height, 0);
        assert!(!track.samples.is_empty());
        assert!(!track.data.is_empty());
    }
}
