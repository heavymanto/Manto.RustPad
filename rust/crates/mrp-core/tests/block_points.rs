//! I punti di blocco dell'indice righe: `newlines_before` deve dare lo stesso risultato di un conteggio ingenuo per OGNI
//! posizione, anche con righe lunghissime (o senza a-capo) dove i punti ogni `every` righe sono lontani, e il tratto da
//! contare non puo' piu' dipendere dalla dimensione del file. Scritti da ARCH: l'oracolo e' un conteggio dei byte di
//! a-capo sul `Vec<u8>` di partenza, mai l'indice.

use mrp_core::newline::Newline;
use mrp_core::{BuildOptions, FileSource, LineIndex};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(data: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_bp_{}_{}.bin", std::process::id(), n));
    std::fs::write(&p, data).unwrap();
    p
}

fn flags() -> (AtomicBool, AtomicU64) {
    (AtomicBool::new(false), AtomicU64::new(0))
}

/// Gli a-capo (unita' intere e allineate) nei primi `offset` byte, contati a mano.
fn ingenuo(dati: &[u8], offset: usize, forma: Newline) -> u64 {
    match forma {
        Newline::Byte => dati[..offset].iter().filter(|&&b| b == 0x0A).count() as u64,
        Newline::Le16 => dati[..offset - offset % 2]
            .chunks(2)
            .filter(|c| c == &[0x0A, 0x00])
            .count() as u64,
        _ => unreachable!(),
    }
}

/// Dati di prova per la forma a byte: righe corte, vuote, lunghe piu' di un blocco, un a-capo proprio sul confine dei
/// blocchi piccoli, una coda senza a-capo.
fn dati_byte() -> Vec<Vec<u8>> {
    let mut tutti = Vec::new();
    // niente a-capo: una riga sola lunga
    tutti.push(vec![b'a'; 300]);
    // solo a-capo
    tutti.push(vec![b'\n'; 50]);
    // righe da 7 byte + a-capo: l'a-capo cade sempre a fine di un blocco da 8
    let mut v = Vec::new();
    for _ in 0..40 {
        v.extend_from_slice(b"abcdefg\n");
    }
    tutti.push(v);
    // un impasto: righe lunghe 1000, vuote, corte, e una coda senza a-capo
    let mut v = Vec::new();
    for k in 0..30usize {
        v.extend(std::iter::repeat_n(b'x', (k * 37) % 1000));
        v.push(b'\n');
        if k % 5 == 0 {
            v.push(b'\n');
        }
    }
    v.extend_from_slice(b"coda senza a capo");
    tutti.push(v);
    // una riga enorme in mezzo a due corte
    let mut v = b"inizio\n".to_vec();
    v.extend(vec![b'z'; 5000]);
    v.extend_from_slice(b"\nfine\n");
    tutti.push(v);
    tutti
}

#[test]
fn newlines_before_is_exact_at_every_offset_with_block_points() {
    for dati in dati_byte() {
        let p = file(&dati);
        let src = FileSource::open(&p).unwrap();
        for chunk in [2usize, 3, 8, 13, 64] {
            for every in [1u32, 256] {
                let (c, pr) = flags();
                let opts = BuildOptions {
                    chunk_bytes: chunk,
                    checkpoint_every: every,
                };
                let idx = LineIndex::build_with(&src, &opts, &c, &pr).unwrap();
                for offset in 0..=dati.len() {
                    assert_eq!(
                        idx.newlines_before(&src, offset as u64),
                        Some(ingenuo(&dati, offset, Newline::Byte)),
                        "len {} blocco {chunk} every {every} offset {offset}",
                        dati.len()
                    );
                }
            }
        }
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn newlines_before_is_exact_with_utf16_units() {
    // "a\n" "bcd" "\n" ... in UTF-16LE, con righe lunghe: i blocchi partono sempre su un'unita'
    let mut dati = Vec::new();
    for k in 0..60usize {
        for _ in 0..(k * 13) % 70 {
            dati.extend_from_slice(&[b'q', 0x00]);
        }
        dati.extend_from_slice(&[0x0A, 0x00]);
    }
    // un byte 0x0A dentro un altro carattere (U+010A = 0A 01) non e' un a-capo
    dati.extend_from_slice(&[0x0A, 0x01, 0x0A, 0x00]);
    let p = file(&dati);
    let src = FileSource::open(&p).unwrap();
    for chunk in [2usize, 6, 8, 64, 1000] {
        let (c, pr) = flags();
        let opts = BuildOptions {
            chunk_bytes: chunk,
            checkpoint_every: 256,
        };
        let idx = LineIndex::build_newline(&src, Newline::Le16, &opts, &c, &pr).unwrap();
        for offset in 0..=dati.len() {
            assert_eq!(
                idx.newlines_before(&src, offset as u64),
                Some(ingenuo(&dati, offset, Newline::Le16)),
                "blocco {chunk} offset {offset}"
            );
        }
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_extended_index_is_exact_too() {
    for dati in dati_byte() {
        if dati.len() < 20 {
            continue;
        }
        // il file cresce: l'indice del pezzo iniziale si allunga con la coda
        for taglio in [dati.len() / 3, dati.len() / 2, dati.len() - 1] {
            let corto = file(&dati[..taglio]);
            let lungo = file(&dati);
            let (c, pr) = flags();
            let opts = BuildOptions {
                chunk_bytes: 8,
                checkpoint_every: 256,
            };
            let src_corto = FileSource::open(&corto).unwrap();
            let idx = LineIndex::build_with(&src_corto, &opts, &c, &pr).unwrap();
            let src_lungo = FileSource::open(&lungo).unwrap();
            let esteso = idx
                .extend(&src_lungo, taglio as u64, 8, &c, &AtomicU64::new(0))
                .unwrap();
            for offset in 0..=dati.len() {
                assert_eq!(
                    esteso.newlines_before(&src_lungo, offset as u64),
                    Some(ingenuo(&dati, offset, Newline::Byte)),
                    "taglio {taglio} offset {offset}"
                );
            }
            let _ = std::fs::remove_file(corto);
            let _ = std::fs::remove_file(lungo);
        }
    }
}

#[test]
fn there_is_one_block_point_per_block_after_the_first() {
    let dati = vec![b'a'; 100];
    let p = file(&dati);
    let src = FileSource::open(&p).unwrap();
    let (c, pr) = flags();
    let opts = BuildOptions {
        chunk_bytes: 8,
        checkpoint_every: 256,
    };
    let idx = LineIndex::build_with(&src, &opts, &c, &pr).unwrap();
    // 100 byte in blocchi da 8: 13 blocchi, il primo non ha il punto
    assert_eq!(idx.block_point_count(), 12);
    // un file piccolo con un solo blocco: nessun punto di blocco
    let (c, pr) = flags();
    let idx = LineIndex::build(&src, &c, &pr).unwrap();
    assert_eq!(idx.block_point_count(), 0);
    let _ = std::fs::remove_file(p);
}
