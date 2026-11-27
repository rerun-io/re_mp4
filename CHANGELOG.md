# `re_mp4` Changelog

## 0.6.0 - 2026-11-27
* ⚠️ Breaking: Remove leftover box size calculations and writer-only types [#34](https://github.com/rerun-io/re_mp4/pull/34) by [@emilk](https://github.com/emilk)
* Parse QuickTime Sound Description version 2 in mp4a [#35](https://github.com/rerun-io/re_mp4/pull/35) by [@alexantoshuk](https://github.com/alexantoshuk)
* Coalesce PCM sample tables into chunks [#38](https://github.com/rerun-io/re_mp4/pull/38) by [@alexantoshuk](https://github.com/alexantoshuk)
* Parse MP4 files that end in the middle of a box [#40](https://github.com/rerun-io/re_mp4/pull/40) by [@rerun-rmack](https://github.com/rerun-rmack)

## 0.5.1 - 2026-07-08
* Improve crate docstring [#26](https://github.com/rerun-io/re_mp4/pull/26)
* Fix `Sample::is_sync` for fragmented mp4s [#27](https://github.com/rerun-io/re_mp4/pull/27)

## 0.5.0 - 2026-05-04
* Update VP8 codec_string to be WebGPU compliant [#22](https://github.com/rerun-io/re_mp4/pull/22) by [@AyushAgrawal-A2](https://github.com/AyushAgrawal-A2)
* Update to Rust 1.92 and enable many more clippy lints [#23](https://github.com/rerun-io/re_mp4/pull/23) by [@Wumpf](https://github.com/Wumpf)

## 0.4.0 - 2025-09-16 - Handle constant frame size videos
* Fix edge case for parsing video with constant frame size [#20](https://github.com/rerun-io/re_mp4/pull/20) by [@ntjohnson1](https://github.com/ntjohnson1)

## 0.3.0 - 2024-11-13 - Handle time shifts
* Account for video with DTS shift and resulting negative dts values [#16](https://github.com/rerun-io/re_mp4/pull/16) by [@Wumpf](https://github.com/Wumpf)
* Shift DTS & CTS by minimum CTS to mimic `ffprobe`'s behavior [#17](https://github.com/rerun-io/re_mp4/pull/17) by [@Wumpf](https://github.com/Wumpf)


## 0.2.1 - 2024-11-12 - Bug fixes
* Fix integer overflow when ctts contains entries with negative offsets [#14](https://github.com/rerun-io/re_mp4/pull/14) by [@Wumpf](https://github.com/Wumpf)
* Handle negative data_offset in TrunBox [#15](https://github.com/rerun-io/re_mp4/pull/15) by [@Wumpf](https://github.com/Wumpf)


## 0.2.0 - 2024-11-11 - Faster video parsing
* Optimize mp4 parse times by not copying video data [#12](https://github.com/rerun-io/re_mp4/pull/12) by [@jprochazk](https://github.com/jprochazk)


## 0.1.0 - 2024-10-14
Initial release
