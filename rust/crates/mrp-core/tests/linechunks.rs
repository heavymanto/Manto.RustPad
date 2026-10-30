//! Test della tabella dei numeri di riga incrementale (E15). Scritti da ARCH prima dell'implementazione: gli attesi
//! vengono da un oracolo ingenuo (un `Vec<u8>` su cui si fa insert/drain e si contano i `\n`) e da calcoli a mano, mai
//! dal codice sotto test. Le soglie "pochi blocchi ricalcolati" sono la prova che il costo non cresce col numero di
//! pezzi: con una tabella ricostruita da capo per ogni modifica il contatore salirebbe al numero di blocchi.

use mrp_core::edit::{Piece, PieceTable};
use mrp_core::Document;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// File temporaneo, nome unico per ogni apertura (su Windows non si tronca un file mappato), cancellato alla fine.
struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!(
            "mrp_linechunks_test_{}_{}.bin",
            std::process::id(),
            n
        ));
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

/// Contenuto di partenza: 400 byte con molti `\n`.
fn base_content() -> Vec<u8> {
    (0..400u32)
        .map(|i| {
            if i % 7 == 3 {
                b'\n'
            } else {
                b'a' + (i % 23) as u8
            }
        })
        .collect()
}

enum Op {
    Ins(u64, Vec<u8>),
    Del(u64, u64),
}

/// Una modifica casuale, SEMPRE efficace (mai una cancellazione di 0 byte), applicata subito all'oracolo.
/// Gli inserimenti hanno 1..=5 byte e possono contenere `\n`; le cancellazioni sono corte (1..=8 byte) e ogni
/// tanto lunghe (fino a un quarto del documento).
fn random_op(r: &mut Lcg, oracle: &mut Vec<u8>) -> Op {
    let len = oracle.len() as u64;
    if len > 0 && r.below(3) == 0 {
        let max = if r.below(15) == 0 {
            (len / 4).max(1)
        } else {
            len.min(8)
        };
        let n = 1 + r.below(max);
        let pos = r.below(len - n + 1);
        oracle.drain(pos as usize..(pos + n) as usize);
        return Op::Del(pos, n);
    }
    let n = 1 + r.below(5) as usize;
    let data: Vec<u8> = (0..n)
        .map(|_| {
            if r.below(4) == 0 {
                b'\n'
            } else {
                b'A' + r.below(26) as u8
            }
        })
        .collect();
    let pos = r.below(len + 1);
    for (k, b) in data.iter().enumerate() {
        oracle.insert(pos as usize + k, *b);
    }
    Op::Ins(pos, data)
}

fn apply(d: &mut Document, op: &Op) {
    match op {
        Op::Ins(p, x) => d.insert(*p, x).unwrap(),
        Op::Del(p, n) => d.delete(*p, *n).unwrap(),
    }
}

/// Inizi di riga del documento, ricalcolati da zero (stessa convenzione di `Document::line_count`).
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

fn naive_nl_before(v: &[u8], pos: u64) -> u64 {
    v[..pos as usize].iter().filter(|b| **b == b'\n').count() as u64
}

/// Confronta con l'oracolo il conteggio righe, un campione di inizi di riga (tutti se `tutte`) e di
/// posizioni.
fn check_lines(d: &mut Document, oracle: &[u8], r: &mut Lcg, tutte: bool, ctx: &str) {
    let starts = naive_line_starts(oracle);
    assert_eq!(d.line_count(), Some(starts.len() as u64), "{ctx}");
    let righe: Vec<usize> = if tutte {
        (0..=starts.len()).collect()
    } else {
        (0..4)
            .map(|_| r.below(starts.len() as u64 + 1) as usize)
            .collect()
    };
    for l in righe {
        assert_eq!(
            d.offset_of_line(l as u64),
            starts.get(l).copied(),
            "riga {l}: {ctx}"
        );
    }
    if !oracle.is_empty() {
        let posizioni: Vec<u64> = if tutte {
            (0..oracle.len() as u64).collect()
        } else {
            (0..4).map(|_| r.below(oracle.len() as u64)).collect()
        };
        for pos in posizioni {
            assert_eq!(
                d.line_at_offset(pos),
                Some(naive_nl_before(oracle, pos)),
                "pos {pos}: {ctx}"
            );
        }
        assert_eq!(d.line_at_offset(oracle.len() as u64), None, "{ctx}");
    }
}

