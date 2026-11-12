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
        <img src="https://img.shields.io/badge/version-0.1.6-blue" alt="version 0.1.6" />
        <img src="https://img.shields.io/badge/tests-1673-brightgreen" alt="1673 tests" />
        <img src="https://img.shields.io/badge/frontends-mrp--tui%20%C2%B7%20mrp--gui-blue" alt="mrp-tui and mrp-gui" />
        <img src="https://img.shields.io/badge/languages-7-blue" alt="seven languages" />
        <img src="https://img.shields.io/badge/license-AGPL--3.0-brightgreen" alt="AGPL-3.0" />
        <img src="https://img.shields.io/badge/status-early%20development-orange" alt="early development" />
      </p>
      <p>
        <a href="#quickstart"><img src="https://img.shields.io/badge/%F0%9F%94%A7-QUICKSTART-007ec6?style=flat-square" alt="Quickstart" /></a>
        <a href="README.it.md"><img src="https://img.shields.io/badge/%F0%9F%87%AE%F0%9F%87%B9-LEGGILO%20IN%20ITALIANO-007ec6?style=flat-square" alt="Leggilo in italiano" /></a>
      </p>
    </td>
  </tr>
</table>

---

## Abstract

<p align="justify">A 100 GB log opens in a few seconds, scrolls smoothly, navigates in a timeline, lays its record lengths out in a checkable histogram and can be edited. The file is never loaded into RAM.</p>

<p align="justify">
I wrote it because I could not find a tool that did all of these things together: the editors that edit well load everything into memory, the viewers that hold large files cannot edit, and no one had a visual timeline. And the OS text editor, on a file above 4 GiB, truncates it on save without warning — 1.7 GB lost, measured, not a rumour (the proof is in the table below). So I wrote the tool I needed, with the same layout as Manto.FreeDisk: one engine, two interfaces, the terminal and the window.
</p>

## How it's built

Six well-known decisions, applied all the way:

1. **The file stays on disk, not in RAM.** It is memory-mapped read-only: the kernel hands over the pages the editor asks for, when it asks for them (the way `less` works). The line index builds in the background and pauses when the user jumps, so it does not compete for the disk. On a 100 GB file the editor uses tens of MiB.

2. **Saving is atomic, the session is a journal of the edits.** The new file is written next to the old one, flushed to disk and swapped in with a rename: the same protocol SQLite uses for its commits. The session records only the edits, so after a crash, or after closing without saving, tabs, cursors and unsaved edits are back in place. The piece table is the structure Crowley described for text editors in 1998 and VS Code adopted in 2018.

