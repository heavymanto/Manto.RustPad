//! Test del salvataggio. Scritti da ARCH prima dell'implementazione: gli attesi vengono da un oracolo
//! ingenuo (un `Vec<u8>` su cui si fa splice/drain) o da calcoli a mano, mai dall'output del codice.

use mrp_core::edit::{Buf, Piece};
use mrp_core::{Document, SaveError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Cartella temporanea propria del test (cosi' si vede se resta qualche file temporaneo), rimossa alla fine.
struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_save_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    /// Crea `nome` con `data` e ne restituisce il percorso.
    fn file(&self, nome: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(nome);
        std::fs::write(&p, data).unwrap();
        p
    }
    /// Nomi dei file presenti nella cartella, ordinati.
    fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
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

fn save_ok(d: &mut Document) {
    let (c, p) = flags();
    d.save(&c, &p).unwrap();
}

fn whole(d: &Document) -> Vec<u8> {
    d.read_range(0, d.len())
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
}

/// Contenuto pseudocasuale deterministico di `n` byte.
fn random_bytes(seed: u64, n: usize) -> Vec<u8> {
    let mut r = Lcg(seed);
    (0..n).map(|_| (r.next() & 0xff) as u8).collect()
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap()
}

// ---------- apertura e modifica ----------

#[test]
fn open_reads_the_file_and_is_not_modified() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"hello world");
    let d = Document::open(&p).unwrap();
    assert_eq!(d.len(), 11);
    assert!(!d.is_empty());
    assert!(!d.is_modified());
    assert_eq!(whole(&d), b"hello world");
    assert_eq!(d.read_range(6, 100), b"world");
    assert_eq!(
        d.table().pieces(),
        &[Piece {
            buf: Buf::Original,
            start: 0,
            len: 11
        }]
    );
    assert!(d.path().is_absolute());
}

#[test]
fn open_fails_for_missing_file_and_for_a_directory() {
    let dir = TestDir::new();
    assert!(Document::open(&dir.0.join("non_esiste")).is_err());
    assert!(Document::open(&dir.0).is_err());
}

#[test]
fn edits_mark_the_document_modified_only_when_they_change_something() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"0123456789");
    let mut d = Document::open(&p).unwrap();
    d.insert(3, b"").unwrap();
    d.delete(3, 0).unwrap();
    assert!(!d.is_modified());
    assert!(d.insert(99, b"x").is_err());
    assert!(d.delete(8, 5).is_err());
    assert!(!d.is_modified(), "un errore non modifica il documento");
    d.insert(3, b"XY").unwrap();
    assert!(d.is_modified());
    assert_eq!(whole(&d), b"012XY3456789");
    assert_eq!(d.len(), 12);
    // il file su disco non e' stato toccato
    assert_eq!(read(&p), b"0123456789");
}

// ---------- salvataggio: contenuto ----------

#[test]
fn save_writes_exactly_the_edited_content() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"0123456789");
    let mut d = Document::open(&p).unwrap();
    d.insert(4, b"XYZ").unwrap();
    d.delete(0, 2).unwrap();
    let (c, prog) = flags();
    d.save(&c, &prog).unwrap();
    assert_eq!(read(&p), b"23XYZ456789");
    assert_eq!(prog.load(Ordering::SeqCst), 11);
    assert!(!d.is_modified());
    assert_eq!(d.len(), 11);
    assert_eq!(whole(&d), b"23XYZ456789");
    assert_eq!(
        d.table().pieces(),
        &[Piece {
            buf: Buf::Original,
            start: 0,
            len: 11
        }]
    );
    assert_eq!(
        dir.names(),
        vec!["a.txt".to_string()],
        "nessun temporaneo rimasto"
    );
}

#[test]
fn several_save_and_edit_rounds_keep_working() {
    // Dopo il primo salvataggio la mappatura e' stata rilasciata e riaperta: il secondo giro lo prova.
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"abc");
    let mut d = Document::open(&p).unwrap();
    d.insert(3, b"def").unwrap();
    save_ok(&mut d);
    assert_eq!(read(&p), b"abcdef");
    d.delete(0, 1).unwrap();
    d.insert(0, b"ZZ").unwrap();
    save_ok(&mut d);
    assert_eq!(read(&p), b"ZZbcdef");
    d.insert(7, b"!").unwrap();
    save_ok(&mut d);
    assert_eq!(read(&p), b"ZZbcdef!");
    assert_eq!(whole(&d), b"ZZbcdef!");
    assert_eq!(dir.names(), vec!["a.txt".to_string()]);
}