/// Un documento con blocchi da 4 pezzi, riempito di modifiche finche' ha molti blocchi, con l'indice righe pronto.
fn many_chunks_doc(f: &TempFile, oracle: &mut Vec<u8>, r: &mut Lcg) -> Document {
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    for _ in 0..250 {
        let op = random_op(r, oracle);
        apply(&mut d, &op);
    }
    assert_eq!(d.read_range(0, u64::MAX), *oracle);
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    d.build_lines(&cancel, &progress).unwrap();
    assert!(
        d.table().chunk_count() >= 30,
        "il test vuole molti blocchi: {}",
        d.table().chunk_count()
    );
    d
}

// ---------- PieceTable: viste sui blocchi ed etichette ----------

#[test]
fn chunk_views_agree_with_the_flat_piece_list() {
    let mut t = PieceTable::with_chunk_capacity(500, 4);
    let mut r = Lcg(3);
    let mut len = 500u64;
    for _ in 0..120 {
        t.insert(r.below(len + 1), b"xy").unwrap();
        len += 2;
    }
    assert_eq!(t.chunk_count(), t.chunk_sizes().len());
    assert!(t.chunk_count() > 10);
    let mut piatti: Vec<Piece> = Vec::new();
    let mut somma = 0u64;
    for c in 0..t.chunk_count() {
        let pezzi = t.chunk_pieces(c);
        assert_eq!(pezzi.len(), t.chunk_sizes()[c]);
        assert_eq!(t.chunk_len(c), pezzi.iter().map(|p| p.len).sum::<u64>());
        somma += t.chunk_len(c);
        piatti.extend_from_slice(pezzi);
    }
    assert_eq!(piatti, t.pieces());
    assert_eq!(somma, t.len());
    assert!(t.chunk_pieces(t.chunk_count()).is_empty());
    assert_eq!(t.chunk_len(t.chunk_count()), 0);
    assert_eq!(t.chunk_stamp(t.chunk_count()), 0);
    assert_eq!(t.chunk_stamp(usize::MAX), 0);
    assert!(t.chunk_pieces(usize::MAX).is_empty());
}

#[test]
fn stamps_are_distinct_nonzero_and_stable_between_calls() {
    let mut t = PieceTable::with_chunk_capacity(500, 4);
    let mut r = Lcg(8);
    for len in (500u64..).take(100) {
        t.insert(r.below(len + 1), b"q").unwrap();
    }
    let stamps: Vec<u64> = (0..t.chunk_count()).map(|c| t.chunk_stamp(c)).collect();
    assert!(stamps.len() > 10);
    assert!(stamps.iter().all(|s| *s != 0));
    let mut ordinate = stamps.clone();
    ordinate.sort_unstable();
    ordinate.dedup();
    assert_eq!(
        ordinate.len(),
        stamps.len(),
        "etichette ripetute: {stamps:?}"
    );
    let ancora: Vec<u64> = (0..t.chunk_count()).map(|c| t.chunk_stamp(c)).collect();
    assert_eq!(stamps, ancora);
    // anche una tabella con un solo pezzo e un clone hanno etichette valide; il clone ha lo stesso contenuto
    // e quindi puo' avere le stesse etichette
    let uno = PieceTable::new(10);
    assert_ne!(uno.chunk_stamp(0), 0);
    let copia = t.clone();
    for c in 0..t.chunk_count() {
        assert_eq!(copia.chunk_pieces(c), t.chunk_pieces(c));
    }
}

