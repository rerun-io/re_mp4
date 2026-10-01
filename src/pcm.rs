//! PCM sample-entry detection and helpers.
//!
//! `QuickTime` / ISOBMFF often store uncompressed audio as one table entry per
//! PCM frame (e.g. 48 000/s). WebCodecs-oriented players need chunk-sized
//! packets instead — see [`crate::Mp4`] track sample building.
//!
//! `FourCC` list aligned with mediabunny `isobmff-demuxer.ts` PCM sample entries.

use crate::FourCC;

/// Returns true if `fourcc` is a known uncompressed / G.711 PCM sample-entry type.
///
/// Used to classify [`crate::StsdBoxContent::Unknown`] entries as audio and to
/// enable chunk-coalesced sample lists when there is no CTTS.
pub fn is_pcm_sample_entry(fourcc: &FourCC) -> bool {
    matches!(
        &fourcc.value,
        b"twos"
            | b"sowt"
            | b"raw "
            | b"in24"
            | b"in32"
            | b"fl32"
            | b"fl64"
            | b"lpcm"
            | b"ipcm"
            | b"fpcm"
            | b"ulaw"
            | b"alaw"
    )
}

/// Best-effort WebCodecs-style codec string for a PCM sample-entry `FourCC`.
///
/// Endianness for `in24`/`in32`/`fl*` follows little-endian QT defaults when
/// the file does not expose a separate endian flag in the stsd we parse today.
pub fn pcm_codec_string(fourcc: &FourCC) -> Option<&'static str> {
    match &fourcc.value {
        b"twos" => Some("pcm-s16be"),
        b"raw " => Some("pcm-u8"),
        b"in24" => Some("pcm-s24"),
        b"in32" => Some("pcm-s32"),
        b"fl32" => Some("pcm-f32"),
        b"fl64" => Some("pcm-f64"),
        b"ulaw" => Some("ulaw"),
        b"alaw" => Some("alaw"),
        // Size/endian come from the sound description; coarse fallback.
        b"sowt" | b"lpcm" | b"ipcm" | b"fpcm" => Some("pcm-s16"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_in24() {
        let fcc = FourCC { value: *b"in24" };
        assert!(is_pcm_sample_entry(&fcc));
        assert_eq!(pcm_codec_string(&fcc), Some("pcm-s24"));
    }

    #[test]
    fn rejects_avc1() {
        let fcc = FourCC { value: *b"avc1" };
        assert!(!is_pcm_sample_entry(&fcc));
    }
}