#[test]
fn save_of_an_unmodified_document_does_nothing() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"abc");
    let before = std::fs::metadata(&p).unwrap().modified().unwrap();
    let mut d = Document::open(&p).unwrap();
    let (c, prog) = flags();
    d.save(&c, &prog).unwrap();
    assert_eq!(prog.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), before);
    assert_eq!(read(&p), b"abc");
    assert_eq!(dir.names(), vec!["a.txt".to_string()]);
}

#[test]
fn empty_original_and_deleting_everything() {
    let dir = TestDir::new();
    let p = dir.file("e.txt", b"");
    let mut d = Document::open(&p).unwrap();
    assert!(d.is_empty());
    d.insert(0, b"nuovo").unwrap();
    save_ok(&mut d);
    assert_eq!(read(&p), b"nuovo");
    d.delete(0, 5).unwrap();
    assert!(d.is_empty());
    save_ok(&mut d);
    assert_eq!(read(&p), b"");
    assert_eq!(std::fs::metadata(&p).unwrap().len(), 0);
    d.insert(0, b"di nuovo").unwrap();
    save_ok(&mut d);
    assert_eq!(read(&p), b"di nuovo");
}

#[test]
fn multi_chunk_file_matches_the_naive_model() {
    // 10 MiB: piu' di due blocchi di salvataggio (4 MiB) e pezzi che li attraversano.
    let dir = TestDir::new();
    let orig = random_bytes(7, 10 * 1024 * 1024);
    let p = dir.file("big.bin", &orig);
    let mut d = Document::open(&p).unwrap();
    let mut model = orig.clone();

    let ins = random_bytes(8, 300_000);
    d.insert(3_000_000, &ins).unwrap();
    model.splice(3_000_000..3_000_000, ins.iter().copied());
    d.delete(5_000_000, 2_500_000).unwrap();
    model.drain(5_000_000..7_500_000);
    let tail = random_bytes(9, 17);
    let end = d.len();
    d.insert(end, &tail).unwrap();
    model.extend_from_slice(&tail);
    d.delete(0, 123).unwrap();
    model.drain(0..123);

    let (c, prog) = flags();
    d.save(&c, &prog).unwrap();
    assert_eq!(prog.load(Ordering::SeqCst), model.len() as u64);
    assert!(read(&p) == model, "il file salvato differisce dal modello");
    assert!(
        whole(&d) == model,
        "il documento riaperto differisce dal modello"
    );
}

#[test]
fn small_edit_in_front_of_a_big_original_piece() {
    // Il caso piu' comune: una modifica piccola all'inizio di un file grande lascia un solo pezzo `Original`
    // di 10 MiB, che il salvataggio deve leggere in 3 blocchi da 4 MiB con le posizioni giuste.
    let dir = TestDir::new();
    let orig = random_bytes(41, 10 * 1024 * 1024);
    let p = dir.file("a.bin", &orig);
    let mut d = Document::open(&p).unwrap();
    d.insert(5, b"ABC").unwrap();
    let mut model = orig.clone();
    model.splice(5..5, *b"ABC");
    save_ok(&mut d);
    assert!(read(&p) == model, "il file salvato differisce dal modello");
}

#[test]
fn large_insert_crosses_several_save_chunks() {
    // 9 MiB inseriti in un solo pezzo `Add`: il salvataggio lo scrive in 3 blocchi da 4 MiB.
    let dir = TestDir::new();
    let orig = random_bytes(31, 1_000_000);
    let p = dir.file("a.bin", &orig);
    let mut d = Document::open(&p).unwrap();
    let ins = random_bytes(32, 9 * 1024 * 1024);
    d.insert(400_000, &ins).unwrap();
    let mut model = orig.clone();
    model.splice(400_000..400_000, ins.iter().copied());
    save_ok(&mut d);
    assert!(read(&p) == model, "il file salvato differisce dal modello");
    assert!(whole(&d) == model);
}