#[test]
fn the_same_stamp_always_means_the_same_pieces_and_an_insert_touches_few_chunks() {
    let mut t = PieceTable::with_chunk_capacity(600, 4);
    let mut r = Lcg(21);
    let mut len = 600u64;
    // contenuto visto per etichetta: la proprieta' che rende sicura la cache del documento
    let mut viste: HashMap<u64, Vec<Piece>> = HashMap::new();
    let registra = |t: &PieceTable, viste: &mut HashMap<u64, Vec<Piece>>| {
        for c in 0..t.chunk_count() {
            let s = t.chunk_stamp(c);
            let pezzi = t.chunk_pieces(c).to_vec();
            if let Some(prima) = viste.insert(s, pezzi.clone()) {
                assert_eq!(prima, pezzi, "l'etichetta {s} ha cambiato contenuto");
            }
        }
    };
    for _ in 0..150 {
        t.insert(r.below(len + 1), b"ab").unwrap();
        len += 2;
    }
    registra(&t, &mut viste);
    assert!(t.chunk_count() >= 30);
    for passo in 0..600 {
        let prima: Vec<u64> = (0..t.chunk_count()).map(|c| t.chunk_stamp(c)).collect();
        if r.below(3) == 0 && len > 20 {
            let n = 1 + r.below(4);
            let pos = r.below(len - n + 1);
            t.delete(pos, n).unwrap();
            len -= n;
        } else {
            let pos = r.below(len + 1);
            t.insert(pos, b"cd").unwrap();
            len += 2;
        }
        registra(&t, &mut viste);
        let dopo: Vec<u64> = (0..t.chunk_count()).map(|c| t.chunk_stamp(c)).collect();
        let nuove = dopo.iter().filter(|s| !prima.contains(s)).count();
        let sparite = prima.iter().filter(|s| !dopo.contains(s)).count();
        assert!(
            (1..=6).contains(&nuove),
            "passo {passo}: {nuove} blocchi con etichetta nuova (attesi 1..=6)"
        );
        assert!(
            sparite <= 8,
            "passo {passo}: {sparite} etichette sparite (al piu' 8)"
        );
    }
}

#[test]
fn undo_and_redo_never_reuse_a_stamp_for_different_pieces() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    let mut oracle = base.clone();
    let mut r = Lcg(5);
    let mut viste: HashMap<u64, Vec<Piece>> = HashMap::new();
    let registra = |d: &Document, viste: &mut HashMap<u64, Vec<Piece>>| {
        let t = d.table();
        for c in 0..t.chunk_count() {
            let s = t.chunk_stamp(c);
            let pezzi = t.chunk_pieces(c).to_vec();
            if let Some(prima) = viste.insert(s, pezzi.clone()) {
                assert_eq!(prima, pezzi, "l'etichetta {s} ha cambiato contenuto");
            }
        }
    };
    let mut passi = 0;
    for _ in 0..120 {
        let op = random_op(&mut r, &mut oracle);
        apply(&mut d, &op);
        d.seal();
        passi += 1;
        registra(&d, &mut viste);
    }
    for _ in 0..passi {
        assert!(d.undo().is_some());
        registra(&d, &mut viste);
    }
    for _ in 0..passi {
        assert!(d.redo().is_some());
        registra(&d, &mut viste);
    }
    assert_eq!(d.read_range(0, u64::MAX), oracle);
}

// ---------- Document: conteggio righe incrementale ----------

#[test]
fn the_first_query_counts_each_chunk_once_and_repeated_queries_cost_nothing() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut oracle = base.clone();
    let mut r = Lcg(42);
    let mut d = many_chunks_doc(&f, &mut oracle, &mut r);
    let c0 = d.chunk_aggregations();
    check_lines(&mut d, &oracle, &mut r, true, "prima interrogazione");
    let c1 = d.chunk_aggregations();
    assert_eq!(
        c1 - c0,
        d.table().chunk_count() as u64,
        "ogni blocco va contato una volta sola"
    );
    // altre interrogazioni senza modifiche in mezzo: nessun ricalcolo
    check_lines(&mut d, &oracle, &mut r, true, "seconda interrogazione");
    assert_eq!(d.chunk_aggregations(), c1);
}

#[test]
fn an_edit_recounts_only_the_touched_chunks() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut oracle = base.clone();
    let mut r = Lcg(43);
    let mut d = many_chunks_doc(&f, &mut oracle, &mut r);
    check_lines(&mut d, &oracle, &mut r, false, "avvio");
    for passo in 0..400 {
        let op = random_op(&mut r, &mut oracle);
        // le cancellazioni lunghe attraversano molti blocchi: il limite vale per le modifiche corte
        let lunga = matches!(op, Op::Del(_, n) if n > 8);
        apply(&mut d, &op);
        let c = d.chunk_aggregations();
        check_lines(
            &mut d,
            &oracle,
            &mut r,
            passo % 25 == 0,
            &format!("passo {passo}"),
        );
        let fatti = d.chunk_aggregations() - c;
        assert!(fatti >= 1, "passo {passo}: nessun blocco ricalcolato");
        if !lunga {
            assert!(
                fatti <= 8,
                "passo {passo}: {fatti} blocchi ricalcolati su {} (attesi al piu' 8)",
                d.table().chunk_count()
            );
        }
    }
}

