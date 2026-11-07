# Manto.RustPad — Manual and test trace

> **Status: first draft.** Written so far: the product overview, the cards for opening and navigating, modes and saving, time and
> filters, and the "under the hood" notes for autosave, saving, the line index, time, encodings, search and caches. Still to do: the
> remaining cards (encodings and columns, tab details, settings, themes, help). Features are frozen for the first release: this
> document describes what the product does today, and it is the trace used for the UX tests.

## 0. How to read this document

Manto.RustPad is a viewer/editor for files that do not fit in RAM, with two faces on the same engine: **`mrp-tui`**, the editor
for the terminal (servers, SSH), and **`mrp-gui`**, the desktop window. Both draw the same text grid and share every key
except where a chapter says otherwise.

Every feature has a **card** with the same fields:

| Field | Meaning |
|---|---|
| **Entry** | How you reach it: key, menu, click. *Terminal* and *Window* when they differ. |
| **What it does** | The behaviour in plain words. |
| **Expected response** | What the interface must show or do. This is what a tester checks. |
| **Must not** | What must never happen (data loss, freezes, a wrong mode...). |
| **Limits and edge cases** | Big files, missing permissions, unsaved changes, narrow screens. |
| **Auto tests** | The automated tests that already cover it, or *none*. |
| **UX check** | A line to fill in during the UX tests: `OK`, `DEFECT` + note, or `NOTE`. |

How each statement was established is marked where it matters: *(observed)* = seen by driving the installed program;
*(code)* = read in the source; *(to verify by hand)* = not yet seen (for instance, anything only the window can show).

Keys are written for Windows, Linux and macOS alike (`Ctrl`, `Alt`). In the terminal, `Alt+letter` needs a terminal that sends
Alt as an escape prefix (most do).

---

## 1. The product in one page

**Three ways to use a file.** They exclude each other: exactly one is on.

| Mode | Keys | What it is |
|---|---|---|
| **VIEW** | `Alt+V` | Read only. The default. |
| **EDIT** | `Alt+E` | Change the file. Nothing touches the disk until you save. |
| **TAIL** | `Alt+T` | Read only, and the view follows the end of the file as it grows (`tail -f`). |

The current mode is always the last word of the status row (`VIEW`, `EDIT`, `TAIL`, with `COL` after it in column mode) and is
lit in the footer. Clicking the word, or `Alt+M`, opens a small menu of the three modes. In the window the toolbar has three
adjacent buttons, one lit.

**The terminal screen**, top to bottom:

1. *(only with more than one tab)* the tab bar;
2. the text, with a gutter of line numbers on the left;
3. the **status row**: position (`Ln 12  Col 3  byte 1234  7%`) or a message on the left; on the right the running job
   (`indexing 45%`), the file name (` *` after it when modified), the size, the encoding if it is not UTF-8, the line ending if it
   is not LF, and the mode;
4. the **footer**: `Alt+V View  Alt+E Edit  Alt+T Tail  F1 Help  F2 Save  F3 Find  F4 Repl  F5 Go ...  F10 Quit`. On a narrow
   terminal the less used F keys give way first (F6, F7, F8, F9, then F5); the three modes never do.

**Standard keys, the same in the terminal and in the window:** `F1` help, `F2` save, `F3` find (`Shift+F3` previous), `F4`
replace, `F5` go to, `F6` theme, `F7` language, `F8` column mode, `F9` encoding, `F10` quit, `Esc` closes a window.
`Ctrl+Z`/`Ctrl+Y` undo/redo, `Ctrl+C`/`X`/`V` clipboard, `Ctrl+A` select all, `Ctrl+O` open, `Ctrl+N` new, `Ctrl+W` close tab.

**The window** has a menu (File, Edit, View, Log, Help), a toolbar (New, Open, Save | Undo, Redo | Cut, Copy, Paste |
Find, Replace, Go to | Timeline, Time filter | VIEW, EDIT, TAIL, Column mode, Jump bar | Zoom), tabs, a scrollbar and the same
status row as the terminal.

---

## 2. Feature cards

### 2.1 Opening and navigating

#### N-01 Open a file from the command line
- **Entry:** `mrp-tui FILE` / `mrp-gui FILE`. With no file, the last session comes back (see C1); with none, an empty "Untitled 1".
- **What it does:** shows the first screen at once, whatever the size. The file is mapped, not read; line numbers are counted in the
  background (see C3).
- **Expected response:** first screen immediately, mode `VIEW`, position `Ln 1  Col 1  byte 0  0%`, file name and size on the right.
  While the index is being built: `indexing N%` on the right, `Ln ?` on the left, and a blank line-number gutter *(observed on a 6 GB file)*.
