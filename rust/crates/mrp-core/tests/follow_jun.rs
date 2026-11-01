//! Test del junior per il seguito della coda di un file (fase F1): crescita un byte alla
//! volta e righe molto lunghe su una crescita piu' grande di un blocco. Gli attesi vengono
//! da un oracolo ingenuo (conta dei `\n` e posizioni degli inizi, dal contenuto scritto o
//! calcolati a mano), mai dall'output del codice sotto test.

use mrp_core::lines::BuildOptions;
use mrp_core::{Document, Follow};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_follow_jun_{}_{}.log", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }

    fn append(&self, data: &[u8]) {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&self.0)
            .unwrap();
        f.write_all(data).unwrap();
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn libero() -> (AtomicBool, AtomicU64) {
    (AtomicBool::new(false), AtomicU64::new(0))
}

/// L'oracolo ingenuo: quante righe ha `d` con la convenzione di `lines.rs` (a-capo piu' 1
/// se `d` e' non vuoto e non finisce con un a-capo; un file vuoto ha 0 righe).
fn righe_oracolo(d: &[u8]) -> u64 {
    if d.is_empty() {
        return 0;
    }
    let nl = d.iter().filter(|&&b| b == b'\n').count() as u64;
    if d.last() == Some(&b'\n') {
        nl
    } else {
        nl + 1
    }
}

/// L'oracolo ingenuo: le posizioni degli inizi di riga di `d` (stessa convenzione).
fn inizi_oracolo(d: &[u8]) -> Vec<u64> {
    let mut v: Vec<u64> = Vec::new();
    if !d.is_empty() {
        v.push(0);
        for (i, &b) in d.iter().enumerate() {
            if b == b'\n' && i + 1 < d.len() {
                v.push(i as u64 + 1);
            }
        }
    }
    v
}

#[test]
fn one_byte_at_a_time_up_to_200_matches_the_naive_count() {
    // 200 byte con a-capo sparsi (anche due di fila e l'ultimo byte senza a-capo): dopo ogni
    // byte aggiunto il documento segue la coda e righe e inizi devono restare quelli
    // dell'oracolo.
    let pattern: Vec<u8> = (0..200u32)
        .map(|i| match i % 6 {
            2 | 3 => b'\n',
            _ => b'a' + (i % 24) as u8,
        })
        .collect();
    let mut contenuto: Vec<u8> = pattern[..1].to_vec();
    let t = TempFile::new(&contenuto);
    let mut d = Document::open(&t.0).unwrap();
    let (c, p) = libero();
    d.build_lines(&c, &p).unwrap();
    for i in 1..200usize {
        t.append(&pattern[i..i + 1]);
        contenuto.push(pattern[i]);
        let (c, p) = libero();
        assert_eq!(
            d.follow_growth(&c, &p).unwrap(),
            Follow::Grown(1),
            "byte {i}"
        );
        assert_eq!(d.len(), (i + 1) as u64, "byte {i}");
        assert_eq!(d.line_count(), Some(righe_oracolo(&contenuto)), "byte {i}");
        // Ogni tanto (e alla fine) tutti gli inizi, sempre dall'oracolo.
        if i % 10 == 0 || i == 199 {
            let inizi = inizi_oracolo(&contenuto);
            for (riga, &pos) in inizi.iter().enumerate() {
                assert_eq!(
                    d.offset_of_line(riga as u64),
                    Some(pos),
                    "riga {riga}, byte {i}"
                );
                assert_eq!(
                    d.line_at_offset(pos),
                    Some(riga as u64),
                    "offset {pos}, byte {i}"
                );
            }
            assert_eq!(d.offset_of_line(inizi.len() as u64), None, "byte {i}");
        }
    }
    assert!(!d.is_modified());
}

#[test]
fn long_lines_over_a_growth_bigger_than_one_block_match_the_hand_made_starts() {
    // Righe molto lunghe (300001 byte ciascuna) e una crescita di oltre 4 MiB: piu' di un
    // blocco di `follow_growth`, con il confine che capita in mezzo a una riga. Tutti gli
    // inizi sono a multipli di 300001: attesi calcolati a mano. Il file si scrive a flusso,
    // una riga alla volta, e non viene mai caricato per intero.
    const RIGA: usize = 300_001; // 300000 lettere + '\n'
    let passo = RIGA as u64;
    let mut riga: Vec<u8> = vec![b'x'; RIGA - 1];
    riga.push(b'\n');
    // Base: 3 righe, scritte prima di aprire il documento.
    let t = TempFile::new(&[]);
    for _ in 0..3 {
        t.append(&riga);
    }
    let mut d = Document::open(&t.0).unwrap();
    // Indice della base con blocchi piccoli e punti fitti: cosi' la giuntura ha punti
    // vecchi vicini a quelli nuovi.
    let opts = BuildOptions {
        chunk_bytes: 256 * 1024,
        checkpoint_every: 2,
    };
    let (c, p) = libero();
    d.build_lines_with(&opts, &c, &p).unwrap();
    assert_eq!(d.len(), 3 * passo);
    assert_eq!(d.line_count(), Some(3));
    // Crescita: 15 righe complete e una troncata senza a-capo (4600015 byte, oltre un
    // blocco di 4 MiB).
    for _ in 0..15 {
        t.append(&riga);
    }
    t.append(&riga[..100_000]);
    let (c, p) = libero();
    assert_eq!(
        d.follow_growth(&c, &p).unwrap(),
        Follow::Grown(15 * passo + 100_000)
    );
    assert_eq!(d.len(), 18 * passo + 100_000);
    // 18 righe complete piu' quella troncata: 19.
    assert_eq!(d.line_count(), Some(19));
    for riga_n in 0..19u64 {
        assert_eq!(
            d.offset_of_line(riga_n),
            Some(passo * riga_n),
            "riga {riga_n}"
        );
    }
    assert_eq!(d.offset_of_line(19), None);
    // Dentro una riga lunga, lontano dagli inizi: nella base e nella parte nuova.
    assert_eq!(d.line_at_offset(150_000), Some(0));
    assert_eq!(d.line_at_offset(17 * passo + 150_000), Some(17));
    assert!(!d.is_modified());
}
