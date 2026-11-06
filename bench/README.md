# bench/ — reproducible measurements

Everything in [`docs/BENCHMARKS.md`](../docs/BENCHMARKS.md) was produced with these scripts. Windows only (Win32 + ConPTY),
Python 3.12, **no third-party packages**.

| Script | What it does |
|---|---|
| `make_samples.py` | Writes `bench/samples/<scenario>.log.gz`: 1 MiB of every synthetic log of `mrp-loggen`, gzipped (about 2.8 MB in all), ready to open. |
| `gen_testfile.py <out> <reps>` | Writes a test file of `reps × 100 MB`: 1,000,000-line blocks, every line exactly 100 bytes (`L0000000 lorem…\n`), so line *N* starts at byte `(N-1)*100`. |
| `conpty.py` | Tiny ConPTY driver: starts a console program in a pseudo-console, sends keys, records output with timestamps, reads working set / private bytes. |
| `tui_bench.py <file> [--edit <line>]` | Drives a terminal editor (default `edit.exe`): open time, memory, `Ctrl+End`, `Ctrl+Home`, go-to-line; with `--edit` types `XYZ`, saves and verifies the file. |
| `mrp_bench.py <file> [--edit] [--line N]` | Same idea as `tui_bench.py` for `mrp-tui`: open, memory, `Ctrl+End`/`Ctrl+Home`, go to line once the line index is ready, and with `--edit` types `XYZ`, saves and verifies the file (**writes to the file**: use a copy). |
| `gui_open.py <npp\|ltf\|ue> <file> [--end]` | Launches a GUI program on the file, samples the process every 100 ms (working set, private bytes, bytes read/written, CPU), answers known "large file" dialogs, reads the status bar of Notepad++, optionally sends `Ctrl+End`. Kills only the process it started. Program paths default to the standard install folders; override them with `MRP_BENCH_NPP`, `MRP_BENCH_LTF`, `MRP_BENCH_UE`. |
| `core_bench.py <file>` | Runs the compiled `info` example of `mrp-core` on a file and records time, peak private bytes, working set, bytes read. Build first: `cargo build --release -p mrp-core --example info`. |
| `flush_cache.py <tmp path> <GiB>` | Empties the OS file cache without admin rights (writes and re-reads a temporary file larger than RAM, then deletes it). Use before any "cold" measurement. |
| `seqread.py <file>` | Plain sequential read in 8 MiB blocks: the disk's ceiling to compare against. |
| `summarize.py` | Prints the tables used in `docs/BENCHMARKS.md` from `results/*.jsonl`. |

## Synthetic logs (`mrp-loggen`)

`gen_testfile.py` writes a perfectly regular file (100-byte lines). For something that looks like a real log -- times that
advance with a day/night rhythm, five levels, error storms, stack traces, rare messages -- use the generator in
`rust/crates/mrp-loggen` (any platform, no dependencies, constant memory, 250-500 MB/s):

```
cargo build --release -p mrp-loggen                         # from rust/
mrp-loggen --size 1G big.log                                # 1 GiB, scenario "service"
mrp-loggen --size 100G --scenario bursts --seed 7 huge.log  # 100 GiB of quiet days and violent error storms
mrp-loggen --size 5G --rate 2 --start 2024-03-01 slow.log   # 2 entries/s on average: the file spans months
mrp-loggen --size 1G - | gzip > big.log.gz                  # to the standard output
mrp-loggen --list                                           # the scenarios
```

The same options give the **same bytes** on every machine and every version (a test pins them), so a number measured on
one generated file can be reproduced by anyone. A gzipped service log is about 1/6 of its size, so a 100 GB file does not
fit on GitHub (100 MB per file, 2 GB per release asset): publish the options, not the file.

| Scenario | What it is for |
|---|---|
| `service` | The default: `2026-11-04 10:17:11.756 +01:00 [INF] ...`, five levels, rhythm of the day and the week, stack traces, rare messages |
| `bursts` | Quiet, with a few long and violent error storms (the timeline shows them at a glance) |
| `syslog` | `Nov  4 10:17:11 host-07 sshd[1234]: ...`: no year, no levels, several hosts |
| `access` | Web server access log with `[04/Nov/2026:10:17:11 +0100]` and 5xx storms |
| `json` | One JSON object per line, `"ts"` in ISO 8601 and `"level"` |
| `epoch` | Lines that start with a Unix time in milliseconds |
| `unordered` | 3% of the lines carry a time in the past (late arrivals): the times are not sorted |
| `longlines` | 1.5% of the lines have a payload from 2 KB up to 200 KB on the same line |
| `crlf` | Windows line endings |
| `unicode` | Italian, German, Russian, Japanese, Chinese, accents and emoji |
| `report` | Log lines mixed with ASCII tables (many lines without a time), like a backtest report |
| `text` | Plain prose: no dates, no levels (the timeline has to say there are no times) |

`bench/samples/` has 1 MiB of each (`gunzip` and open): `python -I bench/make_samples.py` writes them again.

## Reproduce

```powershell
python -I -X utf8 bench\gen_testfile.py D:\bench\big6g.txt 60                 # 6.0 GB, ~40 s
python -I -X utf8 bench\tui_bench.py D:\bench\big6g.txt --out bench\results\tui.jsonl
python -I -X utf8 bench\gui_open.py ltf D:\bench\big6g.txt --end --out bench\results\gui.jsonl
```

`--edit` **writes to the file**: always pass a copy.

## Caveats you should know before trusting a number

- "Open time" is *time until the program stops producing output / CPU activity*, not a perceived-latency measurement.
- GUI runs send `Ctrl+End` to the real foreground window; the script checks that the window is in front before sending.
- Memory figures are Windows *working set* and *private bytes* sampled from outside the process; a memory-mapped file
  shows up in the working set only for the pages actually touched.
- One machine (24 GB RAM, Windows 11), one run per cell unless stated. Treat differences below ~20 % as noise.
- Test files are synthetic and perfectly regular (100-byte lines). Real logs have variable lines.

## Result files (`results/`)

| File | Contents |
|---|---|
| `tui.jsonl` | Microsoft Edit: open, navigation, and the two edit+save runs (1 GB ok; 6 GB truncated) |
| `gui.jsonl` | Final run of Notepad++, LTFViewer, UltraEdit on 1/3/6 GB, dialogs answered |
| `core_before_c3.jsonl`, `core_after_c3.jsonl`, `core_after_c3_readat.jsonl`, `core_after_gate.jsonl` | `mrp-core` index runs, cold (after `flush_cache.py`), in the order of the history table in BENCHMARKS.md |
| `core_final_cold.jsonl`, `core_final_warm.jsonl` | The current code after the review fixes: cold 38.2 s, warm 1.3 s on the 6 GB file |
| `seqread_cold.json` | Plain sequential read of the 6 GB file, cold: 40.3 s (149 MB/s) |
| `gui_repeat.jsonl` | Notepad++ 6 GB repeated (warm cache) |
| `gui_run1_senza_risposta_ai_dialoghi.jsonl` | First run, before the script answered dialogs — kept because it holds the Notepad++ `Ctrl+End` timings |

UltraEdit leaves its temporary copy (`<name>.tx0`) in `%TEMP%` when the process is killed; delete it after a run.
