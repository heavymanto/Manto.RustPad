<table>
  <tr>
    <td valign="middle">
      <h1>Manto.RustPad</h1>
      <h3>A log viewer for files that don't fit in RAM</h3>
      <p>
        <img src="https://img.shields.io/badge/OPENS-INSTANTLY-e3b341?style=for-the-badge" alt="opens instantly" />
        <img src="https://img.shields.io/badge/TIMELINE-OF%20THE%20FILE-e3b341?style=for-the-badge" alt="timeline" />
        <img src="https://img.shields.io/badge/SCROLLS-100%20GB%20FILES-e3b341?style=for-the-badge" alt="100 GB" />
        <img src="https://img.shields.io/badge/COLUMN--MODE-EDITING-e3b341?style=for-the-badge" alt="column mode" />
        <img src="https://img.shields.io/badge/NEVER-TRUNCATES-e3b341?style=for-the-badge" alt="never truncates" />
      </p>
      <p>
        <a href="https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml"><img src="https://github.com/heavymanto/Manto.RustPad/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
        <img src="https://img.shields.io/badge/version-0.1.4-blue" alt="version 0.1.4" />
        <img src="https://img.shields.io/badge/tests-1538-brightgreen" alt="1538 tests" />
        <img src="https://img.shields.io/badge/frontends-mrp--tui%20%C2%B7%20mrp--gui-blue" alt="mrp-tui and mrp-gui" />
        <img src="https://img.shields.io/badge/languages-7-blue" alt="seven languages" />
        <img src="https://img.shields.io/badge/license-AGPL--3.0-brightgreen" alt="AGPL-3.0" />
        <img src="https://img.shields.io/badge/status-early%20development-orange" alt="early development" />
      </p>
      <p>
        <a href="docs/BENCHMARKS.md"><img src="https://img.shields.io/badge/%F0%9F%93%96-MEASUREMENTS-007ec6?style=flat-square" alt="Measurements" /></a>
        <a href="#quickstart"><img src="https://img.shields.io/badge/%F0%9F%94%A7-QUICKSTART-007ec6?style=flat-square" alt="Quickstart" /></a>
        <a href="README.it.md"><img src="https://img.shields.io/badge/%F0%9F%87%AE%F0%9F%87%B9-LEGGILO%20IN%20ITALIANO-007ec6?style=flat-square" alt="Leggilo in italiano" /></a>
      </p>
    </td>
  </tr>
</table>

---

## Abstract

**Open a log of 100 GB in a blink. See its whole life in a timeline. And when you need to, edit it — column by column. The file never enters RAM.**

<p align="justify">
The idea comes from a frustration I know well: a log of many gigabytes and no tool that could hold it. The editors that are good at editing load everything into RAM — and fall. The viewers that are good at scrolling cannot edit — full stop. And one of them, the most trusted one, the one that sits on every machine: on saving, it truncated the file in silence and destroyed 1.7 GB of data without asking. That day I stopped searching and decided to write my own — the same way I had already done for the disk: one engine, two frontends, in a window and in the terminal.
</p>

**Manto.RustPad** is that tool. It opens the file memory-mapped and read-only, builds a sparse line index in the background, and the moment you press *Edit* the same file becomes editable — no reload, no copy. Your edits live in a small piece table next to the untouched original; saving streams a new file and swaps it in atomically.

## The four pillars

Each one stands on a well-known idea — this is not invention, it is the literature, applied:

1. **A big file opens in seconds — and stays open.** The file is *memory-mapped*: the kernel hands the editor the pages it asks for, as it asks for them (the way `less` and the big log viewers work). The line index builds in the background, and **pauses while you jump** so the disk stays yours. Reading 100 GB costs tens of MiB because nothing is ever copied.

2. **Nothing is ever lost — not even what you did not save, not even on a crash.** Saving is *atomic*: the new file is written whole next to the old one, flushed, then swapped in with a single rename — the same all-or-nothing protocol SQLite uses for its commits. And the session is a journal of *changes only*: close without asking, kill the process, lose power — reopen, and the tabs, the cursors and the **unsaved edits** are back (the piece table keeps the original intact and the edits apart, the structure Crowley described for text editors in 1998 and VS Code adopted in 2018).

