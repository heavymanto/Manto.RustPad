"""Writes the sample logs in bench/samples/: one small, compressed file per scenario of mrp-loggen.

    cargo build --release -p mrp-loggen        (from rust/)
    python -I bench/make_samples.py            (from the repository root)
    python -I bench/make_samples.py --plain D:/temp/test_logs    (also the plain files, to open them at once)

The files are 1 MiB of log each (about 150-250 KB gzipped), with a slow rate so that each one spans days, not minutes:
open one in mrp-tui or mrp-gui (after `gunzip`) and the timeline has something to show. The generator is deterministic, so
running this again gives the same logs. You do not need Python to get the plain files: `mrp-loggen --all DIR` writes them
all (this script only adds the gzip). Bigger ones: `mrp-loggen --size 10G --scenario service big.log`.
"""
import gzip
import os
import subprocess
import sys
import tempfile

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(RAIZ, 'rust', 'target', 'release', 'mrp-loggen' + ('.exe' if os.name == 'nt' else ''))
DEST = os.path.join(RAIZ, 'bench', 'samples')

if not os.path.exists(EXE):
    sys.exit('build first: cargo build --release -p mrp-loggen (from rust/); not found: ' + EXE)
os.makedirs(DEST, exist_ok=True)
SEMPLICI = sys.argv[sys.argv.index('--plain') + 1] if '--plain' in sys.argv else None

# i file semplici li scrive il generatore stesso (`--all`: il ritmo di ogni scenario sta li', in un posto solo)
with tempfile.TemporaryDirectory() as tmp:
    cartella = SEMPLICI or tmp
    subprocess.run([EXE, '--all', '--force', cartella], capture_output=True, check=True)
    for nome in sorted(os.listdir(cartella)):
        if not nome.endswith('.log'):
            continue
        with open(os.path.join(cartella, nome), 'rb') as f:
            dati = f.read()
        percorso = os.path.join(DEST, nome + '.gz')
        with open(percorso, 'wb') as f:
            # mtime = 0 e nessun nome: lo stesso log da' lo stesso file
            with gzip.GzipFile(filename='', mode='wb', fileobj=f, compresslevel=9, mtime=0) as z:
                z.write(dati)
        print('%-14s %8d bytes -> %7d gz' % (nome, len(dati), os.path.getsize(percorso)))
