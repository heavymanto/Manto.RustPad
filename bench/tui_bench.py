"""Misura un editor TUI (default: Microsoft Edit) su un file grande, pilotandolo in un ConPTY.

Uso:  python -I -X utf8 tui_bench.py <file> [--edit <riga>] [--exe C:\\Windows\\System32\\edit.exe] [--out risultati.jsonl]

Misura: tempo fino al primo disegno, memoria dopo l'apertura, Ctrl+End, Ctrl+Home, "vai a riga" (Ctrl+G).
Con --edit <riga>: va a quella riga, digita XYZ, salva con Ctrl+S e verifica il file (ATTENZIONE: SCRIVE sul file:
usare sempre una COPIA; le righe del file di prova sono di 100 byte, quindi la riga N parte a (N-1)*100).
"""
import json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from conpty import Pty

path = sys.argv[1]
exe = sys.argv[sys.argv.index('--exe') + 1] if '--exe' in sys.argv else r'C:\Windows\System32\edit.exe'
out = sys.argv[sys.argv.index('--out') + 1] if '--out' in sys.argv else None
edit_line = int(sys.argv[sys.argv.index('--edit') + 1]) if '--edit' in sys.argv else None
name = os.path.basename(path).split('.')[0]
res = dict(app=os.path.basename(exe), file=os.path.basename(path), file_bytes=os.path.getsize(path))

p = Pty('"' + exe + '" "' + path + '"')
t_draw = p.wait_for(name, timeout=180)
p.wait_idle(0, quiet=1.0, timeout=180)
res['t_name_visible_s'] = None if t_draw is None else round(t_draw, 2)
res['after_open'] = p.mem(); res['alive'] = p.running()

def act(label, keys, quiet=0.8, timeout=180):
    m = p.mark(); p.send(keys); last, _ = p.wait_idle(m, quiet=quiet, timeout=timeout)
    return None if last is None else round(last, 2)

res['ctrl_end_s'] = act('end', '\x1b[1;5F')
res['ctrl_home_s'] = act('home', '\x1b[1;5H')
res['goto_line_s'] = act('goto', '\x07') and act('goto2', '30000000\r')
res['mem_after_nav'] = p.mem()

if edit_line:
    size0 = os.path.getsize(path)
    act('goto', '\x07'); act('line', f'{edit_line}\r'); act('type', 'XYZ')
    t0 = time.perf_counter(); mt0 = os.stat(path).st_mtime_ns; p.send('\x13')
    sizes = []; changed = None; stable = None; peak = 0
    while time.perf_counter() - t0 < 600:
        time.sleep(0.05); st = os.stat(path); sizes.append(st.st_size); peak = max(peak, p.mem()['private'])
        if st.st_mtime_ns != mt0 and changed is None: changed = time.perf_counter() - t0
        if changed is not None:
            if len(sizes) > 2 and sizes[-1] == sizes[-2]:
                stable = stable or time.perf_counter()
                if time.perf_counter() - stable > 1.5: break
            else: stable = None
    p.close(); time.sleep(0.5)
    size1 = os.stat(path).st_size
    with open(path, 'rb') as f:
        f.seek((edit_line - 1) * 100); head = f.read(24)
    res['edit'] = dict(line=edit_line, save_first_change_s=changed and round(changed, 2),
                       save_done_s=round((stable or time.perf_counter()) - t0, 2), private_peak_mb=peak,
                       size_before=size0, size_after=size1, delta_bytes=size1 - size0, expected_delta=3,
                       bytes_at_line=head.decode('latin1'), edit_applied=head.startswith(b'XYZ'))
else:
    p.close()
line = json.dumps(res, ensure_ascii=False)
print(line)
if out:
    with open(out, 'a', encoding='utf-8') as f: f.write(line + '\n')