3. **The levels, at a glance.** ERROR in red, WARN in yellow, INFO in green, dates and times in their own hues: colour is *preattentive* — the eye parses it before reading a single line (Healey's work on preattentive processing). On a log with storms of errors, you see the storm before you find it.

4. **The timeline — one of a kind.** A column of periods, each as long as its volume of lines, split by level: a proportional map of the whole file, where the dense spots *are* the problems. Scroll it, and the log reads like an X-ray of itself: where things went wrong shows before any line is read. The periods are found by binary search, without reading the file.

## Measured, not promised

On a 6.0 GB file (60 million lines), one machine, one HDD — the whole method in [docs/BENCHMARKS.md](docs/BENCHMARKS.md):

| | Memory | Open | What happens |
|---|---:|---|---|
| The editor everyone uses | 6,288 MiB | 7 s warm / 26–30 s cold | holds everything in RAM |
| The one from the OS | 4,100 MiB | 2.8 s | **truncates at 4 GiB — loses 1.7 GB on save** |
| The streaming viewer | 43 MiB | UI 0.5 s | full scan in the background, ~86 s |
| The commercial one | 19 MiB | 7.5 s | writes a 5.7 GB temp copy; `Ctrl+End` re-reads the file |
| **Manto.RustPad** | **13–35 MiB** | **instantly** | exact, no truncation, no copy — the index fills in behind you |

Memory grows at about 3.7 MB per GB of file — and never holds the file. A 100 GB log on a hard disk opens in 36 ms. *(Who is who, with versions and sources: [docs/MARKET.md](docs/MARKET.md).)*

---

## Two frontends, one engine

The engine is one (`mrp-core`: documents, tabs, search, edits, sessions) — and two ways to look at it: **`mrp-tui`** for any terminal, made for servers and SSH; **`mrp-gui`** for the desktop, with real menus, toolbar, dialogs, the system clipboard and drag-and-drop. A feature added to the engine is in both.

| `mrp-tui` — in the terminal | `mrp-gui` — in a window |
|:---:|:---:|
| <a href="#two-frontends-one-engine"><img src="docs/img/tui-timeline.gif" alt="mrp-tui in motion: the log, the scroll, the timeline that lights up, the level colours" /></a> | <a href="#two-frontends-one-engine"><img src="docs/img/gui-demo.gif" alt="mrp-gui in motion: the coloured log in the window, the scroll" /></a> |

| | `mrp-tui` | `mrp-gui` |
| --- | --- | --- |
| **Start it** | `mrp-tui file...` | `mrp-gui file...` |
| **Look** | six terminal themes | Blue (Total Commander style), Debian, Dark, Light — or Automatic, which follows the system |
| **Timeline** | `Alt+B`, a column next to the text | the same, or a bar next to the scrollbar |
| **Menus** | the function keys | real menus, toolbar, status bar, scrollbar |

## In action

`mrp-tui` on a log: the file opens at once, the scroll runs free, then the timeline lights up (`Alt+B`) — each period as long as its volume of lines, split by level (ERROR, WARN, INFO, DEBUG), the storms of errors visible at a glance — and the level colours come in (`Alt+L`):

<a href="#in-action"><img src="docs/img/tui-timeline.gif" alt="mrp-tui in motion: scroll, timeline, level colours" /></a>

And `mrp-gui`, the same engine in a window — the log opens, the scroll runs, in full colour:

<a href="#in-action"><img src="docs/img/gui-demo.gif" alt="mrp-gui in motion: the log in the window, the scroll" /></a>

## What it does

- **A log viewer for huge files.** Opens instantly, scrolls with wheel, scrollbar and page jumps (`Alt+PgUp/PgDn`: 10 pages, with Shift 100, with Ctrl 1000). Line numbers fill in from a background index that **pauses while you jump**. Go to a line (`Ctrl+G`) or to a time, follow the end like `tail -f` (`Alt+F`), filter lines like `grep` (`Alt+G`), colour the log levels (`Alt+L`).
- **A timeline of the file.** A column of periods — a minute, ten minutes, an hour, a day — each as long as its volume of lines and split by level, so where the errors are shows at a glance. The part you are reading is marked; a click or a drag takes you there. The periods are found by binary search **without reading the file**.
- **…and you can edit it, with column editing.** Press `F4` and the same file becomes editable — no reload, no copy. **Column (block) mode** (`F8`, or `Alt`+drag with the mouse): typing, `Tab`, copy, cut and paste act on every row of the block at once, in a file of any size.
- **Memory independent of file size.** Tens of MiB, whatever the file. Never the whole file in memory.
- **Never loses data.** 64-bit offsets throughout, tests on files above 4 GiB, loud failure instead of silent truncation, crash-safe save.
- **Tabs, and closing loses nothing.** Several files at once; quit without being asked — next time the tabs, the cursors and the **unsaved changes** are back. Only the changes are saved: on a 100 GB file with a few edits, the session is a few KB.
- **Find and Replace with glob and regex** (`Ctrl+F` / `Ctrl+H`), with a **preview** before anything changes and one `Ctrl+Z` that undoes the whole thing.
- **Honest about encodings.** UTF-8/16/32, 53 single-byte code pages, the East-Asian multi-byte ones — on files larger than RAM.

Non-goals: a log analyser (the log aids here are what a *viewer* needs), and a plugin/IDE platform.

## Get it

Current release: **0.1.4** (November 2026).

Nothing to install: extract and run. Both programs work right away in portable mode (English embedded, the other six languages in `lang/` next to the programs). If you want them as commands from any folder, each package carries `install.ps1` (Windows) or `install.sh` (Linux, macOS). On Windows, `install.ps1 -Associate` adds mrp-gui to the "Open with" list of `.txt` and `.log` and to Settings > Default apps, for the current user only and without admin: the default itself stays a choice Windows signs. On Linux, `./install.sh --associa` sets the default for text files.

<div align="center">
<details>
<summary><h3>💚 ⬇️ DOWNLOAD ⬇️ 💚</h3></summary>
<br/>
<p align="center">
  <a href="#get-it"><img src="docs/img/star.png" alt="Star" width="120" /></a>
  <br/><br/>
  <b>We give you the software. You give us a star.</b> ⭐<br/>
  <i>Stars make dreams — and for a young project they are the proof<br/>
  it matters. They fuel what comes next.</i>
  <br/><br/>
  <a href="https://github.com/heavymanto/Manto.RustPad"><img src="https://img.shields.io/badge/%E2%AD%90-GIVE%20US%20A%20STAR-e3b341?style=for-the-badge" alt="Give us a star" /></a>
  <br/>
  <a href="https://github.com/heavymanto/Manto.RustPad/stargazers"><img src="https://img.shields.io/github/stars/heavymanto/Manto.RustPad?style=flat-square&label=stars" alt="stars" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases"><img src="https://img.shields.io/github/downloads/heavymanto/Manto.RustPad/total?style=flat-square&label=downloads" alt="downloads" /></a>
  <br/><br/>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.4/manto-rustpad-0.1.4-win-x64.zip"><img src="https://img.shields.io/badge/%E2%AC%87-Windows%20x64-2ea44f?style=for-the-badge" alt="Windows x64" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.4/manto-rustpad-0.1.4-linux-x64.tar.gz"><img src="https://img.shields.io/badge/%E2%AC%87-Linux%20x64-2ea44f?style=for-the-badge" alt="Linux x64" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.4/manto-rustpad-0.1.4-macos-aarch64.tar.gz"><img src="https://img.shields.io/badge/%E2%AC%87-macOS-2ea44f?style=for-the-badge" alt="macOS" /></a>
</p>
</details>
</div>

You can also build it from source — two commands, in the [Quickstart](#quickstart).

## Quickstart

You need [Rust](https://rustup.rs) and a clone of this repository.

```sh
cd rust
cargo build --release                 # mrp-tui and mrp-gui in target/release/
./target/release/mrp-tui yourfile.txt     # F1 = help, F4 = edit, F10 = quit
./target/release/mrp-gui yourfile.txt    # the same, in a window
```

No log at hand? Make one, any size, same bytes on every machine:

```sh
./target/release/mrp-loggen --size 2M --scenario bursts demo.log
```

## Known limitations

- **Early development.** 1,538 automated tests, but not yet long sessions of real use. Keep a backup of anything precious.
- **Windows first**; the terminal editor and the core also run on Linux (Debian, WSL). The CI runs the whole suite on all three systems at every push.
- **The window (`mrp-gui`)** has been checked with automated tests and screenshots, not yet with long real keyboard and mouse use.
- Regex, glob and ignore-case work on UTF-8 documents only; plain-text search works in every encoding.
- Measurements are from one machine and one HDD: [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

## Development

```sh
cd rust
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Repository layout

| Path | Contents |
|---|---|
| `rust/crates/mrp-core` | the engine: `FileSource`, `LineIndex`, `PieceTable`/`Document`, search, sessions, encodings, languages |
| `rust/crates/mrp-tui` | the terminal editor (ratatui) |
| `rust/crates/mrp-gui` | the desktop window (egui) |
| `rust/crates/mrp-loggen` | synthetic logs of any size, same bytes for the same seed |
| `docs/` | `MARKET.md` (the landscape, with sources), `BENCHMARKS.md` (measurements, method) |
| `bench/` | reproducible scripts and raw results |

## Documentation

- [docs/MARKET.md](docs/MARKET.md) — the landscape of existing tools, with sources
- [docs/BENCHMARKS.md](docs/BENCHMARKS.md) — measurements, method, limitations
- [CHANGELOG.md](CHANGELOG.md) — what changed
- [SECURITY.md](SECURITY.md) — how to report a vulnerability privately

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The short version: offsets are 64-bit, the whole file is never held in memory, and every number in the docs was really measured. This is a hobby project — everything gets read, but no response times are promised.

## Sibling project

[Manto.FreeDisk](https://github.com/heavymanto/Manto.FreeDisk) — a Rust disk-space analyzer; the core/frontend split used there is the model for this repository.

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