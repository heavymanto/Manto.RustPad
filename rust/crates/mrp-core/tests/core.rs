//! Test del core. Scritti da ARCH prima dell'implementazione: i valori attesi vengono da calcoli a mano
//! o da oracoli ingenui (cicli semplici) scritti qui, mai dall'output del codice sotto test.

use mrp_core::{
    find_next, find_prev, BuildOptions, Cancelled, FileSource, LineIndex, SearchOptions,
    SearchResult,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// File temporaneo cancellato alla fine del test.
struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_core_test_{}_{}.bin", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
    /// File temporaneo scritto a flusso: il contenuto non sta mai tutto in memoria.
    fn stream(write: impl FnOnce(&mut std::io::BufWriter<std::fs::File>)) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_core_test_{}_{}.bin", std::process::id(), n));
        let mut w = std::io::BufWriter::new(std::fs::File::create(&p).unwrap());
        write(&mut w);
        std::io::Write::flush(&mut w).unwrap();
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

/// Scrive `lines` righe da esattamente 100 byte: `L{n:07} ` + 90 'x' + `\n` (n ripete ogni 1.000.000).
fn write_100b_lines(w: &mut impl std::io::Write, lines: u64) {
    let pay = "x".repeat(90);
    for i in 0..lines {
        writeln!(w, "L{:07} {pay}", i % 1_000_000).unwrap();
    }
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

/// Byte casuali con righe di lunghezza 0..59 (comprese righe vuote), alfabeto "abc \r" piu' `\n`.
fn random_lines(seed: u64, total: usize) -> Vec<u8> {
    let mut rng = Lcg(seed);
    let alphabet = b"abc \r";
    let mut v = Vec::with_capacity(total + 64);
    while v.len() < total {
        let n = (rng.next() % 60) as usize;
        for _ in 0..n {
            v.push(alphabet[(rng.next() % alphabet.len() as u64) as usize]);
        }
        v.push(b'\n');
    }
    v.truncate(total);
    v
}

/// Oracolo: posizione di inizio di ogni riga, con la convenzione del progetto.
fn naive_line_starts(data: &[u8]) -> Vec<u64> {
    let mut v = Vec::new();
    if data.is_empty() {
        return v;
    }
    v.push(0);
    for (i, &b) in data.iter().enumerate() {
        if b == b'\n' && i + 1 < data.len() {
            v.push((i + 1) as u64);
        }
    }
    v
}

fn build(src: &FileSource, chunk_bytes: usize, checkpoint_every: u32) -> LineIndex {
    LineIndex::build_with(
        src,
        &BuildOptions {
            chunk_bytes,
            checkpoint_every,
        },
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    )
    .unwrap()
}

fn fnext(src: &FileSource, pat: &[u8], from: u64, chunk: usize) -> SearchResult {
    find_next(
        src,
        pat,
        from,
        &SearchOptions { chunk_bytes: chunk },
        &AtomicBool::new(false),
    )
}

fn fprev(src: &FileSource, pat: &[u8], before: u64, chunk: usize) -> SearchResult {
    find_prev(
        src,
        pat,
        before,
        &SearchOptions { chunk_bytes: chunk },
        &AtomicBool::new(false),
    )
}

// ---------- FileSource ----------

#[test]
fn source_open_missing_file_is_error() {
    let p = std::env::temp_dir().join("mrp_core_test_non_esiste_di_sicuro.bin");
    assert!(FileSource::open(&p).is_err());
}

#[test]
fn source_open_directory_is_error() {
    assert!(FileSource::open(&std::env::temp_dir()).is_err());
}

#[test]
fn source_empty_file() {
    let f = TempFile::new(b"");
    let s = f.open();
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
    assert_eq!(s.bytes(), b"");
    assert_eq!(s.read_range(0, 10), b"");
}

#[test]
fn source_bytes_and_read_range() {
    let f = TempFile::new(b"0123456789");
    let s = f.open();
    assert_eq!(s.len(), 10);
    assert!(!s.is_empty());
    assert_eq!(s.bytes(), b"0123456789");
    assert_eq!(s.read_range(2, 3), b"234");
    assert_eq!(s.read_range(8, 10), b"89"); // accorciato
    assert_eq!(s.read_range(10, 1), b""); // proprio alla fine
    assert_eq!(s.read_range(11, 1), b""); // oltre la fine
    assert_eq!(s.read_range(u64::MAX, 5), b"");
    assert_eq!(s.read_range(0, usize::MAX), b"0123456789");
    assert_eq!(s.read_range(3, 0), b"");
}

// ---------- LineIndex: casi piccoli a mano ----------

fn check_lines(data: &[u8], starts: &[u64]) {
    let f = TempFile::new(data);
    let s = f.open();
    for (chunk, every) in [(1usize, 1u32), (2, 3), (4096, 256)] {
        let idx = build(&s, chunk, every);
        assert_eq!(
            idx.line_count(),
            starts.len() as u64,
            "{data:?} chunk {chunk}"
        );
        for (i, &st) in starts.iter().enumerate() {
            assert_eq!(
                idx.offset_of_line(&s, i as u64),
                Some(st),
                "{data:?} riga {i}"
            );
        }
        assert_eq!(idx.offset_of_line(&s, starts.len() as u64), None);
        assert_eq!(idx.offset_of_line(&s, u64::MAX), None);
        assert_eq!(idx.line_at_offset(&s, data.len() as u64), None);
        assert_eq!(idx.line_at_offset(&s, u64::MAX), None);
    }
}

#[test]
fn lines_empty_file() {
    check_lines(b"", &[]);
}

#[test]
fn lines_with_trailing_newline() {
    // "a\nb\nc\n" -> 3 righe a 0, 2, 4
    check_lines(b"a\nb\nc\n", &[0, 2, 4]);
}

#[test]
fn lines_without_trailing_newline() {
    // "a\nbc" -> 2 righe a 0, 2
    check_lines(b"a\nbc", &[0, 2]);
}

#[test]
fn lines_crlf_keeps_cr_inside_line() {
    // "a\r\nb\r\n" -> 2 righe a 0 e 3
    check_lines(b"a\r\nb\r\n", &[0, 3]);
}

#[test]
fn lines_only_newlines() {
    // "\n\n\n" -> 3 righe vuote a 0, 1, 2
    check_lines(b"\n\n\n", &[0, 1, 2]);
}

#[test]
fn lines_single_line_without_newline() {
    check_lines(b"abc", &[0]);
}

#[test]
fn lines_line_at_offset_small() {
    let f = TempFile::new(b"a\nb\nc\n");
    let s = f.open();
    let idx = build(&s, 2, 1);
    // byte: a(0) \n(1) b(2) \n(3) c(4) \n(5)
    let expected = [0u64, 0, 1, 1, 2, 2];
    for (off, &line) in expected.iter().enumerate() {
        assert_eq!(
            idx.line_at_offset(&s, off as u64),
            Some(line),
            "offset {off}"
        );
    }
}

// ---------- LineIndex: confronto con l'oracolo ingenuo ----------

#[test]
fn lines_match_naive_oracle_for_many_options() {
    for seed in [1u64, 2, 3] {
        for &chunk in &[1usize, 7, 64, 4096, 1 << 20] {
            let size = if chunk == 1 { 3_000 } else { 50_000 };
            let data = random_lines(seed, size);
            let starts = naive_line_starts(&data);
            let f = TempFile::new(&data);
            let s = f.open();
            for &every in &[1u32, 3, 256] {
                let idx = build(&s, chunk, every);
                assert_eq!(
                    idx.line_count(),
                    starts.len() as u64,
                    "seed {seed} chunk {chunk} every {every}"
                );
                for (i, &st) in starts.iter().enumerate() {
                    assert_eq!(
                        idx.offset_of_line(&s, i as u64),
                        Some(st),
                        "seed {seed} chunk {chunk} every {every} riga {i}"
                    );
                }
                assert_eq!(idx.offset_of_line(&s, starts.len() as u64), None);
                // line_at_offset: ogni 13 byte, piu' i byte attorno a ogni inizio di riga
                let mut offs: Vec<u64> = (0..data.len() as u64).step_by(13).collect();
                for &st in &starts {
                    for d in [st.saturating_sub(1), st, st + 1] {
                        if d < data.len() as u64 {
                            offs.push(d);
                        }
                    }
                }
                for off in offs {
                    let want = starts.partition_point(|&x| x <= off) as u64 - 1;
                    assert_eq!(
                        idx.line_at_offset(&s, off),
                        Some(want),
                        "seed {seed} chunk {chunk} every {every} offset {off}"
                    );
                }
            }
        }
    }
}

#[test]
fn lines_index_is_sparse_and_exact_on_100k_lines() {
    // righe "line0\n", "line1\n", ... con posizioni calcolate a mano nel test
    let mut data = Vec::new();
    let mut starts = Vec::new();
    for i in 0..100_000u64 {
        starts.push(data.len() as u64);
        data.extend_from_slice(format!("line{i}\n").as_bytes());
    }
    let f = TempFile::new(&data);
    let s = f.open();
    let chunk = 1 << 20;
    let idx = build(&s, chunk, 256);
    assert_eq!(idx.line_count(), 100_000);
    for k in [0u64, 1, 255, 256, 257, 1023, 1024, 50_000, 99_999] {
        assert_eq!(
            idx.offset_of_line(&s, k),
            Some(starts[k as usize]),
            "riga {k}"
        );
        assert_eq!(idx.line_at_offset(&s, starts[k as usize]), Some(k));
    }
    let chunks = data.len() / chunk + 1;
    assert!(
        idx.checkpoint_count() <= 100_000 / 256 + chunks + 2,
        "troppi punti: {}",
        idx.checkpoint_count()
    );
    assert!(idx.checkpoint_count() < 1000, "l'indice non e' sparso");
}

#[test]
fn lines_checkpoint_count_is_exact() {
    // Ogni blocco memorizza un punto ogni `every` inizi di riga, partendo dal primo:
    // quindi per blocco ceil(inizi_nel_blocco / every). Il numero dei punti e' esatto, non solo limitato.
    for seed in [11u64, 12] {
        let data = random_lines(seed, 20_000);
        let starts = naive_line_starts(&data);
        let f = TempFile::new(&data);
        let s = f.open();
        for &chunk in &[64usize, 1000, 4096, 1 << 20] {
            for &every in &[1u32, 3, 7, 256] {
                let idx = build(&s, chunk, every);
                let mut expected = 0usize;
                let mut a = 0u64;
                while a < data.len() as u64 {
                    let b = (a + chunk as u64).min(data.len() as u64);
                    let n = starts.iter().filter(|&&p| p >= a && p < b).count();
                    expected += n.div_ceil(every as usize);
                    a = b;
                }
                assert_eq!(
                    idx.checkpoint_count(),
                    expected,
                    "seed {seed} chunk {chunk} every {every}"
                );
            }
        }
    }
}

#[test]
fn lines_large_file_with_prefetch_path_is_exact() {
    // 700.000 righe da 100 byte = 70.000.000 byte (> 64 MiB): attiva il precaricamento sequenziale.
    let f = TempFile::stream(|w| write_100b_lines(w, 700_000));
    let s = f.open();
    assert_eq!(s.len(), 70_000_000);
    let progress = AtomicU64::new(0);
    let idx = LineIndex::build(&s, &AtomicBool::new(false), &progress).unwrap();
    assert_eq!(idx.line_count(), 700_000);
    assert_eq!(progress.load(Ordering::SeqCst), 70_000_000);
    for k in [0u64, 1, 255, 256, 41_942, 349_999, 699_999] {
        assert_eq!(idx.offset_of_line(&s, k), Some(k * 100), "riga {k}");
        assert_eq!(idx.line_at_offset(&s, k * 100 + 99), Some(k));
    }
    assert_eq!(idx.offset_of_line(&s, 700_000), None);
    // annullato prima di cominciare: deve tornare subito, senza restare in attesa del precaricamento
    let r = LineIndex::build(&s, &AtomicBool::new(true), &AtomicU64::new(0));
    assert!(matches!(r, Err(Cancelled)));
}

/// Esegue `build_with` in un thread e aspetta al massimo `secs` secondi: se non finisce, annulla
/// (cosi' il thread si libera) e il test fallisce invece di restare appeso per sempre.
fn build_or_fail_on_hang(
    s: std::sync::Arc<FileSource>,
    opts: BuildOptions,
    cancel: std::sync::Arc<AtomicBool>,
    secs: u64,
) -> Result<u64, Cancelled> {
    let (tx, rx) = std::sync::mpsc::channel();
    let c = cancel.clone();
    std::thread::spawn(move || {
        let r = LineIndex::build_with(&s, &opts, &c, &AtomicU64::new(0));
        let _ = tx.send(r.map(|i| i.line_count()));
    });
    match rx.recv_timeout(std::time::Duration::from_secs(secs)) {
        Ok(r) => r,
        Err(_) => {
            cancel.store(true, Ordering::SeqCst);
            panic!("build_with non termina entro {secs} s (attesa reciproca?)");
        }
    }
}

#[test]
fn lines_huge_chunk_with_prefetch_does_not_deadlock() {
    // File > 264 MiB con un blocco piu' grande del vantaggio massimo del precaricamento:
    // regressione di un'attesa reciproca trovata in review.
    let f = TempFile::stream(|w| write_100b_lines(w, 3_000_000)); // 300.000.000 byte
    let s = std::sync::Arc::new(f.open());
    let opts = BuildOptions {
        chunk_bytes: 280 * 1024 * 1024,
        checkpoint_every: 256,
    };
    let r = build_or_fail_on_hang(s, opts, std::sync::Arc::new(AtomicBool::new(false)), 120);
    assert_eq!(r, Ok(3_000_000));
}

#[test]
fn lines_cancel_during_large_build_returns_promptly() {
    let f = TempFile::stream(|w| write_100b_lines(w, 700_000)); // 70.000.000 byte: usa il precaricamento
    let s = std::sync::Arc::new(f.open());
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let c = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2));
        c.store(true, Ordering::SeqCst);
    });
    // L'esito puo' essere Ok (finito prima) o Err(Cancelled): conta che ritorni.
    let r = build_or_fail_on_hang(s, BuildOptions::default(), cancel, 60);
    assert!(r == Ok(700_000) || r == Err(Cancelled), "{r:?}");
}

