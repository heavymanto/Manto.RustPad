//! Test dell'indice righe che segue le modifiche. Scritti da ARCH prima dell'implementazione: gli attesi vengono
//! da un oracolo ingenuo (un `Vec<u8>` con l'elenco degli inizi di riga ricalcolato da zero), mai dal codice.

use mrp_core::{BuildOptions, Document, FileSource, LineIndex};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_dl_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, nome: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(nome);
        std::fs::write(&p, data).unwrap();
        p
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn flags() -> (AtomicBool, AtomicU64) {
    (AtomicBool::new(false), AtomicU64::new(0))
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

/// Byte pseudocasuali con molti `\n` (uno su tre), qualche `\r` e qualche byte non UTF-8.
fn texty(r: &mut Lcg, n: usize) -> Vec<u8> {
    (0..n)
        .map(|_| match r.below(12) {
            0..=3 => b'\n',
            4 => b'\r',
            5 => 0xff,
            6..=8 => b'a',
            _ => b'b',
        })
        .collect()
}

fn opts(chunk: usize, every: u32) -> BuildOptions {
    BuildOptions {
        chunk_bytes: chunk,
        checkpoint_every: every,
    }
}

/// Oracolo: inizi di riga del contenuto, ricalcolati da zero con la convenzione di `lines.rs`
/// (un `\n` finale non apre una riga nuova).
fn starts(m: &[u8]) -> Vec<u64> {
    let mut v = Vec::new();
    if m.is_empty() {
        return v;
    }
    v.push(0);
    for (i, &b) in m.iter().enumerate() {
        if b == b'\n' && i + 1 < m.len() {
            v.push(i as u64 + 1);
        }
    }
    v
}

/// Confronta TUTTO: conteggio, ogni inizio di riga, ogni byte, e i valori fuori limite.
fn check_all(d: &mut Document, model: &[u8], what: &str) {
    let st = starts(model);
    assert_eq!(d.len(), model.len() as u64, "{what}: len");
    assert_eq!(d.line_count(), Some(st.len() as u64), "{what}: line_count");
    for (k, &s) in st.iter().enumerate() {
        assert_eq!(
            d.offset_of_line(k as u64),
            Some(s),
            "{what}: offset_of_line({k})"
        );
    }
    assert_eq!(
        d.offset_of_line(st.len() as u64),
        None,
        "{what}: oltre l'ultima riga"
    );
    assert_eq!(d.offset_of_line(u64::MAX), None, "{what}: u64::MAX");
    for p in 0..model.len() {
        let atteso = st.partition_point(|&s| s <= p as u64) as u64 - 1;
        assert_eq!(
            d.line_at_offset(p as u64),
            Some(atteso),
            "{what}: line_at_offset({p})"
        );
    }
    assert_eq!(
        d.line_at_offset(model.len() as u64),
        None,
        "{what}: pos == len"
    );
    assert_eq!(d.line_at_offset(u64::MAX), None, "{what}: pos u64::MAX");
}

/// Come `check_all` ma su `n` interrogazioni a caso (per i documenti grandi).
fn check_sampled(d: &mut Document, model: &[u8], r: &mut Lcg, n: usize, what: &str) {
    let st = starts(model);
    assert_eq!(d.line_count(), Some(st.len() as u64), "{what}: line_count");
    if st.is_empty() {
        return;
    }
    for _ in 0..n {
        let k = r.below(st.len() as u64);
        assert_eq!(
            d.offset_of_line(k),
            Some(st[k as usize]),
            "{what}: offset_of_line({k})"
        );
        let p = r.below(model.len() as u64);
        let atteso = st.partition_point(|&s| s <= p) as u64 - 1;
        assert_eq!(
            d.line_at_offset(p),
            Some(atteso),
            "{what}: line_at_offset({p})"
        );
    }
    assert_eq!(d.offset_of_line(st.len() as u64), None);
    assert_eq!(d.line_at_offset(model.len() as u64), None);
}

fn open_with_lines(dir: &TestDir, name: &str, data: &[u8], o: &BuildOptions) -> Document {
    let p = dir.file(name, data);
    let mut d = Document::open(&p).unwrap();
    let (c, prog) = flags();
    d.build_lines_with(o, &c, &prog).unwrap();
    assert!(d.has_lines());
    d
}

fn model_insert(m: &mut Vec<u8>, pos: usize, data: &[u8]) {
    m.splice(pos..pos, data.iter().copied());
}

fn model_delete(m: &mut Vec<u8>, pos: usize, len: usize) {
    m.drain(pos..pos + len);
}

// ---------- LineIndex::newlines_before ----------

#[test]
fn newlines_before_matches_a_naive_count_for_every_offset() {
    let dir = TestDir::new();
    for (seed, n) in [(1u64, 0usize), (2, 1), (3, 40), (4, 700), (5, 3000)] {
        let data = texty(&mut Lcg(seed), n);
        let p = dir.file(&format!("n{seed}.txt"), &data);
        let src = FileSource::open(&p).unwrap();
        for o in [
            opts(64, 4),
            opts(1, 1),
            opts(97, 1000),
            BuildOptions::default(),
        ] {
            let (c, prog) = flags();
            let idx = LineIndex::build_with(&src, &o, &c, &prog).unwrap();
            let mut atteso = 0u64;
            for off in 0..=data.len() {
                assert_eq!(
                    idx.newlines_before(&src, off as u64),
                    Some(atteso),
                    "seed {seed} n {n} opts {o:?} off {off}"
                );
                if off < data.len() && data[off] == b'\n' {
                    atteso += 1;
                }
            }
            assert_eq!(idx.newlines_before(&src, data.len() as u64 + 1), None);
            assert_eq!(idx.newlines_before(&src, u64::MAX), None);
        }
    }
}

// ---------- Document: stato dell'indice ----------

#[test]
fn without_the_index_every_query_is_none() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"a\nb\n");
    let mut d = Document::open(&p).unwrap();
    assert!(!d.has_lines());
    assert_eq!(d.line_count(), None);
    assert_eq!(d.offset_of_line(0), None);
    assert_eq!(d.line_at_offset(0), None);
}

