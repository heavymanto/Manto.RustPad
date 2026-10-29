"""Legge bench/results/*.jsonl e stampa le tabelle usate in docs/BENCHMARKS.md (memoria in MiB, tempi in secondi)."""
import json, os, sys
here = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'results')
MIB = 1048576

def rows(name):
    p = os.path.join(here, name)
    return [json.loads(l) for l in open(p, encoding='utf-8')] if os.path.exists(p) else []

def size_mib(r): return r.get('file_bytes', r.get('file_mb', 0) * 10**6 if 'file_mb' in r else 0) / MIB

print('## TUI (Microsoft Edit) — tui.jsonl')
print('| file | file MiB | name visible s | WS MiB | private MiB | WS/file | Ctrl+End s | Ctrl+Home s | go-to-line s |')
for r in rows('tui.jsonl'):
    if 'edit' in r: continue
    f = r['file_bytes'] / MIB; a = r['after_open']
    print(f"| {r['file']} | {f:.0f} | {r['t_name_visible_s']} | {a['ws']} | {a['private']} | {a['ws']/f:.2f} | {r['ctrl_end_s']} | {r['ctrl_home_s']} | {r['goto_line_s']} |")
print('\n### edit + save')
for r in rows('tui.jsonl'):
    if 'edit' in r:
        e = r['edit']
        print(f"- {r['file']}: size {e['size_before']} -> {e['size_after']} (delta {e['delta_bytes']}, expected +3), edit applied={e['edit_applied']}, save done {e['save_done_s']} s, private peak {e['private_peak_mb']} MiB")

print('\n## GUI — gui.jsonl')
print('| app | file | UI/title s | settled s | WS MiB | private MiB | peak WS MiB | read MiB | written MiB | Ctrl+End s | extra read MiB |')
for r in rows('gui.jsonl'):
    a = r['after_open']; ce = r.get('ctrl_end')
    ces = ce['settle_s'] if isinstance(ce, dict) else 'n/a'; cer = ce['extra_read_mb'] if isinstance(ce, dict) else 'n/a'
    print(f"| {r['app']} | {r['file']} | {r['t_title_s']} | {r['t_since_launch_s']} | {a['ws']} | {a['private']} | {a['peak_ws']} | {a['read_mb']} | {a['write_mb']} | {ces} | {cer} |")
    if r.get('statusbar'): print(f"|   | status bar: {r['statusbar'][1]} | | | | | | | | | |")
print('\n## GUI repeat (Notepad++ 6 GB) — gui_repeat.jsonl')
for r in rows('gui_repeat.jsonl'):
    print(f"- {r['file']}: title {r['t_title_s']} s, settled {r['t_since_launch_s']} s, WS {r['after_open']['ws']} MiB")
print('\n## GUI first run (dialogs not answered) — gui_run1_senza_risposta_ai_dialoghi.jsonl')
for r in rows('gui_run1_senza_risposta_ai_dialoghi.jsonl'):
    ce = r.get('ctrl_end'); a = r['after_open']
    print(f"- {r['app']} {r['file']}: title {r['t_title_s']} s, settled {r['t_since_launch_s']} s, read {a['read_mb']} MiB, End {ce['settle_s'] if isinstance(ce, dict) else ce}")
