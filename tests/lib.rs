use std::path::Path;

fn read_sample_file(path: &str) {
    const BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples");
    let bytes = std::fs::read(Path::new(BASE).join(path)).unwrap();
    let mp4 = mp4::read(&bytes).unwrap();
    let samples = &mp4.tracks().get(&1).unwrap().data;
    std::fs::write(Path::new(BASE).join(format!("{}.bin", path)), samples).unwrap();
    panic!();
}

#[test]
fn bbb_video_av1_frag() {
    read_sample_file("bbb_video_av1_frag.mp4");
}
