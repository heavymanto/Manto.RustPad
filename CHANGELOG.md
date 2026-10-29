# Changelog

Format inspired by "Keep a Changelog". No release has been made yet.

## [Unreleased]

### Added
- **`mrp-core` 0.1.0** — the engine, a library with no user interface yet:
  - `FileSource`: read-only memory-mapped file, 64-bit offsets, bytes (not
    UTF-8), empty file handled, error on anything that is not a regular file.
  - `LineIndex`: sparse line index (one checkpoint every 256 lines per 4 MiB
    block), built in parallel, with cancel and progress. The line separator is
    `\n`; a final line without `\n` counts as a line; the empty file has 0 lines.
    On a cold 6 GB file on a spinning disk a prefetch thread keeps the read
    sequential: 38 s (disk speed) instead of 145 s.
  - `find_next` / `find_prev`: byte search, block by block with overlap, with cancel.
  - `info` example: line count, checkpoints, timing and a few sample lines of any file.
- **Tests**: 30, with independent naive oracles, plus one `#[ignore]`d test on a
  6 GB file (line 60,000,000 starts beyond 4 GiB).
- **Benchmarks** of Microsoft Edit, Notepad++, LTFViewer and UltraEdit on a
  6 GB file, and of the core — [docs/BENCHMARKS.md](docs/BENCHMARKS.md), scripts
  and raw data in [bench/](bench).
- **Market survey**: [docs/MARKET.md](docs/MARKET.md).
