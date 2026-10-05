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
/// Only returns a string when the `FourCC` alone determines the format. Entries
/// that need sound-description sample size, endian flags, or a `pcmC` box
/// (`in24`/`in32`/`fl*`/`lpcm`/`ipcm`/`fpcm`, and 8-bit `twos`/`sowt`) return
/// [`None`] — we do not parse that metadata for [`crate::StsdBoxContent::Unknown`]
/// today, and inventing LE/s16 defaults would mislead consumers.
pub fn pcm_codec_string(fourcc: &FourCC) -> Option<&'static str> {
    match &fourcc.value {
        // FourCC encodes endianness; 16-bit is the common QT sample size.
        b"twos" => Some("pcm-s16be"),
        b"sowt" => Some("pcm-s16"),
        b"raw " => Some("pcm-u8"),
        b"ulaw" => Some("ulaw"),
        b"alaw" => Some("alaw"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_in24_without_inventing_codec() {
        let fcc = FourCC { value: *b"in24" };
        assert!(is_pcm_sample_entry(&fcc));
        assert_eq!(pcm_codec_string(&fcc), None);
    }

    #[test]
    fn unambiguous_pcm_fourccs() {
        assert_eq!(
            pcm_codec_string(&FourCC { value: *b"twos" }),
            Some("pcm-s16be")
        );
        assert_eq!(
            pcm_codec_string(&FourCC { value: *b"sowt" }),
            Some("pcm-s16")
        );
        assert_eq!(
            pcm_codec_string(&FourCC { value: *b"raw " }),
            Some("pcm-u8")
        );
        assert_eq!(pcm_codec_string(&FourCC { value: *b"fpcm" }), None);
        assert_eq!(pcm_codec_string(&FourCC { value: *b"ipcm" }), None);
        assert_eq!(pcm_codec_string(&FourCC { value: *b"lpcm" }), None);
    }

    #[test]
    fn rejects_avc1() {
        let fcc = FourCC { value: *b"avc1" };
        assert!(!is_pcm_sample_entry(&fcc));
    }
}
