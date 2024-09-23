#![allow(dead_code)] // TODO(#3): enable tests again
#![allow(clippy::unwrap_used)]

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

fn get_track_description(track: &re_mp4::TrakBox) -> Vec<u8> {
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
    let video = re_mp4::read(&bytes).unwrap();

    #[allow(clippy::iter_over_hash_type)] // what we do in the iteration is not order-dependent
    for (id, track) in video.tracks() {
        if track.kind == re_mp4::TrackKind::Video {
            assert_snapshot(
                &base_path.join(format!("{file_path}.track_{id}.bin")),
                &track.data,
            );
            assert_snapshot(
                &base_path.join(format!("{file_path}.track_{id}.segments")),
                format!(r#"{:#?}"#, track.samples).as_bytes(),
            );
            assert_snapshot(
                &base_path.join(format!("{file_path}.track_{id}.json")),
                format!(
                    r#"{{ "codec": {:?}, "width": {}, "height": {}, "num_samples": {}, "description": {:?} }}"#,
                    track.codec_string(&video).unwrap_or("unknown".to_owned()),
                    track.width,
                    track.height,
                    track.samples.len(),
                    get_track_description(track.trak(&video).unwrap()),
                )
                .as_bytes(),
            );
        }
    }
}
