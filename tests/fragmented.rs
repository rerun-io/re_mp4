mod paths;

/// Regression test: sample sync flags in a *fragmented* mp4 must be read from
/// the `trun`/`tfhd` sample flags with the correct polarity.
///
/// Bit 16 of the 32-bit sample flags is `sample_is_non_sync_sample`, so a sync
/// sample (keyframe) is one where that bit is *clear*. A previous version read
/// the bit without negating it, which inverted every flag: the leading keyframe
/// was reported as non-sync and every P/B-frame as sync.
#[test]
fn fragmented_mp4_reads_sync_flags_with_correct_polarity() {
    let path = std::path::Path::new(paths::SAMPLE_BASE_PATH)
        .join("bigbuckbunny")
        .join("fragmented_avc_bframes.mp4");
    let (mp4, _data) = re_mp4::Mp4::read_file(path).unwrap();

    // This fixture is fragmented (one `moof` per GOP, each starting on a keyframe).
    assert!(
        !mp4.moofs.is_empty(),
        "fixture should be a fragmented mp4 with moof boxes"
    );

    let track = mp4.tracks().get(&1).unwrap();
    assert_eq!(track.kind, Some(re_mp4::TrackKind::Video));
    assert_eq!(track.samples.len(), 30, "fixture has 30 frames");

    // A decodable stream must begin on a sync sample.
    assert!(
        track.samples[0].is_sync,
        "the first sample of a fragmented mp4 must be a sync sample"
    );

    // Only the GOP starts (one per `moof`) are sync samples — far fewer than the
    // total. With the inverted-flag bug this would instead be `total - moofs`.
    let sync_count = track.samples.iter().filter(|s| s.is_sync).count();
    assert_eq!(
        sync_count,
        mp4.moofs.len(),
        "exactly one sync sample per fragment"
    );
    assert!(
        sync_count < track.samples.len(),
        "not every sample can be a keyframe"
    );
}
