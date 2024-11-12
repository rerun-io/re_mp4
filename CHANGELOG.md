# `re_mp4` Changelog


## 0.2.1 - 2024-11-12 - Bug fixes
* Fix integer overflow when ctts contains entries with negative offsets [#14](https://github.com/rerun-io/re_mp4/pull/14) by [@Wumpf](https://github.com/Wumpf)
* Handle negative data_offset in TrunBox [#15](https://github.com/rerun-io/re_mp4/pull/15) by [@Wumpf](https://github.com/Wumpf)


## 0.2.0 - 2024-11-11 - Faster video parsing
* Optimize mp4 parse times by not copying video data [#12](https://github.com/rerun-io/re_mp4/pull/12) by [@jprochazk](https://github.com/jprochazk)


## 0.1.0 - 2024-10-14
Initial release
