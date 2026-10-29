"""Misura il core di Manto.RustPad con lo stesso metodo usato per gli altri programmi.

Uso:  python -I -X utf8 core_bench.py <file> [--out risultati.jsonl]

Lancia `cargo run --release --example info` gia' compilato (rust/target/release/examples/info.exe),
campiona il processo ogni 50 ms (working set, memoria privata, byte letti da disco) e stampa una riga JSON.
Compila prima con:  cargo build --release -p mrp-core --example info
"""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gui_open import sample, k32   # stesse funzioni di campionamento dei programmi GUI

HERE = os.path.dirname(os.path.abspath(__file__))
EXE = os.path.join(HERE, '..', 'rust', 'target', 'release', 'examples', 'info.exe')

path = sys.argv[1]
out = sys.argv[sys.argv.index('--out') + 1] if '--out' in sys.argv else None
t0 = time.perf_counter()
p = subprocess.Popen([EXE, path], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
h = k32.OpenProcess(0x1F0FFF, False, p.pid)
peak_private = 0; last = None
while p.poll() is None:
    last = sample(h); peak_private = max(peak_private, last['private']); time.sleep(0.05)
elapsed = time.perf_counter() - t0
text = p.stdout.read()
res = dict(app='mrp-core info', file=os.path.basename(path), file_mib=round(os.path.getsize(path) / 1048576),
           elapsed_s=round(elapsed, 2), peak_private_mib=peak_private, peak_ws_mib=last['peak_ws'] if last else None,
           read_mib=last['read_mb'] if last else None, cpu_s=last['cpu_s'] if last else None, exit=p.returncode,
           output=text.strip().splitlines()[:12])
print(json.dumps(res, ensure_ascii=False))
if out:
    with open(out, 'a', encoding='utf-8') as f: f.write(json.dumps(res, ensure_ascii=False) + '\n')