- **Must not:** load the whole file; freeze the screen while indexing; ask anything.
- **Limits and edge cases:** **on Windows, while a file is open here, other programs can append to it, rename it or delete it, but cannot truncate it** (opening it for writing with truncation, or shrinking it, fails with a system error: the file is memory-mapped). A service that restarts its log from zero will get that error while someone is viewing the log; appending, rotating by rename and deleting are fine (tested). A missing file gives an error and no window; an empty file opens with 0 lines; a file that is one single
  huge line still scrolls (rows are cut at 128 KiB); a file of 64 MiB or more shows a progress bar after 2 s (N-08).
- **Auto tests:** yes (`app_view`, `cli`, `pausa_indice`, `utf16_core`...).
- **UX check:** `[ ]`

#### N-02 Open another file, new document
- **Entry:** `Ctrl+O` (window: File > Open, system dialog; terminal: a prompt "path of the file to open", Enter OK, Esc cancels).
  `Ctrl+N` opens an empty document in a new tab, in EDIT mode.
- **What it does:** opens the file in a **new tab**, or brings forward the tab that already has it. New documents are called
  "Untitled 1", "Untitled 2"... (the smallest free number) and take the line ending of the system: `CRLF` on Windows, shown in the
  status row *(observed)*.
- **Expected response:** a new tab appears and is active; opening an already open file does not duplicate it.
- **Must not:** replace a tab that has unsaved changes.
- **Limits and edge cases:** opening while the active tab has unsaved changes keeps those changes (a new tab is used).
- **Auto tests:** yes (`workspace`, `untitled`, `menu`).
- **UX check:** `[ ]`

#### N-03 Move around
- **Entry:** arrows, `PgUp`/`PgDn`, `Home`/`End`, `Ctrl+Home`/`Ctrl+End`, mouse wheel, scrollbar (window).
- **What it does:** moves the cursor and the view; `Ctrl+Home`/`End` jump to the start/end of the file.
- **Expected response:** the position in the status row follows; the view scrolls when the cursor leaves the screen.
- **Must not:** read the whole file to move; stall on a very long line.
- **Limits and edge cases:** on a file still being indexed, jumps work by byte and percentage, not by line (N-04); jumping pauses
  the indexer's disk reads for 400 ms (C3).
- **Auto tests:** yes (`app_view`, `app_jump`, `app_mouse`).
- **UX check:** `[ ]`

#### N-04 Go to (`F5` / `Ctrl+G`)
- **Entry:** `F5` or `Ctrl+G`; window: Edit > Go to, or the toolbar button. A prompt: "line number, @byte offset, %percent or ~time".
- **What it does:** jumps to a line (`500`), a byte offset (`@1000000`), a percentage of the file (`%50`) or the first line at or
  after a time (`~2026-01-01 05:44`).
- **Expected response:** the cursor goes there and the target is shown near the middle of the screen *(observed: `500` -> `Ln 500`,
  `%50` -> `49%`, `@1000000` -> `byte 1000000`)*. `0` -> "not a valid position: 0"; a line past the end -> "line N is beyond the end (M lines)".
- **Must not:** move the cursor on an invalid target; land inside a multi-byte character (it snaps back to its start).
- **Limits and edge cases:** by line needs the index (while it is still being built you get a "wait" message and nothing moves);
  `@` and `%` need no index; `~time` needs a recognised time format (see C4).
- **Auto tests:** yes (`app_jump`, `time_dialogs`).
- **UX check:** `[ ]`

#### N-05 Jump pages and the jump bar
- **Entry:** `Alt+PgUp`/`Alt+PgDn` jump 10 pages (`Shift` x100, `Ctrl` x1000); `Ctrl+B` shows a bar with *Home*, *End* and page-step
  buttons (the window draws it as a strip of buttons with a gear that opens the settings of the steps).
- **What it does:** very long moves without reading the lines in between (the index is used for jumps of 5000 rows or more).
- **Expected response:** the view lands where the step says; the status row shows the new position.
- **Must not:** freeze on a huge file.
- **Auto tests:** yes (`jump_config`, `app_jump`, `pausa_indice`).
- **UX check:** `[ ]`