#[test]
fn lines_empty_file_with_cancel_already_set_is_cancelled() {
    let f = TempFile::new(b"");
    let s = f.open();
    let r = LineIndex::build(&s, &AtomicBool::new(true), &AtomicU64::new(0));
    assert!(matches!(r, Err(Cancelled)));
}

#[test]
fn lines_default_build_works() {
    let f = TempFile::new(b"x\ny\nz");
    let s = f.open();
    let idx = LineIndex::build(&s, &AtomicBool::new(false), &AtomicU64::new(0)).unwrap();
    assert_eq!(idx.line_count(), 3);
    assert_eq!(idx.offset_of_line(&s, 2), Some(4));
}

#[test]
fn lines_cancel_before_start_returns_cancelled() {
    let f = TempFile::new(&random_lines(7, 10_000));
    let s = f.open();
    let r = LineIndex::build_with(
        &s,
        &BuildOptions {
            chunk_bytes: 100,
            checkpoint_every: 8,
        },
        &AtomicBool::new(true),
        &AtomicU64::new(0),
    );
    assert!(matches!(r, Err(Cancelled)));
}

#[test]
fn lines_progress_reaches_file_length() {
    let data = random_lines(9, 10_000);
    let f = TempFile::new(&data);
    let s = f.open();
    let progress = AtomicU64::new(0);
    let r = LineIndex::build_with(
        &s,
        &BuildOptions {
            chunk_bytes: 64,
            checkpoint_every: 8,
        },
        &AtomicBool::new(false),
        &progress,
    );
    assert!(r.is_ok());
    assert_eq!(progress.load(Ordering::SeqCst), data.len() as u64);
}

