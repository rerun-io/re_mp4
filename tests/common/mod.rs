pub fn get_sample_data(mp4_data: &[u8], track: &re_mp4::Track) -> Vec<u8> {
    let mut sample_data = Vec::new();
    for sample in &track.samples {
        sample_data.extend_from_slice(&mp4_data[sample.range()]);
    }
    sample_data
}
