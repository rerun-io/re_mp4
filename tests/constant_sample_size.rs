use re_mp4;

mod paths;

fn test_codec_parsing(video_path: &str, expected_sample_length: u32) {
    let mp4_file = std::path::Path::new(paths::SAMPLE_BASE_PATH).join(video_path);
    let (video, _) = re_mp4::Mp4::read_file(mp4_file).expect("Failed parsing mp4");

    let track = video.tracks().get(&1);
    let track = track.expect("Expected a video track with id 1");
    assert!(
        track.samples.len() == expected_sample_length as usize,
        "Expected exactly {expected_sample_length} video sample(s) but got {}",
        track.samples.len()
    );
}

#[test]
fn check_episode_58() {
    test_codec_parsing("lerobot/episode_000058.mp4", 1);
}
