//! Test della ricerca sul documento modificato (`Snapshot`). Scritti da ARCH prima dell'implementazione:
//! l'oracolo e' una ricerca ingenua su un `Vec<u8>` che replica le modifiche.

use mrp_core::docsearch::SearchError;
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_ds_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn open(&self, data: &[u8]) -> Document {
        let p = self
            .0
            .join(format!("a{}.txt", COUNTER.fetch_add(1, Ordering::SeqCst)));
        std::fs::write(&p, data).unwrap();
        Document::open(&p).unwrap()
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

fn ab(r: &mut Lcg, n: usize) -> Vec<u8> {
    (0..n)
        .map(|_| if r.below(2) == 0 { b'a' } else { b'b' })
        .collect()
}

/// Oracolo: primo inizio >= from (con needle intera dentro il documento).
fn o_next(m: &[u8], needle: &[u8], from: u64) -> Option<u64> {
    if needle.is_empty() || needle.len() > m.len() {
        return None;
    }
    (from as usize..=m.len() - needle.len())
        .find(|&s| &m[s..s + needle.len()] == needle)
        .map(|s| s as u64)
}

/// Oracolo: ultimo inizio < before.
fn o_prev(m: &[u8], needle: &[u8], before: u64) -> Option<u64> {
    if needle.is_empty() || needle.len() > m.len() {
        return None;
    }
    let max_start = m.len() - needle.len();
    let hi = (before as usize).min(max_start + 1);
    (0..hi)
        .rev()
        .find(|&s| &m[s..s + needle.len()] == needle)
        .map(|s| s as u64)
}

#[test]
fn by_hand() {
    let dir = TestDir::new();
    let d = dir.open(b"hello world hello");
    let s = d.snapshot().unwrap();
    let (c, p) = flags();
    let next = |from: u64, n: &[u8]| s.find_next(n, from, &c, &p).unwrap();
    let prev = |before: u64, n: &[u8]| s.find_prev(n, before, &c, &p).unwrap();
    assert_eq!(next(0, b"hello"), Some(0));
    assert_eq!(next(1, b"hello"), Some(12));
    assert_eq!(next(13, b"hello"), None);
    assert_eq!(next(0, b"o w"), Some(4));
    assert_eq!(next(0, b"zzz"), None);
    assert_eq!(next(0, b""), None, "ago vuoto: niente");
    assert_eq!(
        next(0, b"hello world hello and more"),
        None,
        "ago piu' lungo del documento"
    );
    assert_eq!(next(99, b"hello"), None, "from oltre la fine");
    assert_eq!(next(12, b"hello"), Some(12), "from e' incluso");
    assert_eq!(prev(17, b"hello"), Some(12));
    assert_eq!(prev(12, b"hello"), Some(0), "before e' escluso");
    assert_eq!(prev(13, b"hello"), Some(12));
    assert_eq!(prev(0, b"hello"), None);
    assert_eq!(
        prev(99, b"hello"),
        Some(12),
        "before oltre la fine: tutto il documento"
    );
    assert_eq!(prev(5, b"o"), Some(4));
    assert_eq!(prev(17, b""), None);
}

#[test]
fn every_position_and_chunk_size_on_edited_documents() {
    let dir = TestDir::new();
    for seed in 1..=6u64 {
        let mut r = Lcg(seed * 101);
        let n = r.below(120) as usize;
        let mut m = ab(&mut r, n);
        let mut d = dir.open(&m);
        for _ in 0..6 {
            if r.below(2) == 0 || m.is_empty() {
                let pos = r.below(m.len() as u64 + 1) as usize;
                let k = r.below(8) as usize;
                let data = ab(&mut r, k);
                d.insert(pos as u64, &data).unwrap();
                m.splice(pos..pos, data.iter().copied());
            } else {
                let pos = r.below(m.len() as u64) as usize;
                let len = r.below((m.len() - pos) as u64 + 1).min(r.below(15) + 1) as usize;
                d.delete(pos as u64, len as u64).unwrap();
                m.drain(pos..pos + len);
            }
        }
        for chunk in [1usize, 2, 3, 7, 64, 1 << 20] {
            let s = d.snapshot().unwrap().with_chunk(chunk);
            assert_eq!(s.len(), m.len() as u64);
            for nlen in 1..=4 {
                for pattern in 0..(1u32 << nlen) {
                    let needle: Vec<u8> = (0..nlen)
                        .map(|i| if pattern >> i & 1 == 0 { b'a' } else { b'b' })
                        .collect();
                    for pos in 0..=(m.len() as u64 + 1) {
                        let (c, p) = flags();
                        assert_eq!(
                            s.find_next(&needle, pos, &c, &p).unwrap(),
                            o_next(&m, &needle, pos.min(m.len() as u64 + 1)),
                            "seed {seed} chunk {chunk} next {needle:?} from {pos}"
                        );
                        assert_eq!(
                            s.find_prev(&needle, pos, &c, &p).unwrap(),
                            o_prev(&m, &needle, pos),
                            "seed {seed} chunk {chunk} prev {needle:?} before {pos}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn non_utf8_needles_work() {
    let dir = TestDir::new();
    let mut data = vec![b'x'; 100];
    data[40..44].copy_from_slice(&[0xff, 0x00, 0xfe, 0x80]);
    let d = dir.open(&data);
    let s = d.snapshot().unwrap().with_chunk(5);
    let (c, p) = flags();
    assert_eq!(
        s.find_next(&[0xff, 0x00, 0xfe], 0, &c, &p).unwrap(),
        Some(40)
    );
    assert_eq!(
        s.find_prev(&[0x00, 0xfe, 0x80], 100, &c, &p).unwrap(),
        Some(41)
    );
}

#[test]
fn cancel_stops_the_search() {
    let dir = TestDir::new();
    let data = vec![b'a'; 4 * 1024 * 1024];
    let d = dir.open(&data);
    let s = d.snapshot().unwrap().with_chunk(64 * 1024);
    let c = AtomicBool::new(true);
    let p = AtomicU64::new(0);
    assert!(matches!(
        s.find_next(b"zz", 0, &c, &p),
        Err(SearchError::Cancelled)
    ));
    assert!(matches!(
        s.find_prev(b"zz", data.len() as u64, &c, &p),
        Err(SearchError::Cancelled)
    ));
    let (c, p) = flags();
    assert_eq!(s.find_next(b"zz", 0, &c, &p).unwrap(), None);
    assert_eq!(
        p.load(Ordering::SeqCst),
        data.len() as u64,
        "progress arriva ai byte esaminati (tutto il documento)"
    );
}

#[test]
fn a_cancel_already_set_wins_over_the_quick_none_answers() {
    let dir = TestDir::new();
    let d = dir.open(b"abc");
    let s = d.snapshot().unwrap();
    let c = AtomicBool::new(true);
    let p = AtomicU64::new(0);
    // ago piu' lungo del documento, partenza oltre la fine: senza cancel sarebbero `None` subito
    assert!(matches!(
        s.find_next(b"abcdef", 0, &c, &p),
        Err(SearchError::Cancelled)
    ));
    assert!(matches!(
        s.find_next(b"a", 99, &c, &p),
        Err(SearchError::Cancelled)
    ));
    assert!(matches!(
        s.find_prev(b"abcdef", 3, &c, &p),
        Err(SearchError::Cancelled)
    ));
    assert!(matches!(
        s.find_prev(b"a", 0, &c, &p),
        Err(SearchError::Cancelled)
    ));
    let (c, p) = flags();
    assert_eq!(s.find_next(b"abcdef", 0, &c, &p).unwrap(), None);
    assert_eq!(s.find_prev(b"a", 0, &c, &p).unwrap(), None);
}

#[test]
fn a_chunk_of_zero_still_progresses_and_find_prev_reports_progress() {
    let dir = TestDir::new();
    let d = dir.open(b"abcabc");
    let s = d.snapshot().unwrap().with_chunk(0);
    let (c, p) = flags();
    assert_eq!(s.find_next(b"ca", 0, &c, &p).unwrap(), Some(2));
    assert_eq!(s.find_prev(b"bc", 6, &c, &p).unwrap(), Some(4));
    // find_prev senza trovare niente: l'avanzamento copre (quasi) tutto il documento
    let data = vec![b'a'; 1024 * 1024];
    let d = dir.open(&data);
    let s = d.snapshot().unwrap().with_chunk(64 * 1024);
    let (c, p) = flags();
    assert_eq!(s.find_prev(b"zz", data.len() as u64, &c, &p).unwrap(), None);
    let fatto = p.load(Ordering::SeqCst);
    assert!(
        fatto >= data.len() as u64 - 1 && fatto <= data.len() as u64,
        "progress {fatto}"
    );
}

#[test]
fn a_snapshot_is_frozen_and_survives_edits_and_saves() {
    let dir = TestDir::new();
    let mut d = dir.open(b"one two three");
    let s = d.snapshot().unwrap();
    d.delete(0, 4).unwrap(); // "two three"
    d.insert(0, b"two ").unwrap();
    let (c, p) = flags();
    assert_eq!(
        s.find_next(b"one", 0, &c, &p).unwrap(),
        Some(0),
        "lo snapshot vede il documento com'era"
    );
    let s2 = d.snapshot().unwrap();
    assert_eq!(s2.find_next(b"one", 0, &c, &p).unwrap(), None);
    assert_eq!(s2.find_next(b"two", 0, &c, &p).unwrap(), Some(0));
    // un salvataggio sostituisce il file: gli snapshot gia' presi continuano a funzionare sul vecchio contenuto
    let (c2, p2) = flags();
    d.save(&c2, &p2).unwrap();
    assert_eq!(s.find_next(b"three", 0, &c, &p).unwrap(), Some(8));
    assert_eq!(s2.find_next(b"three", 0, &c, &p).unwrap(), Some(8));
    let s3 = d.snapshot().unwrap();
    assert_eq!(s3.find_next(b"two", 0, &c, &p).unwrap(), Some(0));
}

#[test]
fn a_match_planted_deep_in_a_big_file_is_found_both_ways() {
    let dir = TestDir::new();
    let mut r = Lcg(8);
    let mut data: Vec<u8> = (0..20 * 1024 * 1024)
        .map(|_| b'a' + r.below(20) as u8)
        .collect();
    let needle = b"ZZ-needle-ZZ";
    let at = 15_000_000usize;
    data[at..at + needle.len()].copy_from_slice(needle);
    let mut d = dir.open(&data);
    d.insert(100, b"Q").unwrap(); // sposta tutto di uno
    let s = d.snapshot().unwrap();
    let (c, p) = flags();
    assert_eq!(s.find_next(needle, 0, &c, &p).unwrap(), Some(at as u64 + 1));
    assert_eq!(
        s.find_prev(needle, s.len(), &c, &p).unwrap(),
        Some(at as u64 + 1)
    );
    assert_eq!(s.find_next(needle, at as u64 + 2, &c, &p).unwrap(), None);
    assert_eq!(s.find_prev(needle, at as u64 + 1, &c, &p).unwrap(), None);
}