#[test]
fn cancelled_build_leaves_no_index() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"a\nb\n");
    let mut d = Document::open(&p).unwrap();
    let c = AtomicBool::new(true);
    let prog = AtomicU64::new(0);
    assert!(d.build_lines(&c, &prog).is_err());
    assert!(!d.has_lines());
    assert_eq!(d.line_count(), None);
}

#[test]
fn small_documents_by_hand() {
    let dir = TestDir::new();
    // vuoto
    let mut d = open_with_lines(&dir, "e.txt", b"", &BuildOptions::default());
    assert_eq!(d.line_count(), Some(0));
    assert_eq!(d.offset_of_line(0), None);
    assert_eq!(d.line_at_offset(0), None);
    // solo un a-capo
    let mut d = open_with_lines(&dir, "n.txt", b"\n", &BuildOptions::default());
    assert_eq!(d.line_count(), Some(1));
    assert_eq!(d.offset_of_line(0), Some(0));
    assert_eq!(d.offset_of_line(1), None);
    assert_eq!(d.line_at_offset(0), Some(0));
    // senza a-capo finale
    let mut d = open_with_lines(&dir, "b.txt", b"ab\ncd", &BuildOptions::default());
    assert_eq!(d.line_count(), Some(2));
    assert_eq!(d.offset_of_line(1), Some(3));
    assert_eq!(d.line_at_offset(2), Some(0));
    assert_eq!(d.line_at_offset(3), Some(1));
    // con a-capo finale
    let mut d = open_with_lines(&dir, "c.txt", b"ab\ncd\n", &BuildOptions::default());
    assert_eq!(d.line_count(), Some(2));
    assert_eq!(d.offset_of_line(2), None);
    assert_eq!(d.line_at_offset(5), Some(1));
}