#### N-06 Line numbers and the line index
- **Entry:** `Alt+N` shows/hides line numbers; `Alt+X` turns the line index on/off (window: View menu).
- **What it does:** the index is what makes line numbers and "go to line" possible on a big file. Off = no counting and no numbers.
- **Expected response:** turning the index **off** first asks "TURN OFF THE LINE INDEX?", listing what is lost (line numbers, go to a line, the total of lines, fast page jumps, the line count of a selection) and what keeps working; Enter confirms and the numbers disappear (`Ln ?`), Esc cancels *(observed)*. Turning it on again restarts the count from zero without asking.
- **Must not:** change the file.
- **Auto tests:** yes (`line_numbers`, `pausa_indice`).
- **UX check:** `[ ]`

#### N-07 Tabs
- **Entry:** `Ctrl+PgDn`/`Ctrl+PgUp`, `Alt+←`/`Alt+→`, `Alt+1`..`9`, `Ctrl+W` closes; window: click on a tab.
- **What it does:** several documents open together, each with its own index, search, history and mode.
- **Expected response:** the active tab changes at once; closing a tab with unsaved changes asks first.
- **Must not:** close a modified tab silently when there is no session (with a session the changes are kept, see C1).
- **Auto tests:** yes (`workspace`, `chrome`).
- **UX check:** `[ ]`

#### N-08 Progress of long jobs on big files
- **Entry:** automatic, for indexing, search and saving.
- **What it does:** after 2 seconds of work, and only for a file of 64 MiB or more, a bar appears in the status row with the time left;
  on wider screens also the bytes done over the total and the speed. The estimate uses the last 3 seconds, so it follows the disk
  going from cache to real reads.
- **Expected response:** from 60 columns `indexing 61% #######.....`; from 90 also `0:01 left`; from 140 also
  `3.45 GiB / 5.59 GiB  1.67 GiB/s` *(observed)*. Below 2 s, or under 64 MiB, only the percentage. The bar is a rainbow
  (themes with colours) *(observed in the escape codes)*.
- **Must not:** block the screen; show an absurd estimate (it stays silent when the job is paused or has no speed yet).
- **Limits and edge cases:** the ETA is hidden above 99 hours.
- **Auto tests:** yes (`avanzamento`).
- **UX check:** `[ ]` — in the window: *(to verify by hand)* bar and detail beside the percentage.

### 2.2 Modes, editing and saving

#### M-01 The three modes
- **Entry:** `Alt+V` VIEW, `Alt+E` EDIT, `Alt+T` TAIL; `F4` is *not* a mode (it is Replace). Window: three toolbar buttons and the
  Edit menu entries with a tick.
- **What it does:** chooses what the file does. The keys *choose* a mode: pressed in the mode you are already in they do nothing
  (`Alt+V` is how you go back). In the window a click on the lit EDIT or TAIL button switches it off (back to VIEW).
- **Expected response:** the last word of the status row changes; the footer lights the current one; a message explains the change
  ("edit mode: the file is not touched until you save", "TAIL: following the end of the file", "view mode").
- **Must not:** be in two modes at once; start TAIL on a modified document (see M-08).
- **Auto tests:** yes (`modi`, `follow`, `commands`, `strumenti`).
- **UX check:** `[ ]`

#### M-02 The mode menu
- **Entry:** `Alt+M`, or a click on the mode word in the status row.
- **What it does:** a small menu with the three modes and what each one means; the current one is highlighted.
- **Expected response:** Up/Down move, Enter chooses, Esc closes.
- **Must not:** change the mode without Enter.
- **Auto tests:** yes (`modi`, `dialog_views`); the click in a real terminal is only tested at the app level *(to verify by hand)*.
- **UX check:** `[ ]`

#### M-03 Typing and editing
- **Entry:** in EDIT: characters, `Enter`, `Tab`, `Backspace`, `Delete`; selection with `Shift`+arrows or the mouse; `Ctrl+C`/`X`/`V`.
- **What it does:** changes the document in memory (see C2). A selection is replaced by what you type.
  Copy (`Ctrl+C`) also puts the text in the system clipboard (through OSC 52, so it works over SSH). Paste (`Ctrl+V`) uses the program's own
  clipboard; to paste text from another program use the terminal's own paste (`Ctrl+Shift+V` in Windows Terminal, which sends it as typed text).
- **Expected response:** the status row shows ` *` after the file name; the text changes at once.
- **Must not:** touch the file on disk before a save. In VIEW, typing is refused with the hint **"read-only: press Alt+E to edit"** *(observed)*.
- **Auto tests:** yes (`app_edit`, `app_select`, `app_multibyte`).
- **UX check:** `[ ]`

