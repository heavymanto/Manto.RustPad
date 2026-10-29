# Benchmarks: what existing tools do with multi-gigabyte files

*Measured 2026-10-28/29. Scripts and raw data: [`bench/`](../bench). Every number below is reproducible with those scripts;
every statement that is **not** a measurement is labelled *inference* or *not measured*.*

## Summary

On a 6.0 GB, 60-million-line text file:

| | Memory used | Open | Loses data? | Disk side effects |
|---|---|---|---|---|
| **Microsoft Edit** 1.2.1 (TUI, editor) | **4,100 MiB** (hard cap at 4 GiB) | 2.8 s (up to 10 s cold) | **Yes — silently truncates at 4 GiB; saving wrote a 4,294,967,296-byte file, destroying 1.7 GB** | none |
| **Notepad++** 8.8.1 (GUI, editor) | **6,288 MiB** (1.10 × file) | 7 s warm, 26–30 s cold; warning dialog above 2 GB | No — all 6,000,000,000 bytes / 60,000,001 lines loaded | none |
| **LTFViewer** 5.2u (GUI, viewer) | **43 MiB** | UI in 0.5 s; **full scan keeps running ~86 s** | n/a (read-only) | none |
| **UltraEdit-32** 10.20.3 (GUI, editor) | **19 MiB** | 7.5 s; **writes a full temporary copy** (5.7 GB) | not tested | **+1 × file size in `%TEMP%`**; `Ctrl+End` re-reads the whole file (21.6 s) |
| **Manto.RustPad core** (library + test program, *not an editor yet*) | **19–28 MiB private** | line index of the whole file: **38 s cold** (disk speed), 1.3 s warm | No — 60,000,000 lines exact, offsets beyond 4 GiB tested | none |

Take-aways:

1. **Two designs exist today and both have a cost.** Load everything (Edit, Notepad++): fast and simple, memory = file size,
   unusable above RAM. Stream from disk (LTFViewer, UltraEdit): tiny memory, but the cost moves to a full pass over the
   file at open (LTFViewer) or to a full temporary copy plus a full re-read for every jump to the end (UltraEdit).
2. **A tool that is both light on memory and cheap to navigate is not among the four.**
3. **Silent data loss is real** in a mainstream, recently shipped editor (Edit), on a file that merely exceeds 4 GiB.

## Test setup

| | |
|---|---|
| Machine | AMD Ryzen 7 3700X (16 threads), 24 GB RAM, Windows 11 Pro build 26100 |
| Test files | on **D: = 7200 rpm HDD** (Seagate ST1000DM010); UltraEdit's temp copies go to C: (NVMe/SSD) |
| Test data | synthetic: every line exactly **100 bytes** (`L0000000 lorem ipsum…\n`), 1,000,000-line blocks repeated. Sizes: 100 MB, 1 GB, 3 GB, 6 GB (decimal: 954 / 2,861 / 5,722 MiB). Line *N* starts at byte `(N-1)*100`, which lets us verify edits exactly. |
| Programs | Microsoft Edit 1.2.1, Notepad++ 8.8.1 (`-multiInst -nosession -noPlugin`), LTFViewer 5.2u, UltraEdit-32 10.20.3 (old, 32-bit, unregistered) |
| Memory | Windows *working set* and *private bytes* of the process, sampled from outside every 100 ms. **All memory figures are MiB.** |
| "Settled" | wall-clock until the process's CPU time and disk-read counter stop changing for 1.5 s |
| Dialogs | answered like a user would (Notepad++ "large file": Yes; UltraEdit "not DOS format": No); recorded in the raw data |
| Terminal editor | driven inside a ConPTY pseudo-console with real key sequences (`Ctrl+End`, `Ctrl+G`, `Ctrl+S`…) |

## Results

### Memory (MiB)

