//! Test della piece table. Scritti da ARCH prima dell'implementazione: gli attesi vengono da calcoli a mano
//! o da un oracolo ingenuo (un `Vec<u8>` su cui si fa insert/drain), mai dall'output del codice sotto test.

use mrp_core::edit::{Buf, EditError, Piece, PieceTable};
use mrp_core::FileSource;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// File temporaneo cancellato alla fine del test.
struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_edit_test_{}_{}.bin", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
    fn open(&self) -> FileSource {
        FileSource::open(&self.0).unwrap()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn orig(start: u64, len: u64) -> Piece {
    Piece {
        buf: Buf::Original,
        start,
        len,
    }
}

fn add(start: u64, len: u64) -> Piece {
    Piece {
        buf: Buf::Add,
        start,
        len,
    }
}

/// Le tre invarianti della forma canonica.
fn assert_canonical(t: &PieceTable) {
    let mut sum = 0u64;
    for (i, p) in t.pieces().iter().enumerate() {
        assert!(p.len > 0, "pezzo vuoto in posizione {i}: {:?}", t.pieces());
        sum += p.len;
        if i > 0 {
            let a = t.pieces()[i - 1];
            assert!(
                !(a.buf == p.buf && a.start + a.len == p.start),
                "pezzi {} e {i} contigui e non fusi: {:?}",
                i - 1,
                t.pieces()
            );
        }
    }
    assert_eq!(sum, t.len(), "somma dei pezzi diversa da len()");
}

/// Il documento intero letto con `read_range`.
fn whole(t: &PieceTable, f: &FileSource) -> Vec<u8> {
    t.read_range(f, 0, t.len())
}

/// Generatore pseudocasuale deterministico (LCG).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

// ---------- costruzione ----------

#[test]
fn new_empty_document_has_no_pieces() {
    let t = PieceTable::new(0);
    assert_eq!(t.len(), 0);
    assert!(t.is_empty());
    assert!(t.pieces().is_empty());
    assert_eq!(t.add_len(), 0);
}

#[test]
fn new_document_is_one_original_piece() {
    let t = PieceTable::new(10);
    assert_eq!(t.len(), 10);
    assert_eq!(t.pieces(), &[orig(0, 10)]);
    assert_eq!(t.add_len(), 0);
}

// ---------- inserimenti: valori a mano ----------

#[test]
fn insert_in_the_middle_splits_the_original_piece() {
    let mut t = PieceTable::new(10);
    t.insert(4, b"XYZ").unwrap();
    assert_eq!(t.len(), 13);
    assert_eq!(t.pieces(), &[orig(0, 4), add(0, 3), orig(4, 6)]);
    assert_eq!(t.add_len(), 3);
}

#[test]
fn insert_at_start_and_at_end() {
    let mut t = PieceTable::new(10);
    t.insert(0, b"AB").unwrap();
    assert_eq!(t.pieces(), &[add(0, 2), orig(0, 10)]);
    t.insert(12, b"C").unwrap();
    assert_eq!(t.pieces(), &[add(0, 2), orig(0, 10), add(2, 1)]);
    assert_eq!(t.len(), 13);
}

#[test]
fn insert_into_empty_document() {
    let mut t = PieceTable::new(0);
    t.insert(0, b"hello").unwrap();
    assert_eq!(t.len(), 5);
    assert_eq!(t.pieces(), &[add(0, 5)]);
}

#[test]
fn typing_in_sequence_coalesces_into_one_add_piece() {
    // Si digita "abc" un carattere alla volta a partire dalla posizione 4: un solo pezzo Add.
    let mut t = PieceTable::new(10);
    t.insert(4, b"a").unwrap();
    t.insert(5, b"b").unwrap();
    t.insert(6, b"c").unwrap();
    assert_eq!(t.pieces(), &[orig(0, 4), add(0, 3), orig(4, 6)]);
    assert_eq!(t.add_len(), 3);
    assert_canonical(&t);
}

#[test]
fn insert_at_a_piece_boundary_does_not_split() {
    let mut t = PieceTable::new(10);
    t.insert(4, b"XY").unwrap(); // [orig 0..4][add 0..2][orig 4..10]
    t.insert(6, b"Z").unwrap(); // confine fra add e orig(4..): si fonde con add (contiguo nel buffer)
    assert_eq!(t.pieces(), &[orig(0, 4), add(0, 3), orig(4, 6)]);
    t.insert(4, b"Q").unwrap(); // confine fra orig(0..4) e add: Q non e' contiguo con add(0..3) -> pezzo nuovo
    assert_eq!(t.pieces(), &[orig(0, 4), add(3, 1), add(0, 3), orig(4, 6)]);
    assert_canonical(&t);
}

#[test]
fn insert_of_empty_data_changes_nothing_but_checks_position() {
    let mut t = PieceTable::new(10);
    t.insert(3, b"").unwrap();
    assert_eq!(t.pieces(), &[orig(0, 10)]);
    assert_eq!(t.add_len(), 0);
    assert!(t.insert(11, b"").is_err());
}

// ---------- cancellazioni: valori a mano ----------

#[test]
fn delete_inside_one_piece_splits_it() {
    let mut t = PieceTable::new(10);
    t.delete(3, 4).unwrap();
    assert_eq!(t.len(), 6);
    assert_eq!(t.pieces(), &[orig(0, 3), orig(7, 3)]);
}

#[test]
fn delete_at_the_edges_trims_a_piece() {
    let mut t = PieceTable::new(10);
    t.delete(0, 2).unwrap();
    assert_eq!(t.pieces(), &[orig(2, 8)]);
    t.delete(6, 2).unwrap();
    assert_eq!(t.pieces(), &[orig(2, 6)]);
    assert_eq!(t.len(), 6);
}

#[test]
fn delete_across_several_pieces() {
    let mut t = PieceTable::new(10);
    t.insert(4, b"XYZ").unwrap(); // [orig 0..4][add 0..3][orig 4..10], len 13
    t.delete(2, 8).unwrap(); // cancella le posizioni 2..10 del documento: orig(2..4), add intero, orig(4..7)
    assert_eq!(t.len(), 5);
    assert_eq!(t.pieces(), &[orig(0, 2), orig(7, 3)]);
    assert_eq!(
        t.add_len(),
        3,
        "cancellare non svuota il buffer delle aggiunte"
    );
}

#[test]
fn delete_everything_leaves_no_pieces() {
    let mut t = PieceTable::new(10);
    t.insert(5, b"abc").unwrap();
    t.delete(0, 13).unwrap();
    assert_eq!(t.len(), 0);
    assert!(t.pieces().is_empty());
}

#[test]
fn delete_that_rejoins_contiguous_pieces_merges_them() {
    // Si inserisce e subito si cancella il testo inserito: i due pezzi originali tornano contigui e si fondono.
    let mut t = PieceTable::new(10);
    t.insert(4, b"XYZ").unwrap();
    t.delete(4, 3).unwrap();
    assert_eq!(t.pieces(), &[orig(0, 10)]);
    assert_eq!(t.len(), 10);
}

#[test]
fn delete_of_zero_length_changes_nothing_but_checks_position() {
    let mut t = PieceTable::new(10);
    t.delete(10, 0).unwrap();
    assert_eq!(t.pieces(), &[orig(0, 10)]);
    assert!(t.delete(11, 0).is_err());
}

// ---------- errori ----------

#[test]
fn out_of_range_operations_fail_and_change_nothing() {
    let mut t = PieceTable::new(10);
    t.insert(4, b"XYZ").unwrap();
    let before = t.pieces().to_vec();
    let add_before = t.add_len();

    assert_eq!(
        t.insert(14, b"!"),
        Err(EditError::OutOfRange {
            pos: 14,
            len: 1,
            doc_len: 13
        })
    );
    assert!(t.delete(13, 1).is_err());
    assert!(t.delete(10, 4).is_err());
    assert!(t.delete(0, 14).is_err());
    assert!(t.delete(14, 0).is_err());
    assert!(t.delete(u64::MAX, 1).is_err());
    assert!(t.delete(1, u64::MAX).is_err(), "pos + len va in overflow");
    assert!(t.insert(u64::MAX, b"x").is_err());

    assert_eq!(t.pieces(), &before[..]);
    assert_eq!(t.len(), 13);
    assert_eq!(t.add_len(), add_before);
}

// ---------- lettura ----------

#[test]
fn read_range_returns_the_edited_content() {
    let f = TempFile::new(b"0123456789");
    let src = f.open();
    let mut t = PieceTable::new(src.len());
    t.insert(4, b"XYZ").unwrap();
    t.delete(0, 2).unwrap();
    // documento: "23" "XYZ" "456789" = 23XYZ456789
    assert_eq!(whole(&t, &src), b"23XYZ456789");
    assert_eq!(t.read_range(&src, 1, 5), b"3XYZ4");
    assert_eq!(t.read_range(&src, 5, 3), b"456");
}

#[test]
fn read_range_clamps_like_file_source() {
    let f = TempFile::new(b"0123456789");
    let src = f.open();
    let mut t = PieceTable::new(src.len());
    t.insert(10, b"END").unwrap();
    assert_eq!(t.read_range(&src, 8, 100), b"89END");
    assert_eq!(t.read_range(&src, 13, 5), b"");
    assert_eq!(t.read_range(&src, 99, 5), b"");
    assert_eq!(t.read_range(&src, 3, 0), b"");
    assert_eq!(t.read_range(&src, u64::MAX, u64::MAX), b"");
}

#[test]
fn read_range_of_an_empty_document_is_empty() {
    let f = TempFile::new(b"");
    let src = f.open();
    let t = PieceTable::new(src.len());
    assert_eq!(t.read_range(&src, 0, 10), b"");
}

// ---------- oracolo ingenuo, operazioni casuali ----------

fn run_random(seed: u64, original: &[u8], ops: usize) {
    let f = TempFile::new(original);
    let src = f.open();
    let mut t = PieceTable::new(src.len());
    let mut model: Vec<u8> = original.to_vec();
    let mut rng = Lcg(seed);
    let mut added = 0u64;
    for step in 0..ops {
        let r = rng.next() % 100;
        if r < 55 || model.is_empty() {
            // inserimento: spesso vicino all'ultimo punto (digitazione), a volte ai bordi
            let pos = match rng.next() % 5 {
                0 => 0,
                1 => model.len(),
                _ => (rng.next() as usize) % (model.len() + 1),
            };
            let n = (rng.next() % 6) as usize; // anche 0
            let data: Vec<u8> = (0..n).map(|_| b'a' + (rng.next() % 26) as u8).collect();
            t.insert(pos as u64, &data).unwrap();
            model.splice(pos..pos, data.iter().copied());
            added += data.len() as u64;
        } else {
            let pos = (rng.next() as usize) % model.len();
            let n = ((rng.next() % 12) as usize).min(model.len() - pos);
            t.delete(pos as u64, n as u64).unwrap();
            model.drain(pos..pos + n);
        }
        assert_eq!(t.len(), model.len() as u64, "passo {step}");
        assert_eq!(t.add_len(), added, "passo {step}");
        assert_canonical(&t);
        assert_eq!(whole(&t, &src), model, "passo {step}");
        if step % 25 == 0 && !model.is_empty() {
            let a = (rng.next() as usize) % model.len();
            let n = (rng.next() % 40) as usize;
            let want = &model[a..(a + n).min(model.len())];
            assert_eq!(t.read_range(&src, a as u64, n as u64), want, "passo {step}");
        }
    }
}

#[test]
fn random_edits_match_the_naive_model() {
    let original: Vec<u8> = (0..5000u32).map(|i| b'A' + (i % 26) as u8).collect();
    run_random(1, &original, 1500);
    run_random(2, &original, 1500);
    run_random(3, &original[..7], 800);
}

#[test]
fn random_edits_on_an_empty_original() {
    run_random(4, b"", 800);
}

// ---------- posizioni oltre 4 GiB (nessun file: la tabella e' pura) ----------

#[test]
fn positions_beyond_4_gib_are_exact() {
    const G: u64 = 1024 * 1024 * 1024;
    let mut t = PieceTable::new(10 * G);
    assert_eq!(t.len(), 10 * G);
    t.insert(9 * G, b"abc").unwrap();
    assert_eq!(t.len(), 10 * G + 3);
    assert_eq!(t.pieces(), &[orig(0, 9 * G), add(0, 3), orig(9 * G, G)]);
    t.delete(5 * G, G).unwrap();
    assert_eq!(t.len(), 9 * G + 3);
    assert_eq!(
        t.pieces(),
        &[
            orig(0, 5 * G),
            orig(6 * G, 3 * G),
            add(0, 3),
            orig(9 * G, G)
        ]
    );
    assert_canonical(&t);
}

// ---------- la memoria dipende dalle modifiche, non dal file ----------

#[test]
fn memory_follows_the_edits_not_the_file_size() {
    // Un file "da 8 EiB" (solo un numero) con 1000 inserimenti sparsi: il primo e' a inizio documento (2 pezzi), ogni altro ne aggiunge 2 -> 2000 pezzi; il buffer ha 1000 byte.
    let mut t = PieceTable::new(1 << 63);
    for i in 0..1000u64 {
        t.insert(i * 1_000_003 + i, b"x").unwrap();
    }
    assert_eq!(t.add_len(), 1000);
    assert_eq!(t.pieces().len(), 2000);
    assert_eq!(t.len(), (1u64 << 63) + 1000);
    assert_canonical(&t);
}