// ---------- ricerca: casi a mano ----------

const CHUNKS: [usize; 10] = [1, 2, 3, 5, 6, 7, 50, 99, 100, 1000];

#[test]
fn search_next_basic() {
    let f = TempFile::new(b"xxabcxxabcxx"); // "abc" a 2 e a 7
    let s = f.open();
    for chunk in CHUNKS {
        assert_eq!(
            fnext(&s, b"abc", 0, chunk),
            SearchResult::Found(2),
            "chunk {chunk}"
        );
        assert_eq!(fnext(&s, b"abc", 2, chunk), SearchResult::Found(2));
        assert_eq!(fnext(&s, b"abc", 3, chunk), SearchResult::Found(7));
        assert_eq!(fnext(&s, b"abc", 7, chunk), SearchResult::Found(7));
        assert_eq!(fnext(&s, b"abc", 8, chunk), SearchResult::NotFound);
        assert_eq!(fnext(&s, b"abc", 1_000, chunk), SearchResult::NotFound);
        assert_eq!(fnext(&s, b"abd", 0, chunk), SearchResult::NotFound);
    }
}

#[test]
fn search_next_overlapping_matches() {
    let f = TempFile::new(b"aaaa");
    let s = f.open();
    for chunk in CHUNKS {
        assert_eq!(
            fnext(&s, b"aa", 0, chunk),
            SearchResult::Found(0),
            "chunk {chunk}"
        );
        assert_eq!(fnext(&s, b"aa", 1, chunk), SearchResult::Found(1));
        assert_eq!(fnext(&s, b"aa", 2, chunk), SearchResult::Found(2));
        assert_eq!(fnext(&s, b"aa", 3, chunk), SearchResult::NotFound);
    }
}

