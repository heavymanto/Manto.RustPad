# Landscape: tools for very large text files

<p align="justify">
Re­search date: 2026-10-28. Ev­ery­thing below comes from public pages listed
in [Sources](#sources) unless marked **(mea­sured)** (our own tests, see
[BENCHMARKS.md](BENCHMARKS.md)) or **(from memory)** (not re-checked,
treat as a lead).
</p>
<p align="justify">
I did not want to build "yet another tool that already exists". This page
records what does exist, so the gap Manto.RustPad aims at is ex­plic­it —
and fal­si­fi­able. The README tells the story without names; this is where
the names live, each with its source.
</p>

## 1. Terminal viewers (read-only)

| Tool | Language | How it handles big files | Edits? |
|---|---|---|---|
| [lnav](https://docs.lnav.org/en/latest/performance.html) | C++ | Indexes every line; its documentation benchmarks a 3.3 GB / 10M-line log and shows the UI while loading. The maintainer says file contents are never kept in memory, only the line index (a commenter reports a few hundred MB of RAM on that file). | No |
| [logana](https://docs.rs/crate/logana/0.1.0/source/ARCHITECTURE.md) | Rust (ratatui) | `mmap` + 16 MiB instant preview, then a background parallel line index (Rayon, ≥ 4 MiB chunks). Aho-Corasick filters, vim-style keys, per-file filter profiles in SQLite. | No (marks and comments only) |
| [bvr](https://docs.rs/crate/bvr/0.0.4) | Rust | Log pager with regex/literal filters, tabs, follow mode. Mechanism for big files not documented. v0.0.4, "heavy development". | **No — editing is a stated non-goal** |
| [Toolong](https://pypi.org/project/toolong/1.0.1) | Python | tail/less replacement for logs. | No |
| `less` | C | **(from memory)** Reads lazily; jumping to a line number needs a scan. | No |

## 2. GUI viewers

| Tool | Notes |
|---|---|
| [klogg / glogg](https://www.saashub.com/compare-lnav-vs-klogg) | "grep + less" for huge logs, fast search. GUI only, read-only. |
| BareTail | Windows-only, proprietary; listed as able to open files > 2 GB. |
| [large-text-viewer](https://rust-digger.code-maven.com/crates/large-text-viewer) | GUI. Goal: open > 4 GB in < 100 ms without loading it into RAM; full index only for files < 10 MB. |
| LTFViewer | Windows. **(measured)** — see below. |

## 3. Editors

| Tool | Notes |
|---|---|
| [Microsoft Edit](https://heise.de/-10450362) | Rust TUI, ships with Windows 11, Linux/macOS builds. Press coverage says it opens gigabyte files quickly ([Slashdot](https://tech.slashdot.org/story/25/06/24/2039231)). **(measured)**: loads the whole file in RAM and silently truncates at 4 GiB — see [BENCHMARKS.md](BENCHMARKS.md). |
| [Emacs + VLF](https://elpa.gnu.org/devel/vlf.html) | Views, edits, searches and compares large files in batches, trading memory for CPU. The strongest *editing* option we found. |
| Vim (+ LargeFile plugin) | The plugin disables undo, syntax and events above a threshold (default 100 MB); Vim still tries to load the whole file ([discussion](https://www.baeldung.com/linux/vim-large-files-performance)). |
| Notepad++ | **(measured)** — see [BENCHMARKS.md](BENCHMARKS.md). |
| UltraEdit | **(measured)** — see [BENCHMARKS.md](BENCHMARKS.md). |
| EmEditor | Commercial GUI editor marketed for very large files; reviews conflict on how it compares with UltraEdit ([comparison](https://www.techjockey.com/compare/emeditor-vs-ultraedit)). **Not tested by us.** |

## 4. What we did *not* find

These are absence-of-evidence statements from a limited search; they are hypotheses, not facts.

1. A tool that **navigates like a viewer and turns into an editor** on the same multi-GB file without loading it.
2. A terminal tool with **column (block) mode on huge files**.
3. A big-file tool aimed at **general text and data files** (CSV, SQL dumps, XML, fixed-width) rather than logs, with
   **UTF-8, UTF-16 and legacy code pages** handled on files larger than RAM.
4. **Streaming whole-file operations** (replace-all, sort, split) on files larger than RAM with progress and cancel.
5. **Instant approximate positioning** (jump to 40 % of the file immediately; exact line numbers refined later)
   instead of "index everything first".

## 5. Where Manto.RustPad aims

> *The editor for files that do not fit in RAM* — a familiar (non-modal) editor whose memory use does not depend on
> file size, that never silently truncates, and that opens like a viewer and becomes editable on demand.

Not a log analyser: lnav, logana, klogg and bvr already cover that well.

## Sources

- lnav performance: <https://docs.lnav.org/en/latest/performance.html>
- logana architecture: <https://docs.rs/crate/logana/0.1.0/source/ARCHITECTURE.md>
- bvr: <https://docs.rs/crate/bvr/0.0.4>
- Toolong: <https://pypi.org/project/toolong/1.0.1>
- klogg vs lnav: <https://www.saashub.com/compare-lnav-vs-klogg>
- large-text-viewer: <https://rust-digger.code-maven.com/crates/large-text-viewer>
- Microsoft Edit: <https://heise.de/-10450362>, <https://tech.slashdot.org/story/25/06/24/2039231>, <https://www.pcworld.com/article/2938023/a-new-default-text-editor-is-coming-soon-to-windows-11.html>
- Emacs VLF: <https://elpa.gnu.org/devel/vlf.html>
- Vim large files: <https://www.baeldung.com/linux/vim-large-files-performance>
- EmEditor vs UltraEdit: <https://www.techjockey.com/compare/emeditor-vs-ultraedit>