| File | Edit (WS) | Notepad++ (WS / private) | LTFViewer (WS / private) | UltraEdit (WS / private) |
|---|---|---|---|---|
| 1 GB (954 MiB) | 958 | 1,088 / 1,123 | 20 / 9 | 21 / 14 |
| 3 GB (2,861 MiB) | 2,865 | 3,171 / 3,221 | 30 / 19 | 21 / 14 |
| 6 GB (5,722 MiB) | **4,100 (capped)** | 6,288 / 6,343 (peak 6,542) | 43 / 32 | 19 / 14 |

Edit and Notepad++ scale 1 : 1 with the file (Notepad++ ≈ 1.10 ×). LTFViewer grows slowly (≈ 5 MiB per GiB of file — *inference:* its
line/offset index). UltraEdit stays flat.

### Open and navigation

| | 1 GB | 3 GB | 6 GB | Notes |
|---|---|---|---|---|
| Edit — file name visible | 0.5 s | 1.5 s | 2.8 s (10.4 s on an earlier cold run) | `Ctrl+End`/`Ctrl+Home`/go-to-line ≤ 0.3 s at every size (all in RAM) |
| Notepad++ — window titled | 6.0 s | 3.6 s | 7.1 s warm / 25.9–29.9 s cold | cold ≈ HDD speed (~200 MB/s); `Ctrl+End` ≈ 0.1–1.3 s, no extra reads (earlier runs) |
| LTFViewer — UI shown | 0.5 s | 0.5 s | 0.5 s | **then reads the whole file once**; settles at 16.5 / 44.6 / 85.5 s; CPU time ≈ elapsed (≈ 83 s CPU for 5.8 GB, i.e. CPU-bound at ~70 MB/s); afterwards `Ctrl+End` 0.2 s, no re-read |
| UltraEdit — title shown | 2.2 s | 4.0 s | 7.5 s | settles at 10.5 / 12.1 / 15.6 s; `Ctrl+End` 3.7 / 10.9 / **21.6 s, re-reading the entire file each time** |

Cold/warm matters a lot: the same Notepad++ 6 GB open took 7 s with the file in the OS cache and 26–30 s without (the first
matrix run, with unanswered dialogs, also gave 9.7 s). Single runs; treat differences under ~20 % — and any cold-cache
number — as indicative.

Not measured: whether LTFViewer is *usable during* its ~86 s scan (we expect so, since the UI is up at 0.5 s); search speed
in any tool; cold-cache behaviour of Edit and UltraEdit.

### Editing and saving

| Test | Result |
|---|---|
| Edit, 1 GB: go to line 5,000,000, type `XYZ`, `Ctrl+S` | Correct: file +3 bytes, `XYZ` at byte 499,999,900; save finished in **6.0 s**; memory unchanged (957 MiB) |
| Edit, 6 GB (copy): same | **File became exactly 4,294,967,296 bytes (2³²). 1,705,032,704 bytes lost. The edit was not applied.** Save took 31 s. No warning was visible on the first screen after opening. Reproduced twice. |
| Notepad++ 6 GB | Status bar after load: `lungh.: 6.000.000.000 linee: 60.000.001` — the whole file is held (edit/save not tested) |
| LTFViewer | read-only by design |
| UltraEdit | edit/save not tested |

On the 6 GB file Edit's last visible line was **42,949,640** instead of 60,000,000, consistent with a 32-bit offset limit
(*inference*; we did not read the source).

### Disk side effects

UltraEdit wrote **954 / 2,861 / 5,722 MiB** to disk while opening 1 / 3 / 6 GB files (a full copy named like `big6g.tx0` in
`%TEMP%`) and read the file once. The copy was still there after the process was killed; we deleted it by hand. The other
three tools wrote nothing.

## Manto.RustPad core — first measurements