#[test]
fn search_next_match_crossing_chunk_boundary() {
    let mut data = vec![b'x'; 99];
    data.extend_from_slice(b"needle"); // inizia a 99
    data.extend_from_slice(&[b'y'; 20]);
    let f = TempFile::new(&data);
    let s = f.open();
    for chunk in CHUNKS {
        assert_eq!(
            fnext(&s, b"needle", 0, chunk),
            SearchResult::Found(99),
            "chunk {chunk}"
        );
        assert_eq!(fnext(&s, b"needle", 98, chunk), SearchResult::Found(99));
        assert_eq!(fnext(&s, b"needle", 99, chunk), SearchResult::Found(99));
        assert_eq!(fnext(&s, b"needle", 100, chunk), SearchResult::NotFound);
    }
}

#[test]
fn search_edge_cases() {
    let f = TempFile::new(b"abc");
    let s = f.open();
    let empty = TempFile::new(b"");
    let e = empty.open();
    for chunk in CHUNKS {
        assert_eq!(fnext(&s, b"", 0, chunk), SearchResult::NotFound);
        assert_eq!(fprev(&s, b"", 3, chunk), SearchResult::NotFound);
        assert_eq!(fnext(&s, b"abcd", 0, chunk), SearchResult::NotFound);
        assert_eq!(fprev(&s, b"abcd", 3, chunk), SearchResult::NotFound);
        assert_eq!(fnext(&e, b"a", 0, chunk), SearchResult::NotFound);
        assert_eq!(fprev(&e, b"a", 0, chunk), SearchResult::NotFound);
        assert_eq!(fnext(&s, b"abc", 0, chunk), SearchResult::Found(0));
        assert_eq!(fprev(&s, b"abc", 3, chunk), SearchResult::Found(0));
    }
}

