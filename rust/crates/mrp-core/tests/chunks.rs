//! Test della piece table a blocchi (E14). Scritti da ARCH prima dell'implementazione: gli attesi vengono da un
//! oracolo ingenuo (un `Vec<u8>` su cui si fa insert/drain) e da calcoli a mano, mai dal codice sotto test.
//! I blocchi piccoli (`cap` 4) costringono a spezzare e fondere blocchi anche su documenti da poche centinaia di byte.

use mrp_core::edit::{Buf, Piece, PieceTable};
use mrp_core::{Document, FileSource};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// File temporaneo, nome unico per ogni apertura (su Windows non si tronca un file mappato), cancellato alla fine.
struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_chunks_test_{}_{}.bin", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Contenuto di partenza: 200 byte con molti `\n`, cosi' lo stesso file serve ai test sulle righe.
fn base_content() -> Vec<u8> {
    (0..200u32)
        .map(|i| {
            if i % 7 == 3 {
                b'\n'
            } else {
                b'a' + (i % 23) as u8
            }
        })
        .collect()
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

/// Forma canonica (nessun pezzo vuoto, nessuna coppia fondibile) e coerenza fra le viste sui pezzi.
fn assert_canonical(t: &PieceTable) {
    let v = t.pieces();
    assert_eq!(t.piece_count(), v.len());
    assert_eq!(t.iter().collect::<Vec<_>>(), v);
    let mut sum = 0u64;
    for (i, p) in v.iter().enumerate() {
        assert!(p.len > 0, "pezzo vuoto in posizione {i}");
        sum += p.len;
        if i > 0 {
            let a = v[i - 1];
            assert!(
                !(a.buf == p.buf && a.start + a.len == p.start),
                "pezzi {} e {i} fondibili: {a:?} {p:?}",
                i - 1
            );
        }
        assert_eq!(t.piece(i), Some(*p));
    }
    assert_eq!(sum, t.len());
    assert_eq!(t.piece(v.len()), None);
    assert_eq!(t.piece(usize::MAX), None);
}

/// Invarianti dei blocchi: la somma torna, nessun blocco vuoto o oltre `cap`, due blocchi vicini non sono
/// cosi' vuoti da poter stare insieme in mezzo blocco (somma > cap / 2).
fn assert_chunks(sizes: &[usize], pieces: usize, cap: usize) {
    assert_eq!(sizes.iter().sum::<usize>(), pieces, "{sizes:?}");
    for s in sizes {
        assert!(
            *s >= 1 && *s <= cap,
            "blocco di {s} pezzi con cap {cap}: {sizes:?}"
        );
    }
    for w in sizes.windows(2) {
        assert!(
            w[0] + w[1] > cap / 2,
            "blocchi vicini troppo piccoli ({} + {}) con cap {cap}: {sizes:?}",
            w[0],
            w[1]
        );
    }
}

/// Una modifica da fare al documento.
enum Op {
    Ins(u64, Vec<u8>),
    Del(u64, u64),
}

impl Op {
    /// Cambia qualcosa? Una cancellazione di 0 byte no (e non entra nella storia dell'annulla).
    fn effective(&self) -> bool {
        !matches!(self, Op::Del(_, 0))
    }
}

/// Sceglie una modifica casuale e la applica SUBITO all'oracolo; il chiamante la applica al documento.
fn random_op(r: &mut Lcg, oracle: &mut Vec<u8>) -> Op {
    let len = oracle.len() as u64;
    match r.below(20) {
        // cancellazione lunga: attraversa molti pezzi (e molti blocchi)
        0 => {
            let n = r.below(len / 2 + 1);
            let pos = r.below(len - n + 1);
            oracle.drain(pos as usize..(pos + n) as usize);
            Op::Del(pos, n)
        }
        // cancellazione corta
        1..=7 => {
            let n = r.below(len.min(12) + 1);
            let pos = r.below(len - n + 1);
            oracle.drain(pos as usize..(pos + n) as usize);
            Op::Del(pos, n)
        }
        // inserimento di 1..=5 byte
        _ => {
            let n = 1 + r.below(5) as usize;
            let data: Vec<u8> = (0..n).map(|_| b'A' + r.below(26) as u8).collect();
            let pos = r.below(len + 1);
            for (k, b) in data.iter().enumerate() {
                oracle.insert(pos as usize + k, *b);
            }
            Op::Ins(pos, data)
        }
    }
}

// ---------- PieceTable ----------

#[test]
fn new_table_has_one_chunk_with_the_single_piece() {
    assert_eq!(PieceTable::new(10).chunk_sizes(), vec![1]);
    let vuota = PieceTable::new(0);
    assert_eq!(vuota.chunk_sizes(), Vec::<usize>::new());
    assert_eq!(vuota.piece_count(), 0);
    assert_eq!(vuota.piece(0), None);
    assert_eq!(vuota.len(), 0);
}

#[test]
fn two_inserts_give_the_hand_computed_pieces_and_split_the_chunk() {
    // Calcolo a mano: [O0..100]; 'a' in 50 -> [O(0,50) A(0,1) O(50,50)]; 'b' in 10 spezza O(0,50) in
    // O(0,10) e O(10,40) e mette A(1,1) in mezzo -> 5 pezzi, piu' del cap 4: i blocchi devono essere due.
    let f = TempFile::new(&[b'x'; 100]);
    let src = FileSource::open(&f.0).unwrap();
    let mut t = PieceTable::with_chunk_capacity(100, 4);
    t.insert(50, b"a").unwrap();
    assert_eq!(t.pieces(), &[orig(0, 50), add(0, 1), orig(50, 50)]);
    assert_eq!(t.chunk_sizes(), vec![3]);
    t.insert(10, b"b").unwrap();
    assert_eq!(
        t.pieces(),
        &[
            orig(0, 10),
            add(1, 1),
            orig(10, 40),
            add(0, 1),
            orig(50, 50)
        ]
    );
    let sizes = t.chunk_sizes();
    assert_eq!(sizes.iter().sum::<usize>(), 5);
    assert!(sizes.len() >= 2, "{sizes:?}");
    assert_chunks(&sizes, 5, 4);
    let mut atteso = vec![b'x'; 100];
    atteso.insert(50, b'a');
    atteso.insert(10, b'b');
    assert_eq!(t.read_range(&src, 0, 1000), atteso);
}

#[test]
fn chunk_capacity_below_four_is_raised_to_four() {
    let f = TempFile::new(&[b'x'; 300]);
    let src = FileSource::open(&f.0).unwrap();
    let mut t = PieceTable::with_chunk_capacity(300, 1);
    let mut oracle = vec![b'x'; 300];
    for k in 0..60u64 {
        let pos = (k * 5) % (oracle.len() as u64 + 1);
        t.insert(pos, b"q").unwrap();
        oracle.insert(pos as usize, b'q');
    }
    assert_chunks(&t.chunk_sizes(), t.piece_count(), 4);
    // con cap 1 ogni blocco avrebbe un pezzo solo; con cap 4 due blocchi vicini non possono avere un pezzo ciascuno
    assert!(
        t.chunk_sizes().iter().any(|s| *s >= 2),
        "il cap deve valere 4, non 1"
    );
    assert_eq!(t.read_range(&src, 0, 10_000), oracle);
}

#[test]
fn random_edits_match_the_naive_oracle_for_several_capacities() {
    for (seed, cap) in [(1u64, 4usize), (2, 5), (3, 16), (4, 512)] {
        let base = base_content();
        let f = TempFile::new(&base);
        let src = FileSource::open(&f.0).unwrap();
        let mut t = PieceTable::with_chunk_capacity(base.len() as u64, cap);
        let mut oracle = base.clone();
        let mut r = Lcg(seed);
        let mut grande = 0usize; // picco di pezzi, per essere sicuri che il test spezzi davvero
        for step in 0..3000 {
            match random_op(&mut r, &mut oracle) {
                Op::Ins(p, d) => t.insert(p, &d).unwrap(),
                Op::Del(p, n) => t.delete(p, n).unwrap(),
            }
            assert_eq!(t.len(), oracle.len() as u64, "cap {cap} passo {step}");
            assert_eq!(
                t.read_range(&src, 0, u64::MAX),
                oracle,
                "cap {cap} passo {step}"
            );
            assert_canonical(&t);
            assert_chunks(&t.chunk_sizes(), t.piece_count(), cap.max(4));
            // letture parziali a caso
            let len = oracle.len() as u64;
            let a = r.below(len + 1);
            let n = r.below(40);
            let fine = (a + n).min(len);
            assert_eq!(
                t.read_range(&src, a, n),
                oracle[a as usize..fine as usize].to_vec()
            );
            grande = grande.max(t.piece_count());
        }
        if cap <= 16 {
            assert!(
                grande > 3 * cap,
                "cap {cap}: troppo pochi pezzi ({grande}) per provare i blocchi"
            );
        }
    }
}

#[test]
fn deleting_everything_then_typing_again_works() {
    let base = base_content();
    let f = TempFile::new(&base);
    let src = FileSource::open(&f.0).unwrap();
    let mut t = PieceTable::with_chunk_capacity(base.len() as u64, 4);
    for k in 0..40u64 {
        t.insert((k * 13) % (t.len() + 1), b"zz").unwrap();
    }
    let n = t.len();
    t.delete(0, n).unwrap();
    assert_eq!(t.len(), 0);
    assert_eq!(t.piece_count(), 0);
    assert_eq!(t.chunk_sizes(), Vec::<usize>::new());
    assert!(t.read_range(&src, 0, 10).is_empty());
    t.insert(0, b"fine").unwrap();
    assert_eq!(t.read_range(&src, 0, 10), b"fine");
    assert_canonical(&t);
    assert_chunks(&t.chunk_sizes(), t.piece_count(), 4);
}

// ---------- Document: annulla / ripeti, salvataggio, ricerca, righe ----------

fn whole(d: &Document) -> Vec<u8> {
    d.read_range(0, u64::MAX)
}

fn check_doc(d: &Document, cap: usize) {
    assert_canonical(d.table());
    assert_chunks(&d.table().chunk_sizes(), d.table().piece_count(), cap);
}

#[test]
fn undo_and_redo_walk_through_every_sealed_step_with_small_chunks() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    let mut oracle = base.clone();
    let mut states = vec![oracle.clone()];
    let mut r = Lcg(77);
    let passi = 250;
    for _ in 0..passi {
        let mut efficace = false;
        for _ in 0..1 + r.below(3) {
            let op = random_op(&mut r, &mut oracle);
            efficace |= op.effective();
            match op {
                Op::Ins(p, x) => d.insert(p, &x).unwrap(),
                Op::Del(p, n) => d.delete(p, n).unwrap(),
            }
        }
        d.seal();
        check_doc(&d, 4);
        // un passo fatto solo di cancellazioni di 0 byte non entra nella storia
        if efficace {
            states.push(oracle.clone());
        }
    }
    assert_eq!(whole(&d), oracle);
    let ultimo = states.len() - 1;
    for i in (0..ultimo).rev() {
        assert!(d.undo().is_some());
        assert_eq!(whole(&d), states[i], "dopo annulla verso lo stato {i}");
        check_doc(&d, 4);
    }
    assert!(!d.is_modified());
    assert!(d.undo().is_none());
    for (i, stato) in states.iter().enumerate().skip(1) {
        assert!(d.redo().is_some());
        assert_eq!(&whole(&d), stato, "dopo ripeti verso lo stato {i}");
        check_doc(&d, 4);
    }
    assert!(d.redo().is_none());
}

