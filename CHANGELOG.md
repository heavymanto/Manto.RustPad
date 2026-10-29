# Changelog

Format inspired by "Keep a Changelog". No release has been made yet.

## [Unreleased]

### Added
- **Row navigation** (`scan`): start/end of visual rows without an index and without ever scanning more than `max_scan` bytes (very long lines are cut). **View model** (`view`): from a `Document` to terminal cells — tabs, control characters, non-UTF-8 bytes (one U+FFFD each), wide characters, horizontal scroll, offset <-> column. UI-agnostic: TUI and GUI share it. 19 tests with naive oracles.
- **Languages** (`lang`): English embedded in the binary, every other language a JSON file (one per language, hand-written reader, per-key fallback, `_ai` marker), discovered in the profile folder or next to the executable; `en.json` and `it.json` ship, a test enforces the same keys and placeholders in every file. Same technique as Manto.FreeDisk. 15 tests.
- **Undo / redo** (`Document::seal`, `undo`, `redo`, `can_undo`, `can_redo`, `install_lines`): the history records *pieces*, never document data, so undoing a 19 MiB delete copies nothing; `is_modified` follows the position in the history (undo back to the file = not modified); a successful save clears the history. 13 tests with a whole-copy oracle.
- **Line index that follows the edits** (`Document::build_lines`, `line_count`, `offset_of_line`, `line_at_offset`): line numbers of the *edited* document from the sparse index of the original file plus per-piece newline counts; cost depends on the number of pieces, not on the file size. 12 tests with a naive oracle, one 6 GB test.
- **`PieceTable`** (`edit.rs`): the document as a list of pieces pointing into the original file (never copied) or a small
  buffer of additions; `insert`, `delete`, `read_range` with 64-bit offsets, canonical form (no empty pieces, contiguous
  pieces merged), errors that change nothing. Memory follows the edits, not the file size. 22 tests, with a naive
  `Vec<u8>` oracle on random edits.
- **`Document`** (`document.rs`): a file plus its `PieceTable`, and **`save`**: writes a temporary file in the same folder,
  `fsync`s it, copies the permissions and renames it over the original. Cancel (checked every 4 MiB) and failure leave the
  original untouched and no temporary file; if the file on disk changed since it was opened (length or date, or gone)
  the save is refused (`SourceChanged`) instead of overwriting someone else's work. 16 tests plus one 6 GB test.
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