#[test]
fn random_edit_save_rounds_match_the_naive_model() {
    let dir = TestDir::new();
    let mut model = random_bytes(21, 20_000);
    let p = dir.file("r.bin", &model);
    let mut d = Document::open(&p).unwrap();
    let mut rng = Lcg(5);
    for round in 0..6 {
        for _ in 0..40 {
            if rng.next() & 1 == 0 || model.is_empty() {
                let pos = (rng.next() as usize) % (model.len() + 1);
                let data = random_bytes(rng.next(), (rng.next() % 50) as usize);
                d.insert(pos as u64, &data).unwrap();
                model.splice(pos..pos, data.iter().copied());
            } else {
                let pos = (rng.next() as usize) % model.len();
                let n = ((rng.next() % 300) as usize).min(model.len() - pos);
                d.delete(pos as u64, n as u64).unwrap();
                model.drain(pos..pos + n);
            }
        }
        assert!(whole(&d) == model, "giro {round} prima del salvataggio");
        save_ok(&mut d);
        assert!(read(&p) == model, "giro {round} file");
        assert!(whole(&d) == model, "giro {round} dopo il salvataggio");
        assert_eq!(
            dir.names().len(),
            1,
            "giro {round}: file temporanei rimasti"
        );
    }
}

// ---------- salvataggio: fallimenti che non devono toccare nulla ----------

#[test]
fn cancel_before_start_leaves_everything_untouched() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"0123456789");
    let mut d = Document::open(&p).unwrap();
    d.insert(4, b"XYZ").unwrap();
    let c = AtomicBool::new(true);
    let prog = AtomicU64::new(0);
    let r = d.save(&c, &prog);
    assert!(matches!(r, Err(SaveError::Cancelled)), "{r:?}");
    assert_eq!(read(&p), b"0123456789");
    assert_eq!(dir.names(), vec!["a.txt".to_string()]);
    assert!(d.is_modified());
    assert_eq!(whole(&d), b"0123XYZ456789");
    // e dopo l'annullamento si puo' salvare
    save_ok(&mut d);
    assert_eq!(read(&p), b"0123XYZ456789");
}

#[test]
fn cancel_during_a_large_save_leaves_the_original_and_no_temp_file() {
    // 64 MiB = 16 blocchi. Un altro thread annulla appena vede avanzare `progress`.
    // L'esito puo' essere Ok (finito prima dell'annullamento) o Cancelled: in tutti e due i casi
    // il file e la cartella devono essere coerenti.
    let dir = TestDir::new();
    let orig = random_bytes(3, 64 * 1024 * 1024);
    let p = dir.file("big.bin", &orig);
    let mut d = Document::open(&p).unwrap();
    d.insert(10, b"HELLO").unwrap();
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let prog = std::sync::Arc::new(AtomicU64::new(0));
    let (c2, p2) = (cancel.clone(), prog.clone());
    let watcher = std::thread::spawn(move || {
        while p2.load(Ordering::SeqCst) == 0 {
            std::thread::yield_now();
        }
        c2.store(true, Ordering::SeqCst);
    });
    let r = d.save(&cancel, &prog);
    let written = prog.load(Ordering::SeqCst); // byte scritti quando save e' tornato
    cancel.store(true, Ordering::SeqCst);
    prog.fetch_add(1, Ordering::SeqCst); // sblocca il watcher se il salvataggio non e' mai partito
    watcher.join().unwrap();
    match r {
        Err(SaveError::Cancelled) => {
            // l'annullamento deve fermare il lavoro presto, non a file quasi finito
            assert!(
                written < orig.len() as u64,
                "annullato ma scritti {written} byte su {}",
                orig.len() + 5
            );
            assert!(read(&p) == orig, "l'originale e' stato toccato");
            assert_eq!(dir.names(), vec!["big.bin".to_string()]);
            assert!(d.is_modified());
            assert_eq!(
                d.read_range(8, 9),
                [&orig[8..10], b"HELLO".as_slice(), &orig[10..12]].concat()
            );
        }
        Ok(()) => {
            let saved = read(&p);
            assert_eq!(saved.len(), orig.len() + 5);
            assert_eq!(&saved[10..15], b"HELLO");
            assert!(!d.is_modified());
            assert_eq!(dir.names(), vec!["big.bin".to_string()]);
        }
        Err(e) => panic!("esito inatteso: {e}"),
    }
}

