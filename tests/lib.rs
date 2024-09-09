use std::path::Path;

fn read_sample_file(path: &str) {
    const BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples");
    let bytes = std::fs::read(Path::new(BASE).join(path)).unwrap();
    mp4::read(&bytes).unwrap();
}

#[test]
fn bbb_video_av1_frag() {
    read_sample_file("bbb_video_av1_frag.mp4");
}

#[test]
fn sintel_trailer_720p_h264() {
    read_sample_file("sintel_trailer-720p-h264.mp4");
}
