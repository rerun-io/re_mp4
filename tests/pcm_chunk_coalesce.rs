//! PCM `in24` chunk-coalesce (issue #37 / mediabunny PCM rewrite).

mod paths;

use re_mp4::TrackKind;

#[test]
fn pcm_in24_is_audio_and_chunk_coalesced() {
    let path = std::path::Path::new(paths::SAMPLE_BASE_PATH).join("pcm/in24_stereo_2s.mov");
    let (mp4, _) = re_mp4::Mp4::read_file(&path).expect("parse pcm mov");

    let audio = mp4
        .tracks()
        .values()
        .find(|t| t.kind == Some(TrackKind::Audio))
        .expect("expected PCM audio track");

    // 2s @ 48kHz would be 96_000 per-frame samples without coalesce.
    assert!(
        audio.samples.len() < 5_000,
        "expected chunk-coalesced PCM, got {} samples",
        audio.samples.len()
    );
    assert!(
        !audio.samples.is_empty(),
        "expected at least one coalesced chunk sample"
    );

    assert_eq!(
        audio.codec_string(&mp4).as_deref(),
        Some("pcm-s24"),
        "in24 → pcm-s24 codec string"
    );

    // Contiguous chunks: each sample size should cover many PCM frames (≥1ms stereo s24 = 288 bytes).
    for sample in &audio.samples {
        assert!(
            sample.size >= 288,
            "coalesced chunk unexpectedly small: {}",
            sample.size
        );
        assert!(sample.is_sync);
    }

    let total_bytes: u64 = audio.samples.iter().map(|s| s.size).sum();
    // 2s * 48000 * 3 bytes * 2 ch = 576_000
    assert_eq!(
        total_bytes, 576_000,
        "sum of chunk sizes should match PCM payload"
    );
}

#[test]
fn pcm_coalesce_optional_long_plate() {
    // Optional stress path: testvideo 60s plate (~2.9M frames → few thousand chunks).
    let path = std::env::var_os("RE_MP4_PCM_LONG_MOV").map(std::path::PathBuf::from);
    let Some(path) = path else {
        eprintln!("skip: set RE_MP4_PCM_LONG_MOV to a long in24 .mov to stress-test");
        return;
    };
    if !path.is_file() {
        eprintln!("skip: RE_MP4_PCM_LONG_MOV not a file: {}", path.display());
        return;
    }

    let t0 = std::time::Instant::now();
    let (mp4, _) = re_mp4::Mp4::read_file(&path).expect("parse long pcm mov");
    let elapsed = t0.elapsed();

    let audio = mp4
        .tracks()
        .values()
        .find(|t| t.kind == Some(TrackKind::Audio))
        .expect("audio");

    assert!(
        audio.samples.len() < 50_000,
        "long plate still expanded too far: {} samples in {elapsed:?}",
        audio.samples.len()
    );
    eprintln!(
        "long PCM plate: {} audio samples in {elapsed:?}",
        audio.samples.len()
    );
}