Code: [`rust/crates/mrp-core`](../rust/crates/mrp-core) (`FileSource`, `LineIndex`, `find_next`/`find_prev`). Same machine, same
6.0 GB file, measured with [`bench/core_bench.py`](../bench/core_bench.py). "Index" means: count every line and keep a sparse
set of checkpoints (one per 256 lines per 4 MiB block) so that any line number can be resolved to a byte offset later.

| Measurement | Result |
|---|---|
| Lines found | **60,000,000 — exact** (also checked against a naive oracle on small files, with edge cases: empty file, CRLF, no final newline) |
| Position of the last line | byte 5,999,999,900 — beyond 2³² (the `ignored` test `big_file_beyond_4gib` checks it, including a search at the end) |
| Memory | **19–28 MiB private bytes** peak. The working set reaches ~5.7 GiB because it counts the file-backed pages of the system cache that the mapping touches; those are shared and evictable, not heap |
| Index size | 234,604 checkpoints ≈ 3.7 MB for 6 GB |
| Cold cache (file on HDD, cache flushed) | **38.1 s = 150 MiB/s**, versus 40.3 s (149 MB/s) for a plain sequential read of the same file — i.e. *at the disk's limit* |
| Warm cache (file in RAM) | 1.3 s (4.3 GB/s) in the final version; 0.44 s (13 GB/s) in the first, fully parallel version |

How the cold time got there (same file, cache flushed before each run):

| Version | Cold index time | Throughput |
|---|---|---|
| 1. Blocks scanned in parallel straight from the memory mapping | 145 s | 39 MB/s |
| 2. Same, but blocks taken in ascending order and pages "touched" ahead | 205 s | 28 MB/s |
| 3. A prefetch thread reads sequential 8 MiB blocks with explicit reads | 92 s | 64 MB/s |
| 4. Workers also **wait** for the prefetch to cover their block | **38 s** | **150 MB/s** |

What we learned: on a spinning disk, **page faults on a memory mapping are 4–5× slower than one large explicit read, even when
sequential**, and any second reader (a worker faulting on a page the prefetcher has not reached) breaks the sequential
pattern. The fix was to make the prefetch thread the only thing that touches the disk. The price is that, when the file is
already cached, the single-threaded prefetch copy limits the index to ~4 GB/s (1.3 s) instead of ~13 GB/s.

### Saving (first measurement)

`Document::save` writes the edited document to a temporary file next to the original (4 MiB blocks, explicit reads, `fsync`),
then renames it over the original. Test `big_file_save_beyond_4gib`, release build, same 6.0 GB file on the HDD: a 7-byte insert at
byte 4,500,000,000 and a 100-byte delete near the start. Result: file of 5,999,999,907 bytes (6,000,000,000 + 7 − 100),
first 100 bytes, the bytes around the insert and the last 100 bytes verified, no temporary file left. The whole test
took **116 s** — it reads and writes ~6 GB on the *same* disk, so the two streams compete for it. One run, file freshly
copied (partly in cache), the figure includes opening and the checks. It is **not** a clean throughput number.

### The terminal editor `mrp` on the same file (first measurement)

[`bench/mrp_bench.py`](../bench/mrp_bench.py) drives the release build of `mrp` in a ConPTY, the same way `tui_bench.py` drives
Microsoft Edit: open, `Ctrl+End`, `Ctrl+Home`, go to line 30,000,000 (retried every second until the line index is ready,
because `mrp` refuses a line jump while the index is still building), then `F4`, type `XYZ` at that line, `Ctrl+S`, and check
the file. 6.0 GB file, same HDD. The raw line of the last run is in
[`bench/results/tui_mrp.jsonl`](../bench/results/tui_mrp.jsonl); the table also quotes two earlier runs of the same steps
(`tui_bench.py` for open and navigation, a throw-away script for edit and save) whose raw output is not in the repository.