#### M-04 Undo and redo
- **Entry:** `Ctrl+Z`, `Ctrl+Y` (`Ctrl+Shift+Z`); window: Edit menu, toolbar.
- **What it does:** undoes/redoes the last step. A run of typing is one step; `Enter` closes a step; any other key or a click closes it.
- **Expected response:** the text goes back/forward by whole steps.
- **Must not:** work in VIEW (refused with the same hint as typing, "read-only: press Alt+E to edit" *(observed)*); survive a save (after a save the history is empty).
- **Limits and edge cases:** the history is not kept in the session: a restored tab can be saved but not undone (C1).
- **Auto tests:** yes (`app_edit`, `commands`).
- **UX check:** `[ ]`

#### M-05 Save and Save as
- **Entry:** `F2` / `Ctrl+S`; `Ctrl+Shift+S` Save as (asks the name, then the encoding). Both work in VIEW too when there are changes.
- **What it does:** writes the document to disk safely (see C2); the saved file becomes the document. Save as makes the document
  *be* the new file; the old one is not touched.
- **Expected response:** the message "saved: NAME" and no ` *`. On a big file a dialog with a progress bar, Esc cancels *(observed: `saved: ux_copy.log`)*.
- **Must not:** leave a half-written file, ever, whatever the interruption; overwrite a file another program has changed since you opened it.
- **Limits and edge cases:** nothing to save -> the message "nothing to save" *(observed)*; file changed on disk -> refused with the reason, edits stay;
  read-only folder or full disk -> an error, original intact, no temp file left.
- **Auto tests:** yes (`app_edit`, `save_encoding`, `session`).
- **UX check:** `[ ]`

#### M-06 Discard changes (`Ctrl+R`)
- **Entry:** `Ctrl+R` (or `Alt+R`); window: File > Revert.
- **What it does:** throws away the unsaved changes and shows the file as it is on disk. Asks once.
- **Expected response:** a question naming the file; Enter/S/Y confirms and the tab reopens in VIEW near the old position; Esc/N keeps the edits.
- **Must not:** touch the file on disk. With nothing to discard: the message **"no unsaved changes to discard"** and no dialog *(observed)*.
- **Auto tests:** yes (`revert`).
- **UX check:** `[ ]`

#### M-07 Quit with unsaved changes
- **Entry:** `F10` / `Ctrl+Q`; window: close the window.
- **What it does:** with a session (the normal case) it writes the session and exits **without asking**: the changes come back at the next start (C1).
  Without a session it asks, tab by tab.
- **Expected response (no session):** a window "UNSAVED CHANGES": `S` Save and quit, `D` Discard changes, `C` Cancel *(observed)*.
  "Save and quit" exits only if the save succeeded.
- **Must not:** lose changes silently.
- **Auto tests:** yes (`session`, `app_edit`).
- **UX check:** `[ ]`

#### M-08 Tail (follow the end)
- **Entry:** `Alt+T`; window: toolbar button / Edit menu.
- **What it does:** read only; the view goes to the end and follows the file as it grows; the index is extended over the new bytes, not rebuilt.
  Checks twice a second. Leaving the end of the file with the cursor stops it.
- **Expected response:** `TAIL` in the status row and "TAIL: following the end of the file".
- **Must not:** start on a document with unsaved changes: it stays where it is and says **"save or discard the changes (F2, Ctrl+R) before TAIL"**.
- **Limits and edge cases:** from EDIT without changes it switches straight to TAIL; a file that shrinks stops following with a message.
- **Auto tests:** yes (`follow`, `modi`).
- **UX check:** `[ ]`

### 2.3 Time, timeline and filters

#### T-01 Time format of the lines (`Alt+O`)
- **Entry:** `Alt+O`; automatic on first use. Window: Log menu / the Time window with calendar and "example" box *(to verify by hand)*.
- **What it does:** tells the program where the timestamp sits in each line. It recognises the format by itself from a sample of the
  head and tail of the file, once per tab. 19 formats are known (ISO with `.` or `,` milliseconds, `T` separator, `dd/MM/yyyy`,
  Apache `[01/Nov/2026:02:00:00 +0100]`, syslog `Nov  1 02:00:00`, time only, `epoch_s`, `epoch_ms`, date only). You can type
  your own (`yyyy MM dd HH mm ss SSS MMM epoch_s ...`), pick a known one with `Alt+N`/`Alt+P`, re-detect with `Alt+A`, or *teach by
  example* by selecting a piece of a line.
- **Expected response:** a window "TIME FORMAT" with the format, how many characters from the start to look within, the known
  formats with a sample, and a preview of the first lines as `text => 2026-01-01 00:00:28` *(observed)*. Everything that depends on
  time (status-row time, timeline, go to a time, time range, colours) follows the chosen format.