#[test]
fn unmodified_document_agrees_with_its_line_index() {
    let dir = TestDir::new();
    let data = texty(&mut Lcg(77), 2500);
    let mut d = open_with_lines(&dir, "a.txt", &data, &opts(64, 4));
    check_all(&mut d, &data, "non modificato");
}

// ---------- Document: modifiche a mano nei punti difficili ----------

#[test]
fn edits_at_the_difficult_points() {
    let dir = TestDir::new();
    let base: &[u8] = b"uno\ndue\ntre\nquattro\n";
    for o in [opts(4, 1), opts(5, 2), BuildOptions::default()] {
        // (descrizione, operazione) applicate in sequenza sullo stesso documento; dopo ognuna si confronta tutto.
        let mut d = open_with_lines(&dir, "h.txt", base, &o);
        let mut m = base.to_vec();
        // 1. si toglie un a-capo: due righe si fondono
        d.delete(3, 1).unwrap();
        model_delete(&mut m, 3, 1);
        check_all(&mut d, &m, "toglie un a-capo in mezzo");
        // 2. un a-capo inserito in mezzo a una riga la spezza
        d.insert(2, b"\n").unwrap();
        model_insert(&mut m, 2, b"\n");
        check_all(&mut d, &m, "spezza una riga");
        // 3. testo senza a-capo in fondo, dopo l'a-capo finale: nasce una riga non terminata
        let fine = m.len();
        d.insert(fine as u64, b"coda").unwrap();
        model_insert(&mut m, fine, b"coda");
        check_all(&mut d, &m, "riga non terminata in fondo");
        // 4. si aggiunge l'a-capo finale: la riga si chiude e non ne nasce una nuova
        let fine = m.len();
        d.insert(fine as u64, b"\n").unwrap();
        model_insert(&mut m, fine, b"\n");
        check_all(&mut d, &m, "a-capo finale");
        // 5. si toglie l'a-capo finale: l'ultima riga resta non terminata
        let fine = m.len();
        d.delete(fine as u64 - 1, 1).unwrap();
        model_delete(&mut m, fine - 1, 1);
        check_all(&mut d, &m, "toglie l'a-capo finale");
        // 6. si inserisce all'inizio
        d.insert(0, b"x\ny\n").unwrap();
        model_insert(&mut m, 0, b"x\ny\n");
        check_all(&mut d, &m, "inserisce all'inizio");
        // 7. si cancella un tratto che contiene piu' a-capo a cavallo di pezzi
        d.delete(1, m.len() as u64 - 3).unwrap();
        let l = m.len();
        model_delete(&mut m, 1, l - 3);
        check_all(&mut d, &m, "cancella a cavallo di pezzi");
        // 8. si cancella tutto, poi si scrive solo testo aggiunto
        d.delete(0, m.len() as u64).unwrap();
        m.clear();
        check_all(&mut d, &m, "tutto cancellato");
        d.insert(0, b"solo\naggiunte").unwrap();
        model_insert(&mut m, 0, b"solo\naggiunte");
        check_all(&mut d, &m, "solo testo aggiunto");
    }
}

#[test]
fn deleting_the_whole_original_keeps_the_index_but_the_document_is_empty() {
    let dir = TestDir::new();
    let mut d = open_with_lines(&dir, "a.txt", b"a\nb\nc\n", &opts(2, 1));
    d.delete(0, 6).unwrap();
    assert_eq!(d.line_count(), Some(0));
    assert!(d.has_lines());
    assert_eq!(d.offset_of_line(0), None);
    assert_eq!(d.line_at_offset(0), None);
}

// ---------- Document: modifiche casuali contro l'oracolo ----------

