"""Svuota (in pratica) la cache dei file di sistema: scrive e rilegge un file temporaneo di N GiB, poi lo cancella.
Uso: python flush_cache.py <percorso_temporaneo_su_un_altro_disco> <GiB>  (usare >= RAM del PC; qui 30 GiB con 24 GB di RAM)
Serve a misurare le letture "a freddo" senza privilegi di amministratore."""
import os, sys, time
p = sys.argv[1]; gb = int(sys.argv[2]); blk = (b'0123456789abcdef' * 65536)  # 1 MiB
t = time.time()
with open(p, 'wb') as f:
    for _ in range(gb * 1024): f.write(blk)
    f.flush(); os.fsync(f.fileno())
# rilegge per riempire la cache con questi dati
with open(p, 'rb') as f:
    while f.read(16 * 1024 * 1024): pass
print('flush', gb, 'GiB in', round(time.time() - t, 1), 's')
os.remove(p); print('rimosso')