- **Must not:** read the whole file (detection uses at most 64 KiB from the head and 64 KiB from the tail); change the file.
  A wrong token (`yyyyy`) gives a "bad token" error and leaves the format unchanged.
- **Limits and edge cases:** UTF-16/UTF-32 files are never recognised, so they have no time features. Masks without a year (syslog)
  use the year 2000. Timestamps that go backwards break the assumption behind the searches (see C4).
- **Auto tests:** yes (`time_range`, `time_dialogs`, core `timemask*`).
- **UX check:** `[ ]`

#### T-02 Go to a time (`F5`, then `~time`)
- **Entry:** the go-to prompt with a `~` before the time: `~03:20`, `~2026-10-31 05:00:00`.
- **What it does:** jumps to the first line whose time is at or after the one you wrote (binary search, no scan).
- **Expected response:** the cursor and the view land there *(observed: `~03:20` on a log with times from 00:00 lands on the 03:22:55 line, `Ln 283`)*.
  A target before everything stays on the first line.
- **Must not:** read the file linearly; land on a line with a smaller time.
- **Limits and edge cases:** needs a recognised format and times that never go backwards; lines without a time are skipped.
- **Auto tests:** yes (`goto_time`).
- **UX check:** `[ ]`

#### T-03 Time range (`Alt+I`)
- **Entry:** `Alt+I`; window: Log > Time range / toolbar clock button (lit while a range is on).
- **What it does:** limits the whole view to the lines between two times. Empty "From" = from the start, empty "To" = to the end,
  and a time written to a unit covers the whole unit (`02:30` covers that minute). The range is just two byte offsets: removing it
  restores the whole file at once, and the dates you wrote are remembered.
- **Expected response:** a prompt "Dates like 2026-11-01 02:30, or only 02:30 (the day of the file). Empty = no limit.", with the
  first time of the file prefilled in "From". After Enter the view shows only that interval and the status row says
  `Time range: 2026-01-01 00:00:28 to 2026-01-01 00:10:00` *(observed)*.
- **Must not:** accept "from" later than "to" (an error that also tells the real first and last times of the file; nothing changes).
- **Limits and edge cases:** search finds only matches inside the range; a match outside counts as not found. The last time of a log is
  looked for at most 4096 rows back from the end.
- **Auto tests:** yes (`time_range`).
- **UX check:** `[ ]`

#### T-04 Timeline (`Alt+B`)
- **Entry:** `Alt+B`; window: toolbar clock button / Log > Timeline. In the window it turns itself on when a file with a recognised time is
  opened (once per tab, file up to 2 GiB, unmodified); in the terminal it never does.
- **What it does:** a bar next to the text that shows the whole file in time: one row per period (minute, 10 minutes, hour, day), the
  length of the bar is the volume of that period, and each bar is split by level (ERROR, WARN, INFO, DEBUG) in proportion to how many
  lines of each level start in it. A click or a drag jumps to that period.
- **Expected response:** a 25-column panel with a header (`~1 hour`, the `~` meaning the step is automatic, then the legend `E W I D`)
  and one row per period like `01-01 04:00 ▒▒▒▒▒` *(observed)*. A level that is present always gets at least one cell, so a single
  ERROR among thousands of INFO lines is still visible.
- **Must not:** read the whole file for each period (it gallops forward and bisects); block the screen (it is built in a background thread
  that opens its own copy of the file); work on a modified document (it says "unavailable").
- **Limits and edge cases:** needs at least 40 text columns; at most 20,000 periods (forcing 1 minute on a multi-day log gives
  "too many buckets" with the count); UTF-16/32 documents have no timeline.
- **Auto tests:** yes (`timeline_*`, core `timeline*`).
- **UX check:** `[ ]` — in the window: *(to verify by hand)* the drawn bar, click and drag.

#### T-05 Timeline step (`Alt+P`)
- **Entry:** `Alt+P`, or a click on the timeline header (only while it is on).
- **What it does:** chooses the size of the period.
- **Expected response:** a menu "TIMELINE: PERIOD": *Auto (chosen from the length of the log)*, 1 minute, 10 minutes, 1 hour, 1 day *(observed)*.
  Auto picks the longest of day / hour / 10 minutes that still gives at least 12 periods, else the minute.
- **Must not:** change the file or the range.
- **Auto tests:** yes (`timeline_tasti`).
- **UX check:** `[ ]`