#[test]
fn random_edits_agree_with_the_oracle() {
    let dir = TestDir::new();
    for (seed, o) in [
        (11u64, opts(64, 4)),
        (12, opts(1, 1)),
        (13, opts(100, 3)),
        (14, BuildOptions::default()),
        (15, opts(33, 2)),
    ] {
        let mut r = Lcg(seed);
        let n = r.below(1500) as usize;
        let mut m = texty(&mut r, n);
        let mut d = open_with_lines(&dir, &format!("r{seed}.txt"), &m, &o);
        check_all(&mut d, &m, &format!("seed {seed} iniziale"));
        for step in 0..40 {
            if r.below(2) == 0 || m.is_empty() {
                let pos = r.below(m.len() as u64 + 1) as usize;
                let k = r.below(30) as usize;
                let data = texty(&mut r, k);
                d.insert(pos as u64, &data).unwrap();
                model_insert(&mut m, pos, &data);
            } else {
                let pos = r.below(m.len() as u64) as usize;
                let len = r.below((m.len() - pos) as u64 + 1).min(r.below(200) + 1) as usize;
                d.delete(pos as u64, len as u64).unwrap();
                model_delete(&mut m, pos, len);
            }
            check_all(&mut d, &m, &format!("seed {seed} passo {step}"));
        }
    }
}

#[test]
fn many_pieces_agree_with_the_oracle() {
    // 2000 inserimenti sparsi in un file da ~1 MiB: migliaia di pezzi; si interroga a campione.
    let dir = TestDir::new();
    let mut r = Lcg(2024);
    let mut m = texty(&mut r, 1_000_000);
    let mut d = open_with_lines(&dir, "m.txt", &m, &opts(64 * 1024, 256));
    for i in 0..2000 {
        let pos = r.below(m.len() as u64 + 1) as usize;
        let k = (r.below(6) + 1) as usize;
        let data = texty(&mut r, k);
        d.insert(pos as u64, &data).unwrap();
        model_insert(&mut m, pos, &data);
        if i % 400 == 0 {
            check_sampled(&mut d, &m, &mut r, 300, &format!("inserimento {i}"));
        }
    }
    assert!(d.table().pieces().len() > 3000, "il test vuole molti pezzi");
    check_sampled(&mut d, &m, &mut r, 1000, "dopo 2000 inserimenti");
    // e qualche cancellazione larga
    for i in 0..50 {
        let pos = r.below(m.len() as u64) as usize;
        let len = (r.below(5000) + 1).min((m.len() - pos) as u64) as usize;
        d.delete(pos as u64, len as u64).unwrap();
        model_delete(&mut m, pos, len);
        if i % 10 == 0 {
            check_sampled(&mut d, &m, &mut r, 300, &format!("cancellazione {i}"));
        }
    }
    check_sampled(&mut d, &m, &mut r, 1000, "dopo le cancellazioni");
}

// ---------- Document: interazione con il salvataggio ----------

#[test]
fn save_invalidates_the_index_and_a_rebuild_agrees_with_the_new_file() {
    let dir = TestDir::new();
    let mut d = open_with_lines(&dir, "s.txt", b"uno\ndue\ntre\n", &opts(4, 1));
    d.insert(4, b"nuova\nriga\n").unwrap();
    d.delete(0, 2).unwrap();
    let (c, prog) = flags();
    d.save(&c, &prog).unwrap();
    assert!(
        !d.has_lines(),
        "dopo il salvataggio l'indice non e' piu' valido"
    );
    assert_eq!(d.line_count(), None);
    let (c, prog) = flags();
    d.build_lines(&c, &prog).unwrap();
    let on_disk = std::fs::read(d.path()).unwrap();
    check_all(&mut d, &on_disk, "dopo salvataggio e ricostruzione");
}

#[test]
fn save_without_changes_keeps_the_index() {
    let dir = TestDir::new();
    let mut d = open_with_lines(&dir, "s.txt", b"uno\ndue\n", &BuildOptions::default());
    let (c, prog) = flags();
    d.save(&c, &prog).unwrap();
    assert!(d.has_lines(), "nessuna modifica: il file non e' cambiato");
    assert_eq!(d.line_count(), Some(2));
}