3. **The levels show before you read them.** ERROR/WARN/INFO/DEBUG in colour, like dates and times: preattentive colour is processed by the eye before the line is read (Healey's work on preattentive processing). A spike of errors in a log of millions of lines is noticed as soon as it enters the screen.

4. **The timeline is a proportional map of the file.** A column of periods (minute, ten minutes, hour, day), each as tall as its volume of lines and split by level: the dense zones show before you reach them by scrolling. The periods are found by binary search, without reading the file.

5. **Multi use, multi platform.** It runs in a terminal (over SSH, for instance) with no graphical interface needed, or in a window on the desktop. It works on Windows, Linux and macOS.

6. **The record length check spots the outliers at a glance.** `Alt+H` counts how many lines have each content length (bytes, line ending excluded) and lays them out in a checkable histogram: the dominant formats first, the out-of-size lines at the bottom, where the eye catches them. Tick the lengths to see and the view is remade with only those lines — in a fixed-record file, the anomalies stand out on their own.

## Measurements

A 6.0 GB file (60 million lines), one machine, one hard disk:

| | Memory | Open | What happens |
|---|---:|---|---|
| The editor everyone uses | 6,288 MiB | 7 s warm / 26–30 s cold | holds everything in RAM |
| The one from the OS | 4,100 MiB | 2.8 s | **truncates at 4 GiB — loses 1.7 GB on save** |
| The streaming viewer | 43 MiB | UI 0.5 s | full scan in the background, ~86 s |
| The commercial one | 19 MiB | 7.5 s | writes a 5.7 GB temp copy; `Ctrl+End` re-reads the file |
| **Manto.RustPad** | **13–35 MiB** | **instantly** | exact, no truncation, no copy; the index builds in the background |

---

## Two frontends, one engine

The engine is one, `mrp-core` (documents, tabs, search, edits, sessions), with two interfaces: **`mrp-tui`** for the terminal, made for servers and SSH; **`mrp-gui`** for the desktop, with menus, toolbar, dialogs, system clipboard and drag-and-drop. A feature added to the engine is in both.

| `mrp-tui` — in the terminal | `mrp-gui` — in a window |
|:---:|:---:|
| <a href="#two-frontends-one-engine"><img src="docs/img/tui-timeline.gif" alt="mrp-tui in motion: the log, the scroll, the timeline that lights up, the level colours" /></a> | <a href="#two-frontends-one-engine"><img src="docs/img/gui-demo.gif" alt="mrp-gui in motion: the coloured log in the window, the scroll" /></a> |

| | `mrp-tui` | `mrp-gui` |
| --- | --- | --- |
| **Start it** | `mrp-tui file...` | `mrp-gui file...` |
| **Look** | six terminal themes | Blue (Total Commander style), Debian, Dark, Light — or Automatic, which follows the system |
| **Timeline** | `Alt+B`, a column next to the text | the same, or a bar next to the scrollbar |
| **Menus** | the function keys | real menus, toolbar, status bar, scrollbar |

## Features

- Any file opens instantly; scroll with wheel, scrollbar and page jumps (`Alt+PgUp/PgDn`: 10 pages, Shift 100, Ctrl 1000).
- Go to a line (`Ctrl+G`) or to a time; follow the end of the file like `tail -f` (`Alt+T`).
- `grep`-style filters (`Alt+G`), level colours (`Alt+L`), timeline (`Alt+B`), record length check (`Alt+H`).
- Edit without reloading (`F4`); column mode (`F8` or `Alt`+drag): typing, `Tab`, copy, cut and paste on every row of the block.
- Find and replace (`Ctrl+F` / `Ctrl+H`) on text, glob and regex, with a preview and one `Ctrl+Z` that undoes the whole thing.
- Several tabs at once; the session restores tabs, cursors and unsaved edits (on a 100 GB file with a few corrections it weighs a few KB).
- Encodings: UTF-8/16/32, 53 single-byte code pages, the East-Asian multi-byte ones, beyond RAM.
- 64-bit offsets, tests above 4 GiB, a loud error instead of silent truncation; memory stays tens of MiB whatever the size.

It is not a log analyser (the reading aids here are what a viewer needs) and it is not a plugin platform or an IDE.

## Get it

Current release: **0.1.6** (November 2026).

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
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.6/manto-rustpad-0.1.6-win-x64.zip"><img src="https://img.shields.io/badge/%E2%AC%87-Windows%20x64-2ea44f?style=for-the-badge" alt="Windows x64" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.6/manto-rustpad-0.1.6-linux-x64.tar.gz"><img src="https://img.shields.io/badge/%E2%AC%87-Linux%20x64-2ea44f?style=for-the-badge" alt="Linux x64" /></a>
  <a href="https://github.com/heavymanto/Manto.RustPad/releases/download/v0.1.6/manto-rustpad-0.1.6-macos-aarch64.tar.gz"><img src="https://img.shields.io/badge/%E2%AC%87-macOS-2ea44f?style=for-the-badge" alt="macOS" /></a>
</p>
</details>
</div>

You can also build it from source: two commands, in the [Quickstart](#quickstart).

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

- Early development: 1,538 automated tests, but not yet long sessions of real use. Keep a backup of anything precious.
- Windows first. The terminal editor and the core also run on Linux (Debian, WSL); the CI runs the whole suite on all three systems at every push.
- The window (`mrp-gui`) has been checked with automated tests and screenshots, not yet with long real keyboard and mouse use.
- Regex, glob and ignore-case work on UTF-8 documents only; plain-text search works in every encoding.

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
| `bench/` | reproducible scripts and raw results |

## Documentation

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