#### T-06 Level filter (`Alt+S`)
- **Entry:** `Alt+S` (timeline on); also the level flags in the Filters window.
- **What it does:** opens a *filtered view* (see F-03) with only the lines of the levels you choose, keeping the time range.
- **Expected response:** a menu "TIMELINE: SHOW ONLY": *All the lines*, *Errors only*, *Errors and warnings*, *Errors, warnings and info* *(observed)*.
  The choice opens a new read-only tab named with the levels.
- **Must not:** change the original file or tab. The match is the whole level word, exact case.
- **Limits and edge cases:** refused for non-UTF-8 documents, with a message.
- **Auto tests:** yes (`timeline_levels_app`).
- **UX check:** `[ ]`

#### T-07 Log colours (`Alt+L`)
- **Entry:** `Alt+L`; window: Log > Log colours.
- **What it does:** colours the levels (ERROR, WARN, INFO, DEBUG and relatives), the dates with times and IPv4 addresses, in one
  linear pass per line, without regular expressions. DEBUG is blue in the Console, Dark and Light themes (grey on the two blue themes).
- **Expected response:** the colours appear at once and follow the theme; times are coloured only where the current format can read them.
- **Must not:** slow scrolling; colour inside a word (the level must be a whole word).
- **Auto tests:** yes (`logcolors`, core `loghl*`).
- **UX check:** `[ ]`

#### F-01 Find (`F3` / `Ctrl+F`)
- **Entry:** `Ctrl+F` opens the prompt (prefilled with the last text); `F3` repeats forwards, `Shift+F3` backwards (or opens the prompt if there is no search yet).
- **What it does:** searches text, a glob or a regular expression, with or without case; modes are cycled with `Alt+M` and `Alt+C` in the prompt.
  The search runs on the *edited* document, unsaved changes included, in a worker thread that works on a snapshot.
- **Expected response:** a prompt "text to search for (exact bytes)" with `mode: text | match case  (Alt+M, Alt+C)` *(observed)*; the cursor
  goes to the next match; past the end it wraps with a notice. On a big file a progress bar after 2 s and `Esc` cancels ("cancelled", cursor unmoved).
- **Must not:** freeze on a huge file; hang on any regular expression (the engine has no backtracking and a size limit of 8 MiB).
- **Limits and edge cases:** patterns match per line (never across a newline); glob, regex and ignore-case need a UTF-8 document
  (otherwise a message says so); plain text in a code page whose needle cannot be encoded is simply "not found"; matches in multibyte
  Asian encodings count only at the start of a character.
- **Auto tests:** yes (`dialog_search`, core `pattern`).
- **UX check:** `[ ]`

#### F-02 Replace (`F4` / `Ctrl+H`)
- **Entry:** `F4` or `Ctrl+H`; window: Edit > Replace / toolbar.
- **What it does:** replaces text, glob or regex matches in this tab or in all tabs. Two steps: the first Enter counts and previews
  ("N occurrences in M tabs"), the second applies. Escapes `\n \t \r \\` and groups `$1`, `${name}` in regex mode.
- **Expected response:** typing anything resets the preview; after applying, the tab is marked modified and **one** `Ctrl+Z` undoes the whole run.
- **Must not:** touch the files on disk; apply anything when cancelled with `Esc`, or when over 2,000,000 occurrences ("too many").
- **Limits and edge cases:** all-tabs skips the tabs that cannot take it (non-UTF-8 with a regex, for example) and says how many; tabs cannot be opened or closed while it runs.
- **Auto tests:** yes (`replace`).
- **UX check:** `[ ]`

#### F-03 Line filter (`Alt+G`)
- **Entry:** `Alt+G`; window: Filters window. In the prompt, `Alt+I` inverts (hide the lines that match).
- **What it does:** like `grep`: writes the matching lines into a temporary file and opens it as a new **read-only tab** named
  `[filter] TEXT`. Each line remembers its real position in the original.
- **Expected response:** the new tab with the matching lines and a message like `297 lines: Enter on a line goes to it in the file` *(observed)*;
  `Enter` on a line brings forward the original file (reopening it if closed) with the cursor on that exact line.
- **Must not:** modify the original. Editing in the view is refused ("a filtered view is read-only: edit the original file"). With no match:
  "no line matches", no tab and no temporary file left.
- **Limits and edge cases:** at most 20,000,000 lines per view (it stops and says it was cut); the temporary file `mrp-filter-<pid>-<n>.log` is in the
  system temporary folder and is deleted when the tab closes or the program exits.
- **Auto tests:** yes (`filter_view`, `filters_app`).
- **UX check:** `[ ]`

### 2.4 Encodings and line endings

