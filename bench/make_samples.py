"""Writes the sample logs in bench/samples/: one small, compressed file per scenario of mrp-loggen.

    cargo build --release -p mrp-loggen        (from rust/)
    python -I bench/make_samples.py            (from the repository root)

The files are 1 MiB of log each (about 150-250 KB gzipped), with a slow rate so that each one spans days, not minutes:
open one in mrp-tui or mrp-gui (after `gunzip`) and the timeline has something to show. The generator is deterministic, so
running this again gives the same logs. Bigger ones: `mrp-loggen --size 10G --scenario service big.log`.
"""
import gzip
import os
import subprocess
import sys

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(RAIZ, 'rust', 'target', 'release', 'mrp-loggen' + ('.exe' if os.name == 'nt' else ''))
DEST = os.path.join(RAIZ, 'bench', 'samples')

# ritmo (voci al secondo) di ogni scenario: lento, cosi' 1 MiB copre giorni; le raffiche di `bursts` sono ogni 12 ore
RITMO = {'bursts': 0.02}
PREDEFINITO = 0.05

if not os.path.exists(EXE):
    sys.exit('build first: cargo build --release -p mrp-loggen (from rust/); not found: ' + EXE)
os.makedirs(DEST, exist_ok=True)
elenco = subprocess.run([EXE, '--list'], capture_output=True, text=True, check=True).stdout.splitlines()
for riga in elenco:
    nome = riga.split()[0]
    dati = subprocess.run(
        [EXE, '--size', '1M', '--scenario', nome, '--rate', str(RITMO.get(nome, PREDEFINITO)), '-'],
        capture_output=True, check=True,
    ).stdout
    percorso = os.path.join(DEST, nome + '.log.gz')
    with open(percorso, 'wb') as f:
        # mtime = 0 e nessun nome: lo stesso log da' lo stesso file
        with gzip.GzipFile(filename='', mode='wb', fileobj=f, compresslevel=9, mtime=0) as z:
            z.write(dati)
    print('%-10s %8d bytes -> %7d gz' % (nome, len(dati), os.path.getsize(percorso)))
