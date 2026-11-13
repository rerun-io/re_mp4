#![expect(clippy::unwrap_used)]

mod paths;

fn sample_bytes(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(paths::SAMPLE_BASE_PATH)
        .join("bigbuckbunny")
        .join(name);
    std::fs::read(path).unwrap()
}

/// Offset and total length of the `nth` top-level box of the given type.
fn find_box(bytes: &[u8], box_type: &[u8; 4], nth: usize) -> (usize, usize) {
    let mut pos = 0;
    let mut seen = 0;
    while pos + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        assert!(8 <= len, "fixture must use 32-bit box sizes");
        if &bytes[pos + 4..pos + 8] == box_type {
            if seen == nth {
                return (pos, len);
            }
            seen += 1;
        }
        pos += len;
    }
    panic!("box not found");
}

fn num_samples(mp4: &re_mp4::Mp4) -> usize {
    mp4.tracks().get(&1).unwrap().samples.len()
}

#[test]
fn complete_file_is_not_truncated() {
    for name in ["hev1.mp4", "avc.mp4", "fragmented_avc_bframes.mp4"] {
        let mp4 = re_mp4::Mp4::read_bytes(&sample_bytes(name)).unwrap();
        assert!(!mp4.is_truncated, "{name}");
    }
}

/// `moov` precedes `mdat`: the sample tables survive a cut in `mdat`.
#[test]
fn cut_in_mdat_keeps_the_sample_tables() {
    let bytes = sample_bytes("hev1.mp4");
    let full = re_mp4::Mp4::read_bytes(&bytes).unwrap();
    let (mdat_start, mdat_len) = find_box(&bytes, b"mdat", 0);

    for cut in [
        mdat_start + 1,
        mdat_start + 8,
        mdat_start + mdat_len / 2,
        bytes.len() - 1,
    ] {
        let mp4 = re_mp4::Mp4::read_bytes(&bytes[..cut]).unwrap();
        assert!(mp4.is_truncated, "cut at {cut}");
        assert_eq!(num_samples(&mp4), num_samples(&full), "cut at {cut}");
    }
}

/// A fragmented file keeps every fragment whose `moof` is complete.
#[test]
fn cut_in_moof_keeps_the_earlier_fragments() {
    let bytes = sample_bytes("fragmented_avc_bframes.mp4");
    let full = re_mp4::Mp4::read_bytes(&bytes).unwrap();
    let (moof_start, moof_len) = find_box(&bytes, b"moof", 2);

    let before_moof = re_mp4::Mp4::read_bytes(&bytes[..moof_start]).unwrap();
    assert!(!before_moof.is_truncated);
    assert_eq!(before_moof.moofs.len(), 2);
    assert!(num_samples(&before_moof) < num_samples(&full));

    for cut in [
        moof_start + 1,
        moof_start + 8,
        moof_start + moof_len / 2,
        moof_start + moof_len - 1,
    ] {
        let mp4 = re_mp4::Mp4::read_bytes(&bytes[..cut]).unwrap();
        assert!(mp4.is_truncated, "cut at {cut}");
        assert_eq!(mp4.moofs.len(), 2, "cut at {cut}");
        assert_eq!(num_samples(&mp4), num_samples(&before_moof), "cut at {cut}");
    }
}

/// A 64-bit box size whose `largesize` field is cut off.
#[test]
fn cut_in_large_size_header() {
    let mut bytes = sample_bytes("hev1.mp4");
    let (mdat_start, _) = find_box(&bytes, b"mdat", 0);
    bytes.truncate(mdat_start);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(b"mdat");
    bytes.extend_from_slice(&[0; 4]);

    let mp4 = re_mp4::Mp4::read_bytes(&bytes).unwrap();
    assert!(mp4.is_truncated);
}

#[test]
fn cut_before_end_of_moov_is_an_error() {
    let bytes = sample_bytes("hev1.mp4");
    let (moov_start, moov_len) = find_box(&bytes, b"moov", 0);

    let result = re_mp4::Mp4::read_bytes(&bytes[..moov_start + moov_len / 2]);
    assert!(matches!(
        result,
        Err(re_mp4::Error::BoxNotFound(re_mp4::BoxType::MoovBox))
    ));
}