#[test]
fn undo_and_redo_are_exact_and_recount_only_the_touched_chunks() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    d.build_lines(&cancel, &progress).unwrap();
    let mut oracle = base.clone();
    let mut stati = vec![oracle.clone()];
    let mut r = Lcg(7);
    check_lines(&mut d, &oracle, &mut r, false, "avvio");
    // 150 passi da una modifica sola, con le righe interrogate dopo ciascuno
    // quali passi sono cancellazioni lunghe: annullarle rimette molti pezzi, quindi molti blocchi nuovi
    let mut lunghe: Vec<bool> = Vec::new();
    for passo in 0..150 {
        let op = random_op(&mut r, &mut oracle);
        lunghe.push(matches!(op, Op::Del(_, n) if n > 8));
        apply(&mut d, &op);
        d.seal();
        stati.push(oracle.clone());
        check_lines(&mut d, &oracle, &mut r, false, &format!("modifica {passo}"));
    }
    assert!(d.table().chunk_count() >= 20);
    let ultimo = stati.len() - 1;
    for i in (0..ultimo).rev() {
        assert!(d.undo().is_some());
        let c = d.chunk_aggregations();
        check_lines(
            &mut d,
            &stati[i],
            &mut r,
            i % 20 == 0,
            &format!("annulla verso {i}"),
        );
        let fatti = d.chunk_aggregations() - c;
        // una sola modifica del documento (una sola splice): pochi blocchi
        assert!(
            lunghe[i] || fatti <= 8,
            "annulla verso {i}: {fatti} blocchi ricalcolati"
        );
    }
    for (i, stato) in stati.iter().enumerate().skip(1) {
        assert!(d.redo().is_some());
        let c = d.chunk_aggregations();
        check_lines(
            &mut d,
            stato,
            &mut r,
            i % 20 == 0,
            &format!("ripeti verso {i}"),
        );
        let fatti = d.chunk_aggregations() - c;
        assert!(
            lunghe[i - 1] || fatti <= 8,
            "ripeti verso {i}: {fatti} blocchi ricalcolati"
        );
    }
}

#[test]
fn a_step_with_several_edits_is_exact() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    d.build_lines(&cancel, &progress).unwrap();
    let mut oracle = base.clone();
    let mut stati = vec![oracle.clone()];
    let mut r = Lcg(99);
    for _ in 0..80 {
        for _ in 0..1 + r.below(4) {
            let op = random_op(&mut r, &mut oracle);
            apply(&mut d, &op);
        }
        d.seal();
        stati.push(oracle.clone());
        check_lines(&mut d, &oracle, &mut r, false, "passo composto");
    }
    for i in (0..stati.len() - 1).rev() {
        assert!(d.undo().is_some());
        check_lines(&mut d, &stati[i], &mut r, true, &format!("annulla {i}"));
    }
    assert!(!d.is_modified());
}

#[test]
fn queries_before_the_lines_are_built_and_after_a_save_stay_exact() {
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    // senza indice righe niente risposte
    assert_eq!(d.line_count(), None);
    assert_eq!(d.offset_of_line(0), None);
    assert_eq!(d.line_at_offset(0), None);
    let mut oracle = base.clone();
    let mut r = Lcg(11);
    for _ in 0..120 {
        let op = random_op(&mut r, &mut oracle);
        apply(&mut d, &op);
    }
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    d.build_lines(&cancel, &progress).unwrap();
    check_lines(&mut d, &oracle, &mut r, true, "dopo build_lines");
    // salvataggio: il file cambia, l'indice righe va ricostruito e le etichette vecchie non devono servire
    d.save(&cancel, &progress).unwrap();
    assert_eq!(std::fs::read(&f.0).unwrap(), oracle);
    assert_eq!(d.line_count(), None);
    d.build_lines(&cancel, &progress).unwrap();
    check_lines(&mut d, &oracle, &mut r, true, "dopo salvataggio");
    for passo in 0..120 {
        let op = random_op(&mut r, &mut oracle);
        apply(&mut d, &op);
        check_lines(
            &mut d,
            &oracle,
            &mut r,
            passo % 20 == 0,
            &format!("dopo salvataggio, passo {passo}"),
        );
    }
}