#### E-01 Encoding: detection and change (`F9`)
- **Entry:** automatic on open; `F9` (window: View > Encoding…) opens the list of 64 encodings with the current one highlighted; on the command line `--encoding NAME`.
- **What it does:** the file's bytes are never changed: the encoding is only how they are read. Detection looks at the first 64 KiB: a BOM (UTF-32, UTF-16, UTF-8), UTF-16 without BOM (by where the zero bytes are), valid UTF-8, and otherwise falls back to windows-1252. `F9` re-reads the same bytes with another encoding on the spot.
- **Expected response:** the text re-reads in place and the status row names the encoding (nothing is shown for plain UTF-8). Observed on 28 test files (UTF-8 with and without BOM, UTF-16/32 both byte orders, 14 single-byte code pages, 7 DOS and Mac pages, Shift-JIS, EUC-JP, GBK, GB18030, Big5, EUC-KR): with `--encoding` all 28 read exactly; with detection, the Unicode ones with BOM, UTF-8, UTF-16 without BOM and windows-1252/ISO-8859-1 are right, and every other 8-bit or Asian file opens as windows-1252 (garbled but intact), and **choosing the right encoding in `F9` fixes the text in all 21 cases tried**.
- **Must not:** change the file; lose or hide a byte that is not valid (it shows as one replacement character per byte and is written back unchanged).
- **Limits and edge cases:** switching between UTF-16/32 and the other encodings is refused while the document is modified ("save before switching to/from UTF-16/32"). The status row does not say that the encoding was *guessed*.
- **Auto tests:** yes (`app_encoding`, `app_utf16`, `app_utf32`, `app_multibyte`, core encoding tests).
- **UX check:** `[ ]`

#### E-02 Save as with another encoding (`Ctrl+Shift+S`)
- **Entry:** `Ctrl+Shift+S`: first the path, then a list of encodings (the current one highlighted, `Tab` = write a BOM, `Enter` saves).
- **What it does:** writes the whole document (unsaved edits included) in the chosen encoding, block by block, to a temporary file renamed over the target.
- **Expected response:** "Saved NAME as ENCODING"; the document becomes the new file. Observed: 17 conversions from UTF-8 (UTF-16/32 both orders with and without BOM, ISO and Windows code pages, KOI8-R, CP866, Shift-JIS, EUC-JP, GBK, Big5, EUC-KR) gave **byte-exact files** with the right BOM and read back correctly; cp1251 → UTF-8 → cp1251 gave the identical file. A character the target cannot hold stops the save with a precise message ("Cannot write U+1F600 in windows-1252 (at byte 5): nothing was saved") and no file is created.
- **Must not:** write a partial file; touch the source.
- **Limits and edge cases:** the path field is pre-filled with the folder of the file: type only the new name (typing a full path appends to it and fails with the raw system error 123).
- **Auto tests:** yes (`save_encoding`).
- **UX check:** `[ ]`

#### E-03 Editing keeps the encoding and the line ending
- **Entry:** automatic.
- **What it does:** what you type is encoded in the file's own encoding; the BOM stays; the line ending (LF, CRLF, CR) is detected from the first 64 KiB and `Enter` writes the same style.
- **Expected response:** observed: typing `é`, `ж`, `あ`, `好`, `가` in windows-1252, windows-1251, UTF-16, Shift-JIS, GBK and EUC-KR files and saving gave exactly the right bytes; a BOM and invalid UTF-8 bytes are kept; typing a character the encoding cannot hold is refused at once ("cannot write '日' in windows-1252") and nothing is inserted.
- **Must not:** break a line ending.
- **Known defect (found 2026-11-07, not fixed):** in a **CRLF** file the cursor can sit between the `\r` and the `\n`, where `End`, the third `Right`, and `Shift+End` leave it. The status row shows the same column as after the last character (`Col 3`), but a few bytes further (`byte 3` instead of `byte 2`). Typing, `Enter`, `Backspace`, `Delete`, replacing a selection or cutting there leaves a lone `\r` or a bare `\n` in the file: for example `ab\r\ncd\r\n` + `End` + `X` saves `ab\rX\ncd\r\n`. Same in UTF-16 CRLF files. LF files are not affected.
- **Auto tests:** yes (`app_lineending`, `eol_option`, `app_bom`), none of which covers editing at the end of a CRLF line.
- **UX check:** `[ ]` — DEFECT (data integrity).

---

## 3. Under the hood

These chapters explain how the parts work, where they keep their data and what they guarantee. Constants are from the source
and are checked by tests where noted.

### C1. Autosave and session: how unsaved work survives

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

