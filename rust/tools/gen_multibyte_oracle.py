#!/usr/bin/env python3
"""Seconda meta' dell'oracolo delle codifiche multibyte: incrocia le tabelle di .NET (prodotte da
gen_multibyte_oracle.ps1, una cartella con net_<nome>.txt) con i codec di Python e scrive crates/mrp-core/tests/data/
multibyte_oracle.txt con le sole sequenze su cui le DUE fonti concordano (e che danno UN solo carattere). Le altre
(varianti di Microsoft, estensioni HKSCS, JIS X 0212 che .NET non ha...) non entrano: restano garantite solo da
encoding_rs.

Per tenere il file piccolo le sequenze si raggruppano per "prefisso" (tutti i byte tranne l'ultimo): una riga per
prefisso con
    <nome> <prefisso in esadecimale, '-' se vuoto> <maschera di 256 bit: gli ultimi byte concordati> <hash FNV-1a 64>
L'hash passa, per ogni bit acceso in ordine crescente, l'ultimo byte e poi il punto di codice (u32 big endian).
Il test (tests/multibyte.rs) ricalcola maschera e hash dalla codifica sotto test. Per vedere QUALE sequenza differisce:
    python tools/gen_multibyte_oracle.py <cartella> --dump <nome> <prefisso>

Uso (da rust/):  python tools/gen_multibyte_oracle.py <cartella con i net_*.txt>
"""
import os
import sys

PY = {'shift-jis': 'cp932', 'euc-jp': 'euc_jp', 'gbk': 'gbk', 'gb18030': 'gb18030', 'big5': 'big5hkscs', 'euc-kr': 'cp949'}


def py_decode(codec, raw):
    try:
        s = raw.decode(codec)
    except UnicodeDecodeError:
        return None
    if len(s) != 1 or s == '�':
        return None
    return ord(s)


def fnv(h, byte):
    return ((h ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF


def righe_di(nome, net_dir):
    """{prefisso: {ultimo byte: punto di codice}} delle sequenze su cui .NET e Python concordano."""
    codec = PY[nome]
    gruppi = {}
    stat = [0, 0, 0]
    for riga in open(os.path.join(net_dir, 'net_%s.txt' % nome), encoding='utf-8'):
        h, cp = riga.split()
        raw = bytes.fromhex(h)
        p = py_decode(codec, raw)
        if '+' in cp or p is None:
            stat[1] += 1
        elif (len(raw) == 1 and 0x80 <= p <= 0x9F) or 0xE000 <= p <= 0xF8FF:
            # l'area privata (U+E000-U+F8FF) e i C1 dei byte singoli: Windows e Python vi mandano byte che WHATWG dichiara non
            # validi (Shift_JIS 0x80, 0xA0, 0xFD-0xFF) o mappa altrove (GB18030 0xA3A0 e' U+3000 per WHATWG, U+E5E5 per
            # Windows e Python); encoding_rs segue WHATWG: non sono divergenze che interessino
            stat[1] += 1
        elif p == int(cp, 16):
            stat[0] += 1
            gruppi.setdefault(raw[:-1].hex().upper(), {})[raw[-1]] = p
        else:
            stat[2] += 1
    return gruppi, stat


def main():
    net_dir = sys.argv[1]
    here = os.path.dirname(os.path.abspath(__file__))
    if len(sys.argv) > 2 and sys.argv[2] == '--dump':
        gruppi, _ = righe_di(sys.argv[3], net_dir)
        for ultimo, cp in sorted(gruppi.get(sys.argv[4] if sys.argv[4] != '-' else '', {}).items()):
            print('%s%02X -> U+%04X' % (sys.argv[4] if sys.argv[4] != '-' else '', ultimo, cp))
        return
    dest = os.path.join(here, '..', 'crates', 'mrp-core', 'tests', 'data', 'multibyte_oracle.txt')
    out = []
    for nome in PY:
        gruppi, stat = righe_di(nome, net_dir)
        print('%-10s concordano %6d, non usati (solo una fonte / piu' ' caratteri) %5d, diverse %5d' % (nome, *stat))
        for prefisso in sorted(gruppi, key=lambda p: (len(p), p)):
            ultimi = gruppi[prefisso]
            mask = 0
            h = 0xCBF29CE484222325
            for ultimo in sorted(ultimi):
                mask |= 1 << ultimo
                h = fnv(h, ultimo)
                for k in (24, 16, 8, 0):
                    h = fnv(h, (ultimi[ultimo] >> k) & 0xFF)
            out.append('%s %s %064x %016x' % (nome, prefisso or '-', mask, h))
    with open(dest, 'w', encoding='utf-8', newline='\n') as f:
        f.write('\n'.join(out) + '\n')
    print('scritto', os.path.normpath(dest), len(out), 'righe', os.path.getsize(dest) // 1024, 'KiB')


if __name__ == '__main__':
    main()