#[test]
fn source_changed_by_length_is_refused_and_not_overwritten() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"0123456789");
    let mut d = Document::open(&p).unwrap();
    d.insert(2, b"XX").unwrap();
    // qualcun altro allunga il file (in coda: su Windows un file mappato non si puo' troncare)
    let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
    std::io::Write::write_all(&mut f, b"-altrui").unwrap();
    drop(f);
    let (c, prog) = flags();
    let r = d.save(&c, &prog);
    assert!(matches!(r, Err(SaveError::SourceChanged)), "{r:?}");
    assert_eq!(
        read(&p),
        b"0123456789-altrui",
        "il lavoro altrui non va sovrascritto"
    );
    assert_eq!(dir.names(), vec!["a.txt".to_string()]);
    assert!(d.is_modified());
    assert_eq!(whole(&d).len(), 12, "le modifiche restano nel documento");
}

#[test]
fn source_changed_by_time_only_is_refused() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"0123456789");
    let mut d = Document::open(&p).unwrap();
    d.insert(2, b"XX").unwrap();
    // stessa lunghezza, data di modifica diversa (+10 s)
    let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
    let t = std::fs::metadata(&p).unwrap().modified().unwrap() + std::time::Duration::from_secs(10);
    f.set_modified(t).unwrap();
    drop(f);
    let (c, prog) = flags();
    let r = d.save(&c, &prog);
    assert!(matches!(r, Err(SaveError::SourceChanged)), "{r:?}");
    assert_eq!(read(&p), b"0123456789");
    assert_eq!(dir.names(), vec!["a.txt".to_string()]);
}

#[test]
fn deleted_source_is_reported_as_changed() {
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"0123456789");
    let mut d = Document::open(&p).unwrap();
    d.insert(2, b"XX").unwrap();
    std::fs::remove_file(&p).unwrap();
    let (c, prog) = flags();
    let r = d.save(&c, &prog);
    assert!(matches!(r, Err(SaveError::SourceChanged)), "{r:?}");
    assert!(
        dir.names().is_empty(),
        "non deve ricreare il file ne' lasciare temporanei"
    );
}

// ---------- permessi (solo Unix) ----------

#[cfg(unix)]
#[test]
fn permissions_are_preserved() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TestDir::new();
    let p = dir.file("a.txt", b"abc");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o640)).unwrap();
    let mut d = Document::open(&p).unwrap();
    d.insert(3, b"d").unwrap();
    save_ok(&mut d);
    let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640);
}

// ---------- file oltre 4 GiB (si lancia a mano, scrive su disco!) ----------

/// `MRP_BIG_COPY` = COPIA di un file generato da `bench/gen_testfile.py` (righe da 100 byte), perche' il test lo
/// riscrive. Esempio:  MRP_BIG_COPY=D:\bench\big6g_copy.txt cargo test --release -p mrp-core --test save -- --ignored --nocapture
#[test]
#[ignore = "serve MRP_BIG_COPY=<copia di un file da bench/gen_testfile.py, oltre 4 GiB>"]
fn big_file_save_beyond_4gib() {
    let path = std::env::var("MRP_BIG_COPY").expect("imposta MRP_BIG_COPY");
    let mut d = Document::open(Path::new(&path)).unwrap();
    let n = d.len();
    assert!(n > 4 * 1024 * 1024 * 1024, "il file deve superare 4 GiB");
    let at = 4_500_000_000u64;
    let before_at = d.read_range(at - 50, 100); // 50 byte prima e 50 dopo il punto di inserimento
    let last = d.read_range(n - 100, 100);
    let first = d.read_range(0, 100);
    d.insert(at, b"<<MRP>>").unwrap();
    d.delete(100, 100).unwrap(); // toglie la seconda riga
    let (c, prog) = flags();
    d.save(&c, &prog).unwrap();
    assert_eq!(prog.load(Ordering::SeqCst), n + 7 - 100);
    assert_eq!(d.len(), n + 7 - 100);
    assert_eq!(std::fs::metadata(&path).unwrap().len(), n + 7 - 100);
    let at2 = at - 100; // per la riga tolta
    assert_eq!(d.read_range(0, 100), first);
    assert_eq!(d.read_range(at2, 7), b"<<MRP>>");
    assert_eq!(d.read_range(at2 - 50, 50), before_at[..50]);
    assert_eq!(d.read_range(at2 + 7, 50), before_at[50..]);
    assert_eq!(d.read_range(d.len() - 100, 100), last);
}