#[test]
fn edge_documents_are_exact() {
    // vuoto, un solo byte, solo a-capo, e un documento che torna vuoto
    let f = TempFile::new(b"");
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    d.build_lines(&cancel, &progress).unwrap();
    assert_eq!(d.line_count(), Some(0));
    assert_eq!(d.offset_of_line(0), None);
    assert_eq!(d.line_at_offset(0), None);
    d.insert(0, b"\n").unwrap();
    assert_eq!(d.line_count(), Some(1));
    assert_eq!(d.offset_of_line(0), Some(0));
    assert_eq!(d.line_at_offset(0), Some(0));
    d.insert(1, b"ab\n\ncd").unwrap();
    // "\nab\n\ncd": inizi di riga 0, 1, 4, 5
    assert_eq!(d.line_count(), Some(4));
    assert_eq!(d.offset_of_line(2), Some(4));
    assert_eq!(d.offset_of_line(3), Some(5));
    assert_eq!(d.offset_of_line(4), None);
    assert_eq!(d.line_at_offset(6), Some(3));
    let n = d.len();
    d.delete(0, n).unwrap();
    assert_eq!(d.line_count(), Some(0));
    assert_eq!(d.line_at_offset(0), None);
    d.insert(0, b"x").unwrap();
    assert_eq!(d.line_count(), Some(1));
}

#[test]
fn the_cost_of_an_edit_does_not_grow_with_the_number_of_pieces() {
    // Blocchi di dimensione normale (512) e molti pezzi: il numero di blocchi ricalcolati dopo una modifica
    // resta quello di prima (pochi), non il numero totale di blocchi.
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open(&f.0).unwrap();
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    d.build_lines(&cancel, &progress).unwrap();
    let mut oracle = base.clone();
    let mut r = Lcg(1234);
    // inserimenti di 1 byte sparsi: ognuno aggiunge circa 2 pezzi
    for _ in 0..6000 {
        let pos = r.below(oracle.len() as u64 + 1);
        let b = if r.below(5) == 0 { b'\n' } else { b'z' };
        d.insert(pos, &[b]).unwrap();
        oracle.insert(pos as usize, b);
    }
    assert!(
        d.table().chunk_count() >= 10,
        "il test vuole molti blocchi: {}",
        d.table().chunk_count()
    );
    check_lines(&mut d, &oracle, &mut r, false, "dopo 6000 modifiche");
    let c = d.chunk_aggregations();
    for _ in 0..300 {
        let pos = r.below(oracle.len() as u64 + 1);
        d.insert(pos, b"k").unwrap();
        oracle.insert(pos as usize, b'k');
        assert!(d.line_count().is_some());
    }
    let fatti = d.chunk_aggregations() - c;
    assert!(
        (300..=300 * 8).contains(&fatti),
        "{fatti} blocchi ricalcolati per 300 modifiche (attesi 300..=2400) su {} blocchi",
        d.table().chunk_count()
    );
    check_lines(&mut d, &oracle, &mut r, false, "fine");
}

#[test]
fn the_per_chunk_cache_is_pruned_without_losing_the_live_entries() {
    // Una interrogazione dopo ogni modifica, abbastanza a lungo perche' la cache dei conteggi per blocco superi
    // il doppio dei blocchi piu' il margine e venga potata: dopo la potatura i blocchi vivi non vanno ricontati
    // (altrimenti prima o poi una modifica costerebbe tutti i blocchi).
    let base = base_content();
    let f = TempFile::new(&base);
    let mut d = Document::open_with_chunk(&f.0, 4).unwrap();
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    d.build_lines(&cancel, &progress).unwrap();
    let mut oracle = base.clone();
    let mut r = Lcg(2024);
    assert!(d.line_count().is_some());
    let mut massimo = 0u64;
    for passo in 0..3000 {
        let pos = r.below(oracle.len() as u64 + 1);
        let b = if r.below(4) == 0 { b'\n' } else { b'w' };
        d.insert(pos, &[b]).unwrap();
        oracle.insert(pos as usize, b);
        let c = d.chunk_aggregations();
        let attese = naive_line_starts(&oracle).len() as u64;
        assert_eq!(d.line_count(), Some(attese), "passo {passo}");
        let fatti = d.chunk_aggregations() - c;
        massimo = massimo.max(fatti);
        assert!(
            (1..=8).contains(&fatti),
            "passo {passo}: {fatti} blocchi ricalcolati su {}",
            d.table().chunk_count()
        );
    }
    assert!(
        d.table().chunk_count() > 400,
        "il test vuole una cache oltre il margine: {} blocchi",
        d.table().chunk_count()
    );
    assert!(massimo <= 8);
}