#[test]
fn save_keeps_the_chunk_capacity_for_the_new_table() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    let mut oracle = base.clone();
    for k in 0..30u64 {
        let pos = (k * 11) % (oracle.len() as u64 + 1);
        d.insert(pos, b"+").unwrap();
        oracle.insert(pos as usize, b'+');
    }
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    d.save(&cancel, &progress).unwrap();
    assert_eq!(std::fs::read(&f.0).unwrap(), oracle);
    assert_eq!(d.table().chunk_sizes(), vec![1]);
    for k in 0..40u64 {
        let pos = (k * 17) % (oracle.len() as u64 + 1);
        d.insert(pos, b"-").unwrap();
        oracle.insert(pos as usize, b'-');
    }
    assert_eq!(whole(&d), oracle);
    check_doc(&d, 4);
    assert!(
        d.table().chunk_sizes().len() > 1,
        "dopo il salvataggio il cap deve restare 4: {:?}",
        d.table().chunk_sizes()
    );
}

#[test]
fn snapshot_search_finds_every_occurrence_in_a_many_chunks_document() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    let mut oracle = base.clone();
    let mut r = Lcg(5);
    for _ in 0..150 {
        let pos = r.below(oracle.len() as u64 + 1);
        d.insert(pos, b"QQ").unwrap();
        oracle.insert(pos as usize, b'Q');
        oracle.insert(pos as usize, b'Q');
    }
    assert!(d.table().chunk_sizes().len() > 20);
    let snap = d.snapshot().unwrap().with_chunk(64);
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    let mut attesi = Vec::new();
    for i in 0..oracle.len().saturating_sub(1) {
        if &oracle[i..i + 2] == b"QQ" {
            attesi.push(i as u64);
        }
    }
    let mut trovati = Vec::new();
    let mut da = 0u64;
    while let Some(p) = snap.find_next(b"QQ", da, &cancel, &progress).unwrap() {
        trovati.push(p);
        da = p + 1;
    }
    assert_eq!(trovati, attesi);
}

