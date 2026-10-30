"""Misura `mrp` (il TUI di Manto.RustPad) su un file grande, nel terminale vero (ConPTY): apertura, memoria, Ctrl+End,
Ctrl+Home, "vai a riga" una volta pronto l'indice, e con --edit anche modifica + salvataggio con verifica del file.

Uso:  python -I -X utf8 mrp_bench.py <file> [--edit] [--line 30000000] [--exe <mrp.exe>] [--out risultati.jsonl]

ATTENZIONE: con --edit SCRIVE sul file (+3 byte e un file temporaneo grande quanto lui nella stessa cartella):
usare una COPIA di prova. Le righe del file di prova sono di 100 byte (vedi gen_testfile.py).
Il "vai a riga" prima che l'indice sia pronto viene rifiutato dall'editor ("still building"): lo script riprova ogni secondo
e riporta dopo quanti secondi dall'avvio e' riuscito.
"""
import json, os, sys, time
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from conpty import Pty


def opt(nome, default=None):
    return sys.argv[sys.argv.index(nome) + 1] if nome in sys.argv else default


path = sys.argv[1]
exe = opt('--exe', os.path.join(HERE, '..', 'rust', 'target', 'release', 'mrp.exe'))
out = opt('--out')
line = int(opt('--line', '30000000'))
do_edit = '--edit' in sys.argv
name = os.path.basename(path).split('.')[0]
os.environ['MRP_NO_PROFILE'] = '1'   # non toccare il profilo dell'utente

res = dict(app='mrp.exe', file=os.path.basename(path), file_bytes=os.path.getsize(path))
p = Pty('"' + exe + '" "' + path + '"', cols=120, rows=30)
try:
    t0 = time.perf_counter()
    t_draw = p.wait_for(name, timeout=180)
    res['t_name_visible_s'] = None if t_draw is None else round(t_draw, 2)
    res['after_open'] = p.mem()

    def act(keys, quiet=0.8, timeout=180):
        m = p.mark(); p.send(keys); last, _ = p.wait_idle(m, quiet=quiet, timeout=timeout)
        return None if last is None else round(last, 2)

    res['ctrl_end_s'] = act('\x1b[1;5F')
    res['ctrl_home_s'] = act('\x1b[1;5H')
    # vai a riga: riprova finche' l'indice e' pronto (la riga cercata compare nel gutter)
    arrivato = None
    for _ in range(300):
        m = p.mark()
        p.send('\x07'); time.sleep(0.3)
        p.send(f'{line}\r')
        # Arrivato = l'editor NON risponde "still building" (l'indice non e' pronto). Non si cerca il numero di riga:
        # il prompt "vai a" lo mostra mentre si digita (falso "arrivato"), e il terminale manda solo le celle cambiate,
        # quindi "Ln 30000000" nella riga di stato non compare mai tutto di fila nel flusso.
        if p.wait_for('still building', since=m, timeout=2.5) is None:
            arrivato = time.perf_counter() - t0
            break
        time.sleep(1.0)
    res['goto_ok_after_start_s'] = None if arrivato is None else round(arrivato, 1)
    res['mem_after_goto'] = p.mem()
    if do_edit and arrivato is not None:
        size0 = os.path.getsize(path)
        p.send('\x1bOS'); time.sleep(0.5)      # F4
        p.send('XYZ'); time.sleep(0.5)
        t1 = time.perf_counter(); mt0 = os.stat(path).st_mtime_ns
        p.send('\x13')                          # Ctrl+S
        peak = 0; changed = None; stable = None; sizes = []
        while time.perf_counter() - t1 < 600:
            time.sleep(0.1)
            try:
                st = os.stat(path)
            except OSError:
                continue
            sizes.append(st.st_size); peak = max(peak, p.mem()['private'])
            if st.st_mtime_ns != mt0 and changed is None:
                changed = time.perf_counter() - t1
            if changed is not None:
                if len(sizes) > 2 and sizes[-1] == sizes[-2]:
                    stable = stable or time.perf_counter()
                    if time.perf_counter() - stable > 1.5:
                        break
                else:
                    stable = None
        res['edit'] = dict(line=line, save_first_change_s=changed and round(changed, 2),
                           save_done_s=round((stable or time.perf_counter()) - t1, 2), private_peak_mb=peak,
                           mem_after_save=p.mem(), size_before=size0)
finally:
    p.close()
    time.sleep(0.5)
if do_edit and 'edit' in res:
    size1 = os.path.getsize(path)
    with open(path, 'rb') as f:
        f.seek((line - 1) * 100); head = f.read(24)
    res['edit'].update(size_after=size1, delta_bytes=size1 - res['edit']['size_before'], expected_delta=3,
                       bytes_at_line=head.decode('latin1'), edit_applied=head.startswith(b'XYZ'))
txt = json.dumps(res, ensure_ascii=False)
print(txt)
if out:
    with open(out, 'a', encoding='utf-8') as f:
        f.write(txt + '\n')