#[test]
fn search_prev_basic() {
    let f = TempFile::new(b"xxabcxxabcxx"); // "abc" a 2 e a 7
    let s = f.open();
    for chunk in CHUNKS {
        assert_eq!(
            fprev(&s, b"abc", 12, chunk),
            SearchResult::Found(7),
            "chunk {chunk}"
        );
        assert_eq!(fprev(&s, b"abc", 1_000, chunk), SearchResult::Found(7));
        assert_eq!(fprev(&s, b"abc", 8, chunk), SearchResult::Found(7));
        assert_eq!(fprev(&s, b"abc", 7, chunk), SearchResult::Found(2));
        assert_eq!(fprev(&s, b"abc", 3, chunk), SearchResult::Found(2));
        assert_eq!(fprev(&s, b"abc", 2, chunk), SearchResult::NotFound);
        assert_eq!(fprev(&s, b"abc", 0, chunk), SearchResult::NotFound);
    }
}

#[test]
fn search_prev_overlapping_and_boundary() {
    let f = TempFile::new(b"aaaa");
    let s = f.open();
    for chunk in CHUNKS {
        assert_eq!(
            fprev(&s, b"aa", 100, chunk),
            SearchResult::Found(2),
            "chunk {chunk}"
        );
        assert_eq!(fprev(&s, b"aa", 3, chunk), SearchResult::Found(2));
        assert_eq!(fprev(&s, b"aa", 2, chunk), SearchResult::Found(1));
        assert_eq!(fprev(&s, b"aa", 1, chunk), SearchResult::Found(0));
        assert_eq!(fprev(&s, b"aa", 0, chunk), SearchResult::NotFound);
    }
    let mut data = vec![b'x'; 99];
    data.extend_from_slice(b"needle");
    data.extend_from_slice(&[b'y'; 20]);
    let f2 = TempFile::new(&data);
    let s2 = f2.open();
    for chunk in CHUNKS {
        assert_eq!(
            fprev(&s2, b"needle", 1_000, chunk),
            SearchResult::Found(99),
            "chunk {chunk}"
        );
        assert_eq!(fprev(&s2, b"needle", 100, chunk), SearchResult::Found(99));
        assert_eq!(fprev(&s2, b"needle", 99, chunk), SearchResult::NotFound);
    }
}

