#![allow(clippy::unwrap_used)]

mod paths;

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

fn compare_video_snapshot_with_mp4box_output(video_path: &Path) {
    let video_path_str = video_path.to_str().unwrap();
    let base_path = video_path.parent().unwrap();

    // Run mp4 box to parse a video file and dump the result to files.
    assert!(
        std::process::Command::new("node")
            .arg(
                Path::new(paths::TEST_BASE_PATH)
                    .join("mp4box_parse.mjs")
                    .to_str()
                    .unwrap()
            )
            .arg(video_path_str)
            .status()
            .unwrap()
            .success(),
        "Failed to run mp4box."
    );

    let video = re_mp4::Mp4::read_file(base_path.join(video_path)).unwrap();

    for (id, track) in video.tracks() {
        if track.kind == Some(re_mp4::TrackKind::Video) {
            assert_snapshot(
                &base_path.join(format!("{video_path_str}.track_{id}.bin")),
                &track.data,
            );
            assert_snapshot(
                &base_path.join(format!("{video_path_str}.track_{id}.segments")),
                format!(r#"{:#?}"#, track.samples).as_bytes(),
            );
            assert_snapshot(
                &base_path.join(format!("{video_path_str}.track_{id}.json")),
                format!(
                    r#"{{ "codec": {:?}, "width": {}, "height": {}, "num_samples": {}, "description": {:?} }}"#,
                    track.codec_string(&video).unwrap_or("unknown".to_owned()),
                    track.width,
                    track.height,
                    track.samples.len(),
                    track.raw_codec_config(&video),
                )
                .as_bytes(),
            );
        }
    }
}

#[test]
fn compare_video_snapshot_with_mp4box_output_bigbuckbunny() {
    // List all mp4 files in the bigbuckbunny directory.
    let base_path = Path::new(paths::SAMPLE_BASE_PATH);
    let bigbuckbunny_path = base_path.join("bigbuckbunny");

    for entry in std::fs::read_dir(bigbuckbunny_path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |e| e == "mp4") {
            println!("-- Comparing {path:?}");
            compare_video_snapshot_with_mp4box_output(&path);
        }
    }
}
