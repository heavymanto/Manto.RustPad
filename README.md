# Manto.RustPad

### The editor for files that don't fit in RAM

[![CI](https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml/badge.svg)](https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml)
![license AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-brightgreen)
![status early development](https://img.shields.io/badge/status-early%20development-orange)

*A familiar, non-modal text editor written in Rust whose memory use does not depend on file size, that never silently
truncates your data, and that opens a multi-gigabyte file like a viewer and becomes editable on demand.*

> **Status: early development — a first terminal editor (`mrp`) runs.** Memory-mapped file source, sparse line index, search,
> a piece-table edit layer with undo/redo, a crash-safe save and a terminal UI (view, edit, find, go to, selection and clipboard, column mode, themes, 7 languages) are
> implemented and tested (543 tests + 3 tests on 6 GB files beyond 4 GiB). It has only been tried on Windows. On a 6 GB file it opens at
> once and keeps about 13–35 MiB of private memory, even while saving (it grows slowly with the file, about 3.7 MB per GB: ~410 MiB on a 100 GiB sparse file, whose working set stays under 20 MiB) (one machine, one HDD, a handful of runs: see
> [docs/BENCHMARKS.md](docs/BENCHMARKS.md)); Linux and macOS are untried. The evidence that the gap is real:
> [market survey](docs/MARKET.md) and [reproducible benchmarks](docs/BENCHMARKS.md) of four tools on a 6 GB file.
> 🇮🇹 [Leggi in italiano](README.it.md)

---

## Why

Large text files — logs, SQL dumps, CSV exports, XML — are routine, and the tools around them fall into two camps:

- **Editors that load everything into RAM.** Fast and simple, but the file must fit in memory. We measured
  **Notepad++ at 6.3 GB of RAM** for a 6 GB file, and **Microsoft Edit silently truncating a 6 GB file to 4 GiB — saving it
  destroyed 1.7 GB of data with no warning.**
- **Viewers and streaming editors.** Tiny memory, but the cost moves elsewhere: **LTFViewer** (43 MiB resident) scans the
  entire file at open (≈ 86 s, CPU-bound); **UltraEdit** (19 MiB) writes a full temporary copy of the file and re-reads all of it
  for every jump to the end. Terminal log viewers (lnav, logana, bvr, klogg) are excellent but **read-only**.

We did not want to build "yet another tool that already exists". We looked for one that is light on memory, cheap to
navigate **and** editable — and did not find it ([survey](docs/MARKET.md)).

## What it will be

- **Opens like a viewer, edits when you ask.** Instant, memory-mapped, read-only navigation; press *Edit* and the same
  file becomes editable — no reload, no copy.
- **Memory independent of file size.** Targets in the tens of MiB, whatever the file.
- **Never loses data.** 64-bit offsets throughout, tests on files above 4 GiB from day one, loud failure instead of silent truncation.
- **Column (block) mode, explicitly.** A first-class selection type in the core, so every front end shares it.
- **Honest about encodings.** UTF-8, UTF-16 and UTF-32 (little and big endian, surrogate pairs included) and 52 single-byte code pages (Windows-125x and 874, ISO-8859-x, DOS 437/850/855/…, KOI8, the Mac sets, US-ASCII) and the East-Asian multi-byte encodings (Shift_JIS, EUC-JP, GBK, GB18030, Big5, EUC-KR) on files larger than RAM, chosen with `F9` or `--encoding` or recognised at open (the multi-byte ones are never guessed: choose them by hand).
- **Streaming whole-file operations** (replace-all, sort, split) with progress and cancel.
- **Cross-platform.** Windows, Linux, macOS.
- **One core, several front ends** — terminal (TUI) first, desktop GUI after.

Non-goals: a log analyser (use lnav, logana, klogg) and a plugin/IDE platform.

## Measured on a 6.0 GB file (60 M lines)

| | Memory | Open | Notes |
|---|---|---|---|
| Microsoft Edit 1.2.1 | 4,100 MiB | 2.8 s | **truncates at 4 GiB, loses 1.7 GB on save** |
| Notepad++ 8.8.1 | 6,288 MiB | 7 s warm / 26–30 s cold | holds everything in RAM |
| LTFViewer 5.2u | 43 MiB | UI 0.5 s | full scan in the background, ~86 s |
| UltraEdit-32 10.20.3 | 19 MiB | 7.5 s | writes a 5.7 GB temp copy; `Ctrl+End` re-reads the file (21.6 s) |
| **Manto.RustPad core** (library only) | 19–28 MiB | whole-file line index in 38 s cold / 1.3 s warm | exact, no truncation, no copy; the cold time equals the disk's sequential speed |

Details, method and caveats: [docs/BENCHMARKS.md](docs/BENCHMARKS.md). One machine, mostly one run per cell — see the limitations section.

## Roadmap

1. **Core** — memory-mapped file source, sparse line index, search with cancel. *(viewer mode — done for the library, in `rust/crates/mrp-core`)*
2. **Editing** — piece-table overlay over the original file *(done: `PieceTable`)*; streaming save to a temp file + atomic rename *(done: `Document::save`)*; undo/redo and a line index that follows the edits *(done)*.
3. **TUI front end** (ratatui), classic keyboard model, mouse as a shortcut *(first version done: `mrp`, view/edit/find/go to/themes/languages; measured once on a 6 GB file; to do: SSD, Linux/macOS)*.
4. **Column mode** *(done in the core and in `mrp`: `F8`; not tried with a real keyboard yet)* and **encodings** *(single-byte code pages done: `F9`, `--encoding NAME`, auto-detection at open; UTF-16, UTF-32 and the East-Asian multi-byte encodings done too)*.
5. **GUI front end.**

Each step is gated by the same benchmark suite (`bench/`) so claims stay falsifiable.

## Try it (Windows)

```
cd rust
cargo build --release -p mrp-tui
.\install.ps1                  # optional: mrp.exe -> %USERPROFILE%\.local\bin,
                               # languages -> %APPDATA%\mrp\lang
mrp yourfile.txt               # F1 = help, F4 = edit, F10 = quit
```

`install.ps1` never touches your `PATH` (it only tells you if the folder is missing from it) and never touches your saved settings.
Without installing: `cargo run --release -p mrp-tui -- yourfile.txt`.

**Portable:** `.\portable.ps1` (after the build) makes a folder with `mrp.exe` and `lang\`; copy it anywhere. As long as the `lang` folder is next to the exe, languages and saved settings (`settings.txt`) stay in that folder and nothing is written to your profile.

## Repository layout

| Path | Contents |
|---|---|
| `docs/MARKET.md` | Landscape of existing tools, with sources |
| `docs/BENCHMARKS.md` | Measurements, method, limitations |
| `rust/crates/mrp-core` | The core library (Rust): `FileSource`, `LineIndex`, `PieceTable`/`Document` (edit, undo, save), search, view model, languages, tests, `info` example |
| `rust/crates/mrp-tui` | The terminal editor `mrp` (ratatui): app state machine, screen, themes, settings, command line |
| `rust/install.ps1` | Local install on Windows: `mrp.exe` and the language files as user commands, with a self-test |
| `bench/` | Reproducible scripts (Windows, Python 3.12, no dependencies) and raw results |
| `CONTRIBUTING.md` | How to build, and the rules the code follows (64-bit offsets, never the whole file in memory…) |
| `CHANGELOG.md`, `SECURITY.md` | What changed; how to report a vulnerability privately |

## Sibling project

[Manto.FreeDisk](https://github.com/heavymanto/Manto.FreeDisk) — a Rust disk-space analyzer; the core/CLI/TUI split used there is the model for this repository.

## License

Copyright (C) 2026 Francesco Mantovanelli.

This program is free software: you can redistribute it and/or modify it under
the terms of the **GNU Affero General Public License v3.0** as published by the
Free Software Foundation — see [LICENSE](LICENSE). Commercial licensing is
available on request for products that cannot ship under the AGPL.

---

<div align="center">

**Author** — Francesco Mantovanelli · **Project** — Manto.RustPad

</div>
