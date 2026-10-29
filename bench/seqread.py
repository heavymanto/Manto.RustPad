"""Lettura sequenziale semplice (blocchi da 8 MiB, senza buffer): il limite del disco da confrontare con le misure.
Uso: python seqread.py <file>"""
import sys, time
p = sys.argv[1]; t = time.time(); n = 0
with open(p, 'rb', buffering=0) as f:
    while True:
        b = f.read(8 * 1024 * 1024)
        if not b: break
        n += len(b)
dt = time.time() - t
print(f'lettura sequenziale: {n} byte in {dt:.1f} s = {n/dt/1e6:.0f} MB/s')