#[test]
fn a_failed_save_keeps_the_index() {
    let dir = TestDir::new();
    let p = dir.file("s.txt", b"uno\ndue\n");
    let mut d = Document::open(&p).unwrap();
    let (c, prog) = flags();
    d.build_lines(&c, &prog).unwrap();
    d.insert(0, b"x\n").unwrap();
    let c2 = AtomicBool::new(true); // annullato in partenza
    assert!(d.save(&c2, &prog).is_err());
    assert!(d.has_lines());
    check_all(&mut d, b"x\nuno\ndue\n", "dopo un salvataggio annullato");
}

// ---------- oltre 4 GiB (a mano) ----------

/// Serve una COPIA di un file prodotto da `bench/gen_testfile.py` (oltre 4 GiB, ~60 M righe).
/// Il test non salva: il file non viene modificato. Esempio:
///   MRP_BIG_COPY=D:\bench\big6g.txt cargo test --release -p mrp-core --test doclines -- --ignored --nocapture
#[test]
#[ignore = "serve MRP_BIG_COPY=<file da bench/gen_testfile.py, oltre 4 GiB>"]
fn big_file_lines_follow_the_edits_beyond_4gib() {
    let path = std::env::var("MRP_BIG_COPY").expect("imposta MRP_BIG_COPY");
    let mut d = Document::open(Path::new(&path)).unwrap();
    assert!(
        d.len() > 4 * 1024 * 1024 * 1024,
        "il file deve superare 4 GiB"
    );
    let (c, prog) = flags();
    d.build_lines(&c, &prog).unwrap();
    let n = d.line_count().unwrap();
    let riga = 50_000_000u64;
    assert!(riga + 10 < n);
    let a = d.offset_of_line(riga).unwrap();
    assert!(
        a > 4 * 1024 * 1024 * 1024,
        "la riga scelta deve cadere oltre 4 GiB"
    );
    let b = d.offset_of_line(riga + 1).unwrap();
    let prima = d.read_range(a, b - a);
    // due righe nuove davanti alla riga scelta
    d.insert(a, b"X\nY\n").unwrap();
    assert_eq!(d.line_count(), Some(n + 2));
    assert_eq!(d.offset_of_line(riga), Some(a));
    assert_eq!(d.offset_of_line(riga + 2), Some(a + 4));
    assert_eq!(d.line_at_offset(a + 4), Some(riga + 2));
    assert_eq!(d.read_range(a + 4, b - a), prima);
    // si cancella la riga scelta intera: le successive tornano indietro di una
    d.delete(a + 4, b - a).unwrap();
    assert_eq!(d.line_count(), Some(n + 1));
    assert_eq!(d.offset_of_line(riga + 2), Some(a + 4));
    assert_eq!(d.line_at_offset(a + 4), Some(riga + 2));
    // l'ultima riga (indice n, perche' ora le righe sono n + 1) contiene l'ultimo byte
    assert_eq!(d.line_at_offset(d.len() - 1), Some(n));
    assert_eq!(d.offset_of_line(n + 1), None);
}

// ---------- indice costruito altrove ----------

#[test]
fn an_index_built_from_a_second_source_can_be_installed() {
    let dir = TestDir::new();
    let mut r = Lcg(5);
    let mut m = texty(&mut r, 3000);
    let p = dir.file("i.txt", &m);
    let mut d = Document::open(&p).unwrap();
    // come fa l'interfaccia: un thread costruisce l'indice da una SUA FileSource sullo stesso file
    let p2 = p.clone();
    let idx = std::thread::spawn(move || {
        let src = FileSource::open(&p2).unwrap();
        let (c, prog) = flags();
        LineIndex::build_with(&src, &opts(64, 4), &c, &prog).unwrap()
    })
    .join()
    .unwrap();
    assert!(!d.has_lines());
    d.insert(10, b"x\ny\n").unwrap(); // una modifica prima dell'installazione
    model_insert(&mut m, 10, b"x\ny\n");
    d.install_lines(idx);
    assert!(d.has_lines());
    check_all(&mut d, &m, "indice installato dopo una modifica");
}