#[test]
fn search_cancelled() {
    let f = TempFile::new(b"xxabcxxabcxx");
    let s = f.open();
    let opts = SearchOptions { chunk_bytes: 3 };
    let yes = AtomicBool::new(true);
    assert_eq!(
        find_next(&s, b"abc", 0, &opts, &yes),
        SearchResult::Cancelled
    );
    assert_eq!(
        find_prev(&s, b"abc", 12, &opts, &yes),
        SearchResult::Cancelled
    );
}

#[test]
fn search_cancel_priority_over_not_found_but_not_over_empty_pattern() {
    let f = TempFile::new(b"abc");
    let s = f.open();
    let opts = SearchOptions { chunk_bytes: 2 };
    let yes = AtomicBool::new(true);
    // pattern vuoto: NotFound anche se annullato
    assert_eq!(find_next(&s, b"", 0, &opts, &yes), SearchResult::NotFound);
    assert_eq!(find_prev(&s, b"", 3, &opts, &yes), SearchResult::NotFound);
    // l'annullamento vince su "oltre la fine", "before == 0" e "pattern piu' lungo del file"
    assert_eq!(
        find_next(&s, b"abc", 1000, &opts, &yes),
        SearchResult::Cancelled
    );
    assert_eq!(
        find_next(&s, b"abcd", 0, &opts, &yes),
        SearchResult::Cancelled
    );
    assert_eq!(
        find_prev(&s, b"abc", 0, &opts, &yes),
        SearchResult::Cancelled
    );
    assert_eq!(
        find_prev(&s, b"abcd", 3, &opts, &yes),
        SearchResult::Cancelled
    );
    // anche su un file vuoto
    let e = TempFile::new(b"");
    let es = e.open();
    assert_eq!(
        find_next(&es, b"a", 0, &opts, &yes),
        SearchResult::Cancelled
    );
    assert_eq!(
        find_prev(&es, b"a", 0, &opts, &yes),
        SearchResult::Cancelled
    );
}

// ---------- ricerca: confronto con l'oracolo ingenuo ----------

fn naive_next(data: &[u8], pat: &[u8], from: usize) -> SearchResult {
    if pat.is_empty() || data.len() < pat.len() {
        return SearchResult::NotFound;
    }
    for s in from..=(data.len() - pat.len()) {
        if &data[s..s + pat.len()] == pat {
            return SearchResult::Found(s as u64);
        }
    }
    SearchResult::NotFound
}

