use std::path::Path;

fn assert_snapshot(snapshot_path: &Path, contents: &[u8]) {
    // if file doesn't exist, create it
    // otherwise, compare the contents and report failure if they don't match
    // if env var `UPDATE_SNAPSHOTS` is set, overwrite the file

    let new_snapshot_path = snapshot_path.with_file_name(format!(
        "{}.new",
        snapshot_path.file_name().unwrap().to_str().unwrap()
    ));

    if !snapshot_path.exists() || std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        if new_snapshot_path.exists() {
            std::fs::remove_file(new_snapshot_path).expect("failed to remove new snapshot file");
        }
        std::fs::write(snapshot_path, contents).expect("failed to write snapshot file");
        return;
    }

    let actual = std::fs::read(snapshot_path).expect("failed to read snapshot file");
    if actual != contents {
        // add `.new` to path
        std::fs::write(snapshot_path, contents).expect("failed to write new snapshot file");
        panic!("snapshot mismatch: {}", snapshot_path.display());
    }
}

fn get_track_description(track: &mp4::TrakBox) -> Vec<u8> {
    if let Some(ref av01) = track.mdia.minf.stbl.stsd.av01 {
        av01.av1c.raw.clone()
    } else if let Some(ref avc1) = track.mdia.minf.stbl.stsd.avc1 {
        avc1.avcc.raw.clone()
    } else if let Some(ref hvc1) = track.mdia.minf.stbl.stsd.hvc1 {
        hvc1.hvcc.raw.clone()
    } else if let Some(ref vp09) = track.mdia.minf.stbl.stsd.vp09 {
        vp09.vpcc.raw.clone()
    } else {
        Vec::new()
    }
}

const BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples");
fn assert_video_snapshot(file_path: &str) {
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
                &base_path.join(format!("{}.track_{id}.segments", file_path)),
                format!(r#"{:#?}"#, track.samples).as_bytes(),
            );
            assert_snapshot(
                &base_path.join(format!("{}.track_{id}.json", file_path)),
                format!(
                    r#"{{ "codec": {:?}, "width": {}, "height": {}, "num_samples": {}, "description": {:?} }}"#,
                    track.codec_string(&video).unwrap_or("unknown".to_string()),
                    track.width,
                    track.height,
                    track.samples.len(),
                    get_track_description(track.trak(&video)),
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
