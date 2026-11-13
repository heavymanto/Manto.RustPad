# Manto.RustPad — Architecture

**English** · [Italiano](ARCHITECTURE.it.md)

How Manto.RustPad works inside: the choices, the mechanisms, the limits. The constants come from the source and are checked by tests where noted. For how to use the program, the manual is [docs/MANUAL.md](docs/MANUAL.md).

---
## Autosave and session: how unsaved work survives

**What it is.** A small folder of files that remembers your open tabs and, for each tab with unsaved changes, those changes. It is
what lets you press `F10` without a question and find everything again at the next start.

**What is stored, and what is not.** Per tab: the file path, the cursor, the mode, the encoding name and — only if modified — the
edits themselves: the buffer of inserted bytes and the list of pieces that says how they interleave with the original. **The
original file is never copied**: a 100 GB log with three edits has a session of a few kilobytes. The undo history is not stored.

**Where.**

| System | Folder |
|---|---|
| Windows | `%APPDATA%\mrp\session\` |
| Linux / macOS | `~/.config/mrp/session/` |
| Portable (a `lang` folder next to the programs) | `<program folder>/session/` |

The terminal and the window use the same folder (and the same lock). Contents: `manifest.txt` (a text list: `mrp-session 1`,
`active N`, one `tab NAME` per tab), `tab-0.mrps`, `tab-1.mrps`... (binary, little endian, magic `MRPS`, version 1, closed by a 64-bit
FNV-1a checksum), `lock`, `untitled/` (the empty scratch files that hold "Untitled N" documents), `stale/` (what could not be
restored).

**When it is written.** While you work, at most once every **5 seconds**, checked once a second, and only if something changed
(an edit, a cursor move, a tab opened or closed, another tab in front). Every file goes to a temporary name and is renamed, so an
interrupted write leaves the previous session intact. A hard crash can therefore lose up to about 6 seconds of work. On quit, the
session is written at once. While a document is being saved, the session write is skipped and retried a moment later.

**When it comes back.** Starting with no file: every tab returns, edits included, with the same tab in front. Starting with
files: those open first and the old tabs *with unsaved changes* are added behind them (a clean tab of the same file is replaced by
the restored one). The status row says "restored N tab(s) with unsaved changes".

**What can go wrong.**

| Situation | What happens |
|---|---|
| The file changed on disk since | The tab opens clean, in VIEW, a warning names the file, the saved edits go to `stale/`. The check is length + modification time (nanoseconds); content is not hashed. |
| The file was deleted | The tab is skipped; with changes, the copy goes to `stale/` and a warning names it; a clean tab disappears silently. |
| A tab file is corrupt | Checksum or decoding fails: that tab is skipped, its file goes to `stale/`, the status row says so. |
| `manifest.txt` is corrupt | The whole session is ignored and "Untitled 1" opens. No crash. |
| Disk full / no permission | The periodic write retries later; on quit, if the session cannot be written, the editor asks tab by tab. |
| Two instances at once | The second cannot take the lock and runs without a session (its tabs are not protected). |

Only the first warning reaches the status row; with several, the rest are visible inside `stale/`. `stale/` is never emptied by the program.
Deleting the whole folder only forgets tabs, cursors and unsaved changes; user files are never touched.

**Worth knowing.** Unnamed documents survive restarts too (they sit on scratch files in `untitled/`; saving one under a real name
turns it into a normal tab and removes the scratch file). A user file that happens to live in a folder called `untitled` is never
mistaken for a scratch file. Developer runs with `--dump`, or with `MRP_NO_PROFILE=1`, keep no session.

**Tests that cover it:** `mrp-tui/tests/session.rs`, `untitled.rs`.

## Editing and saving: piece table and safe write

**Editing without copying.** A document is a read-only memory map of the file plus a **piece table**: an ordered list of pieces, each
pointing either into the original file or into a small append-only buffer that collects everything you type. Memory depends on the
size of your edits, not on the size of the file. The table is kept in blocks of up to 512 pieces, so an edit touches only the blocks
it crosses. Neighbouring pieces that become contiguous are merged.

**Undo.** Each effective edit is recorded as a tiny splice (which pieces were replaced by which). Undo replays the splices
backwards, redo forwards; no copy of text is made, so undo costs almost nothing even on a huge file. The "modified" flag is a version number that never repeats: undoing back to the saved state clears it.

**Saving.** The original is never written in place. A save:

1. refuses if the file on disk no longer has the length and modification time recorded at open (someone else changed it);
2. creates a temporary file **in the same folder**, named `.<name>.mrp-tmp-<pid>-<n>`;
3. copies the pieces in order, in **4 MiB** blocks (also the granularity of the progress bar and of cancellation);
4. flushes the temporary file to disk, copies the original's permissions (not the owner);
5. renames it over the original — the one atomic step. On Windows, renaming over a mapped file works while truncating it does not, which is why the save is a rename;
6. reopens the new file, collapses the table to a single piece, clears the undo history, and rebuilds the line index in the background.

The save runs in a worker thread that takes the document with it and **always gives it back**, even on error: a failed save never
loses your edits. `Esc` in the saving dialog sets a flag checked before every block and before the rename; after the rename the save
is committed.

**What can go wrong.**

| Situation | What happens |
|---|---|
| The file changed or vanished since open | "the file on disk changed after opening": nothing written, edits stay. |
| Unwritable folder, full disk, refused rename | An error with the system message; the temporary file is deleted; the original is untouched; the document stays modified. |
| Cancelled | The temporary file is deleted, "save cancelled", the document is as it was. |
| The file is shortened by someone else *during* the copy | The save fails with an I/O error; original intact. |
| Antivirus keeps the temporary file locked | The temporary file may be left behind; the original is safe. |
| Rename done but reopening fails | The data is safe on disk, but the tab becomes "closed" and reports the error (rare). |

A modified document is not followed in TAIL mode (it would mix two contents). Saving under a different encoding goes through a
conversion step with its own temporary file (`<name>.<pid>.<n>.mrp-conv`) in the target folder.

**Tests that cover it:** `mrp-core` document tests, `mrp-tui/tests/app_edit.rs`, `revert.rs`, `save_encoding.rs`.

## Line index and navigation without loading the file

**The idea.** Opening is instant because nothing is read up front: the file is memory-mapped and the screen asks only for the bytes
it shows. Line numbers need the index, which is built in the background.

**A sparse index.** It does not store every line. It remembers the byte offset of one line start every **256 lines**, plus one per
**4 MiB** block. Any question — how many lines in total, which line holds this byte, where does line N start — starts at the
nearest remembered point and counts newlines only in the short gap that remains. A huge log costs megabytes of index, not an
entry per line, and a file that is one single line is still answered with bounded work.

**How it is built.** A background thread opens its **own** read-only handle on the file. A pool of workers (one per CPU thread)
takes 4 MiB blocks in order, each read in **256 KiB** segments into a reused buffer — explicit positioned reads, not page faults,
so the process memory does not grow with the file — and a sequential merge turns per-block counts into global line numbers. On
files of **64 MiB or more** a prefetch thread reads ahead in 8 MiB steps, at most 256 MiB ahead of the workers, so the disk sees
contiguous requests.

**It gets out of your way.** When you jump (scrollbar drag, go to, `Alt+PgUp/PgDn`, `Ctrl+Home/End`) the indexer pauses its disk
reads and resumes by itself **400 ms** after the last jump; arrows and typing do not pause it (a test on an 8 GiB file checks this).
Exit, turning the index off, or a new job cancels it between blocks.

**Until it is ready.** Line numbers are unknown: the gutter is blank, the status row shows `Ln ?`, and go to a *line* says to wait;
go to by byte (`@`) or percentage (`%`) works at once. Without the index, movement walks **visual rows**: a row is a real line or a
128 KiB piece of a line that is too long, so a 1 GB single line never blocks the screen.

**When it restarts.** After every successful save (the file changed), when the newline shape changes (for instance a new
encoding) and when you turn the index back on. In TAIL mode a grown file *extends* the index over the new bytes instead of rebuilding it.
The newline shape (one byte, `CR`, or an aligned 2- or 4-byte unit for UTF-16/32) is detected from the first 64 KiB.

**What can go wrong.** (On Windows the memory mapping also prevents other programs from truncating the file while it is open here; they can append, rename and delete.) A file that cannot be opened by the background job gives "index failed" with the system error and no
numbers, but everything else keeps working. A file shortened while indexing makes the affected block be redone through the
mapping. A file truncated by another process under a live mapping can raise an access error from the system: a known and accepted
risk of memory mapping.

**Tests that cover it:** `mrp-core/tests` (index and encodings), `mrp-tui/tests/pausa_indice.rs`, `avanzamento.rs`.

## Time: formats, jump to a time, range, timeline

**The idea.** A log is almost always sorted by time and the time is at the head of each line. So the program never needs an index of
times: it can find any time with a binary search over the bytes of the file, reading only the first few hundred bytes of a handful
of lines. This is why the time features work on a file of any size and never read it whole.

**Time mask.** A *mask* is a pattern that says where the timestamp sits in a line (`yyyy-MM-dd HH:mm:ss.SSS`, `HH:mm:ss`,
`epoch_ms`, syslog, Apache...). It is matched only within the first characters of the line (0 for most formats, 40 for the Apache
one, adjustable). The value read is a number of milliseconds since 1970-01-01, negative before that, computed with a hand-written
Gregorian calendar: no timezone and no date library, so dates before 1970 and leap years work. Epoch values with 9–10 digits are
seconds, 12–13 are milliseconds. A leap second `:60` reads as 59.5 s; a mask without a year uses the year 2000.

**Detection.** Once per tab, the first time anything asks for it, the program takes a sample — up to 1000 non-empty lines from the
first 64 KiB and up to 1000 from the last 64 KiB — and tries the 19 known formats. The one that reads most lines wins, if enough
agree: a full date-and-time format needs 3 matching lines (or all of them), a weak one (time only, date only, epoch) needs half of the
non-empty lines, so a stray 10-digit number is not mistaken for an epoch. The format you chose last is tried first. UTF-16/UTF-32
documents are never recognised.

**Go to a time and time range.** Both bisect the byte range: take the middle, move to the start of its line, look at the first line with a time from there (at most 4096
rows ahead, reading at most 512 bytes of each), and narrow the interval. Lines without a time (a header, a stack trace) are skipped.
This assumes the times never go backwards; with a log that does, the searches can land on the wrong line. The time range is nothing
more than two byte offsets, so switching it off is instant.

**Timeline.** A bar with one row per period. Instead of scanning, the program finds each period boundary by jumping forward from the
previous one with doubling steps (galloping, starting at 4096 bytes) and then bisecting the interval it found. Cost is proportional to the
number of periods (at most 20,000), not to the number of lines. The step is 1 minute, 10 minutes, 1 hour or 1 day; *Auto* takes the
longest that still gives at least 12 periods (two weeks of log gives days, one day gives hours). A second pass, in its own thread and
reading 1 MiB pieces, counts for each period how many lines *start* with each level; a line's level is its first level word, found by
the same scanner that colours the line. The pass waits (20 ms cycles) while the file is still being indexed, so the two do not fight
for the disk. The 12 cells of each bar are split among the levels in proportion, but a level that is present never drops to zero.

**Threads and cancelling.** The ticks are built in a worker thread that opens *its own* handle on the file; the screen polls the
result. Changing the step, the range or the file length, or turning the timeline off, cancels the running jobs and discards partial
results (no half bar). Go to a time and the time range run on the main thread and cannot be interrupted, which is fine because they are
a few dozen small reads.

**What is remembered.** The format you chose becomes the default tried first on the next files; up to 8 rules learned *from an
example* are kept and offered again where they read; both live in the settings file. Each tab has its own mask and its own range.

**What can go wrong.** Timestamps going backwards (see above); a log whose last 4096 rows have no time reports last = first; a
modified or nameless document has no timeline ("unavailable"); a bad interval is refused with the real first and last times of the
file; UTF-16/32 has no time features at all.

**Tests that cover it:** core `timemask*`, `timesearch*`, `timekey*`, `timeline*`, `loghl*`; terminal `goto_time`, `time_*`, `timeline_*`.

## Encodings, line endings, column mode

**The idea.** A document is always a sequence of bytes; the encoding is only the lens that says how those bytes read as characters
(display, cursor, column widths) and how a typed character becomes bytes. Position, line counts and searches stay aligned to the
unit: 1 byte for UTF-8, code pages and the Asian multibyte encodings, 2 for UTF-16, 4 for UTF-32.

**64 encodings.** UTF-8, UTF-16 LE/BE, UTF-32 LE/BE, 53 single-byte code pages (windows-125x, ISO-8859, IBM/DOS pages, KOI8, five Mac pages,
ASCII) and 6 Asian multibyte encodings (Shift_JIS, EUC-JP, GBK, GB18030, Big5, EUC-KR). Names are case-insensitive, `_` equals `-`, with
aliases (`cp1252`, `latin1`, `437`, `cp932`...); a bare `utf-16` is refused as ambiguous. Stateful encodings (ISO-2022-JP, HZ) are not supported.

**Detection.** Without `--encoding`, the first 64 KiB decide, in this order: a UTF-32 BOM, a UTF-16 BOM, then (with at least 8 bytes) which half of each unit holds
the zeros, then any NUL byte means "binary" and UTF-8, then UTF-8 validity, else windows-1252. UTF-32 without BOM is never guessed.
Detection is a guess: **F9** re-reads the same bytes with another encoding on the spot, without touching the file. A byte that is not a
valid character shows as one U+FFFD character per byte, so nothing is hidden.

**The BOM.** It stays in the bytes and survives saves; the editor steers around it (typing at the start lands after it, Backspace stops
at it, Delete skips it).

**Line endings.** LF, CRLF or lone CR is detected from the same sample (lone CR only if single-byte and the sample has CRs but no LF; CRLF
if CRLFs outnumber bare LFs). Enter writes exactly that style, so a CRLF file never becomes mixed. `--eol` forces what Enter writes
without rewriting existing lines. New documents take the system style (CRLF on Windows).

**Changing the encoding.** F9 on a document whose newline shape would change (UTF-16/32 versus the rest) is refused while the document is
modified or has an undo history; otherwise the index is rebuilt in the background and the selection and the search highlight are dropped.

**Save as with conversion.** *Save as* asks for the target encoding and a BOM box. The same encoding and BOM is a plain save; anything else
converts the whole current content, unsaved edits included, in a worker thread, in 1 MiB blocks, into a temporary file
(`<name>.<pid>.<n>.mrp-conv`) that is renamed over the target. If a character cannot be written in the target encoding, nothing is renamed: the
temporary file is deleted, the target is untouched, the cursor jumps to that character and the message names it (U+XXXX) with its offset.

**Column mode (F8).** `Shift`+arrows or `Alt`+drag draw a rectangle over visual rows. Typing, Tab, Backspace and Delete act on every row of the block that
reaches the column; copy joins the rows with the document's newline; paste goes row by row; Enter drops the block. A character belongs to
the block if it *starts* inside the columns; the hidden `\r` and the `\n` never do. Every column action is one undo step. Limits: 200,000 rows,
64 MiB of copied text, a virtual column ceiling of 4096. A paste that needs more rows than the document has is refused whole.

**Tests that cover it:** `app_bom`, `app_lineending`, `eol_option`, `save_encoding`, `app_column`, core encoding tests.

## Search, replace and the line filter

**Searching the edited document.** A search does not run on the file: it takes a *snapshot* of the edited document (a shared
read of the mapped file plus a copy of the piece table) and runs on a worker thread, in 1 MiB blocks with overlap, checking the cancel flag before every block.
The snapshot stays valid even if you edit or save while it runs. Plain case-sensitive text is searched as exact bytes in the document's own encoding,
so it works in every encoding; glob, regex and ignore-case are compiled with the `regex` crate (which has no backtracking, so no pattern can hang the search,
and a compiled-size limit of 8 MiB) and work on UTF-8 documents only. Patterns match per line, like `grep`. Lines longer than 16 MiB are read in pieces
(a match across a cut can be missed). Empty matches are discarded.

**While a job runs** the program ignores every key but `Esc`, which cancels it.

**Replace** is two-phase: the first Enter counts (preview), the second applies. All the changes of one run are one undo step. Over 2,000,000
occurrences is refused ("too many"). In all-tabs mode the workspace runs one job per tab and totals the counts, the tabs with matches and the tabs skipped.

**The line filter** writes the matching lines, byte for byte, to a temporary file in the system temporary folder (`mrp-filter-<pid>-<n>.log`),
opens it as a read-only tab and keeps, beside it, the real offset of each line in the original (8 bytes per line, at most 20,000,000 lines). That
is why `Enter` on a line lands exactly on that line of the original even if the original was closed in the meantime. The temporary file is deleted when its
tab closes or the program exits; a filter that finds nothing, fails or is cancelled leaves no file behind.

**The same search settings** (text, mode, case) are shared by Find, Filter and Replace, and the prompts are prefilled with the last text.

**Tests that cover it:** core `pattern`, `docsearch`; terminal `dialog_search`, `replace`, `filter_view`, `filters_app`.

## Caches: what is remembered, and when it is thrown away

**The principle.** Nothing is persisted: every cache lives in RAM inside the process and disappears when it exits (the only things
written to disk are the saved document, the settings and the session). The big one is not ours: the file is memory-mapped, so its
pages are the operating system's file cache, shared and evictable. Long sequential passes (the line index, the save) use explicit
positioned reads instead of page faults because, on a spinning disk, a measured big sequential read ran at 149 MB/s against 28–39 MB/s
for faults.

**Why edits stay cheap.** The edit table is split into blocks of at most 512 pieces, and every block carries a *label* from a
process-wide counter that changes whenever its pieces change and is never reused: equal labels always mean equal content. That is what
lets the program remember the line counts of blocks it has already seen, and recount only the blocks you actually touched. A test
asserts that one keystroke recounts a handful of blocks, and none while nothing changes.

**The line index is of the original file.** It stays valid across edits: the piece table bridges the difference. So you can type in a
100 GB log and line numbers stay right without re-indexing.

| Cache | Contains | Thrown away when | Weight |
|---|---|---|---|
| Line index | sparse line starts of the original file (one per 256 lines, plus one per 4 MiB) | a save, an encoding change that changes the newline shape, the index turned off, a new index installed; *extended* in place in TAIL mode | 16 B per 256 lines + 16 B per 4 MiB: about 60 MB for a billion lines of 100 bytes |
| Newline count per piece | `\n` in each whole piece | with the index; emptied past 2 × pieces + 1024 | ~32 B per entry |
| Newline count per block | `\n` in a whole block, by label | with the index; pruned to live labels past 2 × blocks + 1024 | 16 B per entry |
| Prefix table | start offset and newlines before each block | **any edit, undo or redo**; rebuilt on first use, in time proportional to the blocks touched | 16 B per block |
| Timeline periods | boundaries of the periods (time, byte offset) | length, range or step change; timeline off; modified document | at most 20,000 × 16 B = 320 KB |
| Timeline levels | counts of the four levels per period | new periods arrive; timeline off | at most about 640 KB |
| Time format | the recognised format, per tab | recognised once; replaced by a manual choice (which also clears the range) | one pattern |
| Time dialog helpers | match counts, suggestions, the first and last time of the file | each time the dialog opens | a few hundred bytes |
| Preview | up to 3 (line, time) pairs | when pattern, distance or document length change | 3 rows |
| Drawn rows | the last screenful of text and its offsets | replaced at every draw; reused while a save holds the document | a screen |
| Progress samples | (time, bytes) of the running job | job changes or restarts; only the last 3 s kept | a few samples |

**Reading the table.** Everything that depends on the *edited* text (the prefix table) is rebuilt after every edit, but only for the
blocks that changed. Everything that depends on the *original file* (the index and the count caches) is thrown away only when the file
itself changes: a save, a new encoding shape, or the index being turned off. Pruning is by size only: if the piece cache is emptied, the
next rebuild recounts everything once.

**Worth knowing.** Switching the timeline off throws its periods away; switching it on recalculates the same periods. A log with a
long header without timestamps still gets its format recognised, because the sample includes the tail. The timeline preference is the
only "remembered" item of this chapter that reaches the settings file.

**Tests that cover it:** core `linechunks` (recounts per edit), terminal `timeline_tasti`, `time_format`.