### C2. Editing and saving: piece table and safe write

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

### C3. Line index and navigation without loading the file

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

### C4. Time: formats, jump to a time, range, timeline

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

### C5. Encodings, line endings, column mode

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

### C6. Search, replace and the line filter

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

### C7. Caches: what is remembered, and when it is thrown away

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

---

## 4. UX test sheet

One row per card. `Terminal` is the result of the terminal passes of 2026-11-07 (about 130 scenarios driven on the installed program with simulated keys, then about 45 with a real console, keyboard and mouse); `Window` is the result of the window passes of the same day: screenshots produced by the developer hooks of `mrp-gui`, then the real window driven with keyboard and mouse. A defect that applies to the whole window: with `--theme light` and a file with recognised times, the timeline panel stays dark.

| ID | Feature | Terminal | Window | Auto tests |
|---|---|---|---|---|
| N-01 | Open from the command line | NOTE: cut footer under 40 columns; mixed-language error; `--help` lists 4 of 6 themes | OK | yes |
| N-02 | Open another file, new document | OK | NOTE: the native Open / Save as dialogs start in the working folder | yes |
| N-03 | Move around | OK (also with the real mouse: wheel, drag, jump-bar buttons) | OK (wheel, scrollbar drag, timeline click, mouse selection) | yes |
| N-04 | Go to (line, @byte, %percent, ~time) | NOTE: errors do not say what is valid | OK | yes |
| N-05 | Jump pages, jump bar | OK | OK | yes |
| N-06 | Line numbers, line index | DEFECT: dialog button says "Replace"; long note cut | DEFECT: `Alt+X`, shown in the menu, does nothing in the window | yes |
| N-07 | Tabs | DEFECT: closing a tab says "Save and quit"; clicking a tab works | OK | yes |
| N-08 | Progress of long jobs | OK (6 GB) | OK (5.6 GB: bar, bytes, speed, ETA) | yes |
| M-01 | The three modes | OK | OK | yes |
| M-02 | The mode menu | NOTE: the click opens the menu but the entries cannot be clicked | OK (click on the mode word) | yes |
| M-03 | Typing and editing | NOTE: `Ctrl+V` pastes only text copied inside the program ("clipboard is empty") | OK | yes |
| M-04 | Undo and redo | OK | OK (Ctrl+Z removes the whole typed word) | yes |
| M-05 | Save and Save as | DEFECT: Save as shows the internal Windows path prefix and a ~20-char field | OK: "saved: NAME", the title loses the `*`; NOTE: the dialog starts in the working folder | yes |
| M-06 | Discard changes | DEFECT: "This cannot be undone" is cut off | OK (full text, wrapped) | yes |
| M-07 | Quit with unsaved changes | OK | NOTE: closing a tab says "Save and quit" | yes |
| M-08 | Tail | OK | OK (live: appended lines are followed) | yes |
| T-01 | Time format of the lines | OK | NOTE: `Alt+O` does nothing in the window (the Filters window works) | yes |
| T-02 | Go to a time | NOTE: "no line with that time" on a file without times | not tested | yes |
| T-03 | Time range | DEFECT: typing appends to the prefilled "From"; garbage accepted | DEFECT: `Alt+I`, shown in the menu, does nothing; the window itself is fine | yes |
| T-04 | Timeline | NOTE: silent when too narrow; mentions a menu the terminal lacks; click on the panel works | DEFECT: `Alt+B`, shown in the menu, does nothing; click and drag work | yes |
| T-05 | Timeline step | NOTE: silent when the timeline is off | NOTE: `Alt+P` does nothing in the window | yes |
| T-06 | Level filter | NOTE: silent when the timeline is off | NOTE: `Alt+S` does nothing in the window | yes |
| T-07 | Log colours | NOTE: no feedback; colours not visible in the dump | OK | yes |
| F-01 | Find | NOTE: raw regex error text | NOTE: label says "(exact bytes)" | yes |
| F-02 | Replace | NOTE: switches to EDIT silently; hint cut | DEFECT: after the preview, Enter does not apply; only the button does | yes |
| F-03 | Line filter | NOTE: raw regex error text | OK | yes |
| E-01 | Encoding: detection and change | OK: 28/28 explicit, 21/21 fixed with F9; NOTE: a guess is not flagged | not tested | yes |
| E-02 | Save as with another encoding | OK: 17/17 byte-exact; NOTE: pre-filled path with `\\?\` | not tested | yes |
| E-03 | Editing keeps encoding and line ending | DEFECT: CRLF broken by typing at the end of a line | not tested | yes |