/// Oracolo delle righe: inizi di riga ricalcolati da zero sul `Vec<u8>`.
fn naive_line_starts(v: &[u8]) -> Vec<u64> {
    let mut s = Vec::new();
    if !v.is_empty() {
        s.push(0);
    }
    for (i, b) in v.iter().enumerate() {
        if *b == b'\n' && i + 1 < v.len() {
            s.push(i as u64 + 1);
        }
    }
    s
}

#[test]
fn line_queries_follow_the_edits_with_many_chunks() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    d.build_lines(&cancel, &progress).unwrap();
    let mut oracle = base.clone();
    let mut r = Lcg(31);
    for passo in 0..300 {
        match random_op(&mut r, &mut oracle) {
            Op::Ins(p, x) => d.insert(p, &x).unwrap(),
            Op::Del(p, n) => d.delete(p, n).unwrap(),
        }
        // confronto completo ogni 10 passi, a campione negli altri
        let starts = naive_line_starts(&oracle);
        assert_eq!(d.line_count(), Some(starts.len() as u64), "passo {passo}");
        let righe: Vec<usize> = if passo % 10 == 0 {
            (0..starts.len()).collect()
        } else {
            (0..4)
                .map(|_| r.below(starts.len() as u64 + 1) as usize)
                .collect()
        };
        for l in righe {
            let atteso = starts.get(l).copied();
            assert_eq!(
                d.offset_of_line(l as u64),
                atteso,
                "riga {l} al passo {passo}"
            );
        }
        for _ in 0..4 {
            if oracle.is_empty() {
                break;
            }
            let pos = r.below(oracle.len() as u64);
            let atteso = oracle[..pos as usize]
                .iter()
                .filter(|b| **b == b'\n')
                .count() as u64;
            assert_eq!(
                d.line_at_offset(pos),
                Some(atteso),
                "pos {pos} al passo {passo}"
            );
        }
    }
    assert!(
        d.table().chunk_sizes().len() > 5,
        "il test vuole molti blocchi"
    );
}