fn naive_prev(data: &[u8], pat: &[u8], before: usize) -> SearchResult {
    if pat.is_empty() || data.len() < pat.len() {
        return SearchResult::NotFound;
    }
    let last_start = data.len() - pat.len();
    let top = before.min(last_start + 1); // inizio s < before e s <= last_start
    for s in (0..top).rev() {
        if &data[s..s + pat.len()] == pat {
            return SearchResult::Found(s as u64);
        }
    }
    SearchResult::NotFound
}

#[test]
fn search_matches_naive_oracle() {
    let alphabet = b"ab\n";
    for (n, chunks) in [(300usize, vec![1usize, 3, 16]), (3000, vec![16, 4000])] {
        let mut rng = Lcg(42 + n as u64);
        let data: Vec<u8> = (0..n)
            .map(|_| alphabet[(rng.next() % 3) as usize])
            .collect();
        let f = TempFile::new(&data);
        let s = f.open();
        // schemi: pezzi veri del file e stringhe casuali di 1..=4 byte
        let mut pats: Vec<Vec<u8>> = Vec::new();
        for _ in 0..8 {
            let len = 1 + (rng.next() % 4) as usize;
            let st = (rng.next() as usize) % (n - len);
            pats.push(data[st..st + len].to_vec());
        }
        for _ in 0..5 {
            let len = 1 + (rng.next() % 4) as usize;
            pats.push(
                (0..len)
                    .map(|_| alphabet[(rng.next() % 3) as usize])
                    .collect(),
            );
        }
        for &chunk in &chunks {
            for pat in &pats {
                for pos in 0..=(n + 2) {
                    assert_eq!(
                        fnext(&s, pat, pos as u64, chunk),
                        naive_next(&data, pat, pos.min(n + 1)),
                        "next n {n} chunk {chunk} pat {pat:?} from {pos}"
                    );
                    assert_eq!(
                        fprev(&s, pat, pos as u64, chunk),
                        naive_prev(&data, pat, pos),
                        "prev n {n} chunk {chunk} pat {pat:?} before {pos}"
                    );
                }
            }
        }
    }
}

// ---------- file oltre 4 GiB (si lancia a mano) ----------

/// File generato da `bench/gen_testfile.py` (righe di esattamente 100 byte: `L{n:07} ...`, n ripete ogni 1.000.000).
/// Esempio:  MRP_BIG_FILE=D:\bench\big6g.txt cargo test --release -- --ignored --nocapture
#[test]
#[ignore = "serve MRP_BIG_FILE=<file da bench/gen_testfile.py>"]
fn big_file_beyond_4gib() {
    let path = std::env::var("MRP_BIG_FILE").expect("imposta MRP_BIG_FILE");
    let src = FileSource::open(std::path::Path::new(&path)).unwrap();
    let len = src.len();
    assert_eq!(len % 100, 0, "il file deve avere righe di 100 byte");
    let lines = len / 100;
    let cancel = AtomicBool::new(false);
    let progress = AtomicU64::new(0);
    let t = std::time::Instant::now();
    let idx = LineIndex::build(&src, &cancel, &progress).unwrap();
    eprintln!(
        "indice: {} byte, {} righe, {} punti, {:.2} s",
        len,
        idx.line_count(),
        idx.checkpoint_count(),
        t.elapsed().as_secs_f64()
    );
    assert_eq!(idx.line_count(), lines);
    assert_eq!(progress.load(Ordering::SeqCst), len);
    for k in [0u64, 1, 255, 256, 4_999_999, lines / 2, lines - 1] {
        if k >= lines {
            continue;
        }
        let off = idx.offset_of_line(&src, k).unwrap();
        assert_eq!(off, k * 100, "riga {k}");
        let want = format!("L{:07} ", k % 1_000_000);
        assert_eq!(
            src.read_range(off, 9),
            want.as_bytes(),
            "contenuto riga {k}"
        );
        assert_eq!(idx.line_at_offset(&src, k * 100 + 99), Some(k));
    }
    if len > u32::MAX as u64 {
        assert!(idx.offset_of_line(&src, lines - 1).unwrap() > u32::MAX as u64);
        let r = fnext(&src, b"L0999999 ", len - 100, 8 * 1024 * 1024);
        assert_eq!(r, SearchResult::Found(len - 100));
    }
}