| Measurement | Result |
|---|---|
| First screen with the file name | 0.07–0.88 s (three runs) |
| Private memory | **13–31 MiB** after opening, **19–20 MiB** after the jump, **34–35 MiB** peak during the save. The working set peaks at ~5.7 GiB for the same reason as in the core table (file-backed pages touched by the background index) |
| Line numbers usable (jump to line 30,000,000 accepted) | 7.0 s and 9.0 s after start (partly cached file); until then the viewer works, the line jump says "still building" |
| Jump to line 30,000,000 | lands on byte 2,999,999,900, as computed by hand; the text on screen is the expected line |
| `Ctrl+End` | 0.39 s in one run, **4.06 s** in another (while the index was still reading the same disk): the figure is "time until the screen stops changing", not perceived latency |
| Edit and save (`XYZ` at line 30,000,000) | file 6,000,000,000 → 6,000,000,003 bytes, `XYZ` found at byte 2,999,999,900, the rest of the line intact; **91 s** and **98 s** in two runs, which is the cost of reading and writing 6 GB on one HDD |
| Search (not measured with the script) | a `--dump` run that searched for a string that is *not* in the file (a full pass plus the wrap-around) took 24 s in total, including the ~7 s the dump waits for the index. Rough, one run |

Not measured yet: a clean save throughput (and on SSD), the cost of many edits (the piece list is a plain vector, O(pieces)
per edit), SSD behaviour, Linux/macOS, the UI with a real keyboard on a real terminal (only the ConPTY driver).

## What this means for Manto.RustPad

These are **targets** derived from the tables above. The first three are now met for the line index (previous section); the rest are still open:

| Goal | Evidence it is reachable | Gap to close |
|---|---|---|
| Resident memory independent of file size, tens of MiB | LTFViewer 43 MiB, UltraEdit 19 MiB on 6 GB | none — proven possible |
| UI interactive immediately, no blocking pass | LTFViewer 0.5 s | LTFViewer still needs ~86 s of CPU-bound scanning |
| Whole-file pass limited by **disk**, not CPU | LTFViewer is CPU-bound at ~70 MB/s; this HDD delivers ~200 MB/s cold; an SSD far more | *Inference:* a SIMD newline scan, parallel by block, should saturate the disk |
| Exact jumps without re-reading the file | LTFViewer `Ctrl+End` 0.2 s *after* scanning; UltraEdit re-reads 5.7 GB every time | build the index once, in the background, and persist it |
| Never truncate; refuse loudly if a limit exists | Edit silently loses 1.7 GB | use 64-bit offsets everywhere; tests on > 4 GiB files from day one |
| No hidden copy of the file | UltraEdit writes 1 × file at open | edits as overlay (piece table); write only at save, to a temp file + rename |
| Save time ≈ one sequential write | Edit 6 s per GB (in-RAM buffer, rewrites everything) | streaming save; progress and cancel |

## Limitations (read before quoting)

- One machine, one disk type for the test files (HDD), **one run per cell** unless the text says otherwise. Cold and
  warm cache differ by ×3–4.
- Synthetic, perfectly regular data. Real files have variable line lengths and non-ASCII text, which changes indexing cost.
- "Settled" is a proxy for "ready", not a measure of perceived latency.
- UltraEdit-32 10.20.3 is an old, unregistered, 32-bit version; the current UltraEdit may behave differently. Newer Notepad++
  or Edit builds may too.
- Keyboard events for GUI programs go to the real foreground window; where the script could not bring the window forward it
  skipped the key and says so (`gui.jsonl`: `ctrl_end` field). Notepad++ `Ctrl+End` numbers come from earlier runs kept in
  `results/gui_run1_senza_risposta_ai_dialoghi.jsonl`.
- EmEditor, Sublime Text, VS Code, Vim, Emacs+VLF, lnav and logana were **not** benchmarked; see [MARKET.md](MARKET.md).

## Reproduce

See [`bench/README.md`](../bench/README.md). Raw data: [`bench/results/`](../bench/results). `python bench/summarize.py`
prints the tables above from the JSON files.
