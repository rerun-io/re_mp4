use std::path::Path;

fn assert_snapshot(path: &Path, contents: &[u8]) {
    // if file doesn't exist, create it
    // otherwise, compare the contents and report failure if they don't match
    // if env var `UPDATE_SNAPSHOTS` is set, overwrite the file

    if !path.exists() || std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        std::fs::write(path, contents).expect("failed to write snapshot file");
    }

    let actual = std::fs::read(path).expect("failed to read snapshot file");
    if actual != contents {
        panic!("snapshot mismatch: {}", path.display());
    }
}

fn assert_video_snapshot(file_path: &str) {
    const BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples");
    let base_path = Path::new(BASE);
    let bytes = std::fs::read(base_path.join(file_path)).unwrap();
    let video = mp4::read(&bytes).unwrap();
    for (id, track) in video.tracks() {
        if track.kind == mp4::TrackKind::Video {
            assert_snapshot(
                &base_path.join(format!("{}.track_{id}.bin", file_path)),
                &track.data,
            );
            assert_snapshot(
                &base_path.join(format!("{}.track_{id}.json", file_path)),
                format!(
                    r#"{{ "width": {}, "height": {}, "num_samples": {} }}"#,
                    track.width,
                    track.height,
                    track.samples.len(),
                )
                .as_bytes(),
            );
        }
    }
}

#[test]
fn bbb_video_av1_frag() {
    assert_video_snapshot("bbb_video_av1_frag.mp4");
}

#[test]
fn bbb_video_avc_frag() {
    assert_video_snapshot("bbb_video_avc_frag.mp4");
}

#[test]
fn bbb_video_hevc_frag() {
    assert_video_snapshot("bbb_video_hevc_frag.mp4");
}

#[test]
fn bbb_video_vp9_frag() {
    assert_video_snapshot("bbb_video_vp9_frag.mp4");
}

#[test]
fn bbb_video_vp8_frag() {
    assert_video_snapshot("bbb_video_vp8_frag.mp4");
}

#[test]
fn sintel_trailer_720p_h264() {
    assert_video_snapshot("sintel_trailer-720p-h264.mp4");
}
