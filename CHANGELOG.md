# Changelog

Format inspired by "Keep a Changelog". No release has been made yet.

## [Unreleased]

### Added
- **`mrp` measured on a 6 GB file** (`bench/mrp_bench.py`, [docs/BENCHMARKS.md](docs/BENCHMARKS.md)): first screen under a second, 13–35 MiB private memory including during the save, line jump to 30,000,000 lands on the exact byte, edit + save of 6 GB takes about 91–98 s on the HDD and leaves the file correct (+3 bytes, text in the right place). One machine, few runs.
- **`rust/install.ps1`**: local install on Windows, same idea as Manto.FreeDisk's — copies `mrp.exe` to `%USERPROFILE%\.local\bin` and the seven language files to `%APPDATA%\mrp\lang`, replaces a running `mrp.exe` by renaming it, never touches `PATH` or the saved settings, and ends with a self-test (`--help`, `--dump` in English and in Italian from the installed language folder).
- **`mrp`, the terminal editor** (`mrp-tui`, ratatui): opens the file at once (the line index is built in the background and line numbers appear when it is ready), view mode by default and `F4` to edit; arrows, Home/End, PgUp/PgDn, Ctrl+Home/End, mouse wheel; `F5`/`Ctrl+G` go to a line, `@byte offset` or `%percent`; `F3`/`Ctrl+F`/`Shift+F3` find next/previous in a background thread (exact bytes, wraps around, cancellable with Esc) on the *edited* document; undo/redo (`Ctrl+Z`/`Ctrl+Y`); `F2`/`Ctrl+S` saves in the background with a cancellable progress dialog; `F10`/`Ctrl+Q` asks what to do with unsaved changes; `F1` help; `F6` themes (Night, Debian, Console, Minimal) and `F7` languages, both saved in the profile. Developer options `--dump WxH --keys ...` print the screen as text without a terminal, which is how the UI is tested. 76 tests (state machine, screen, dialogs, editing, find, command line).
- **Search on the edited document** (`docsearch`): a `Snapshot` (shared mapped file + copy of the piece list, no document data) can be handed to another thread; `find_next`/`find_prev` read block by block with overlap, report progress and honour a cancel flag. 8 tests with a naive oracle.
- **Languages**: Spanish, Brazilian Portuguese, German, Japanese and Simplified Chinese added as machine translations, marked `_ai` and flagged `[AI]` in the language menu; the same-keys-and-placeholders test now requires all 7 files.
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
