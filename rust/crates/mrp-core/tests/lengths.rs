//! L'istogramma delle lunghezze delle righe (`Snapshot::count_lengths`), la passata del check dei tracciati (Alt+H): per
//! ogni lunghezza di contenuto (i byte senza l'a-capo, e senza il `\r` finale) quante righe ce l'hanno. Scritti da ARCH:
//! l'oracolo e' il conteggio ingenuo sulle righe di partenza scritte qui (i byte contati a mano, mai dal programma); la
//! lettura a blocchi si prova con blocchi piccolissimi e righe piu' lunghe del blocco, e le modifiche non salvate passano
//! dalla fotografia (la piece table), non dal file.

use mrp_core::docsearch::SearchError;
use mrp_core::Document;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Lcg(u64);

impl Lcg {
    fn sotto(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

fn file(dati: &[u8]) -> (Document, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_lengths_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    (Document::open(&p).unwrap(), p)
}

/// L'oracolo: quante righe hanno ogni lunghezza di contenuto, contate qui (senza `\n` e senza il `\r` finale).
fn oracolo(righe: &[&str]) -> HashMap<u64, u64> {
    let mut m: HashMap<u64, u64> = HashMap::new();
    for r in righe {
        let contenuto = r.strip_suffix('\r').unwrap_or(r);
        *m.entry(contenuto.len() as u64).or_insert(0) += 1;
    }
    m
}

/// Conta l'istogramma (con blocchi piccoli se chiesti) e riporta anche il progresso raggiunto.
fn conta(doc: &Document, chunk: Option<usize>) -> (HashMap<u64, u64>, u64) {
    let mut snap = doc.snapshot().unwrap();
    if let Some(c) = chunk {
        snap = snap.with_chunk(c);
    }
    let progress = AtomicU64::new(0);
    let istogramma = snap
        .count_lengths(&AtomicBool::new(false), &progress)
        .unwrap();
    (istogramma, progress.load(Ordering::Relaxed))
}

#[test]
fn the_histogram_counts_the_content_bytes_of_every_line() {
    let (doc, p) = file(b"aaa\nbbbb\ncc\naaa\n\n");
    let (hist, _) = conta(&doc, None);
    assert_eq!(hist, oracolo(&["aaa", "bbbb", "cc", "aaa", ""]));
    let _ = std::fs::remove_file(p);
}

#[test]
fn crlf_lines_count_the_content_without_the_carriage_return() {
    // anche il `\r` finale di un'ultima riga senza a-capo: e' il record che conta, non i byte su disco
    let (doc, p) = file(b"aaa\r\nbbbb\r\nab\r");
    let (hist, _) = conta(&doc, None);
    assert_eq!(hist, oracolo(&["aaa\r", "bbbb\r", "ab\r"]));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_last_line_without_a_newline_is_a_line_and_empty_lines_count_zero() {
    let (doc, p) = file(b"xyz");
    let (hist, _) = conta(&doc, None);
    assert_eq!(hist, oracolo(&["xyz"]));
    let (doc, p2) = file(b"\n\n\n");
    let (hist, _) = conta(&doc, None);
    assert_eq!(hist, oracolo(&["", "", ""]));
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(p2);
}

#[test]
fn an_empty_file_gives_no_lengths_and_no_progress() {
    let (doc, p) = file(b"");
    let (hist, progress) = conta(&doc, None);
    assert!(hist.is_empty());
    assert_eq!(progress, 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn tiny_blocks_still_read_whole_lines_across_the_boundaries() {
    // righe piu' lunghe del blocco (500 byte con blocchi da 3), righe vuote, ultima riga chiusa
    let lunga = "x".repeat(500);
    let mut dati = String::from("\nab\n");
    dati.push_str(&lunga);
    dati.push_str("\ncd\n");
    let (doc, p) = file(dati.as_bytes());
    let righe = ["", "ab", lunga.as_str(), "cd"];
    let (hist, _) = conta(&doc, Some(3));
    assert_eq!(hist, oracolo(&righe));
    let (hist, _) = conta(&doc, None);
    assert_eq!(hist, oracolo(&righe), "con i blocchi interi lo stesso");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_pass_reports_the_bytes_it_examined_and_can_be_cancelled() {
    let (doc, p) = file(b"aaa\nbb\ncc\n");
    let (_, progress) = conta(&doc, None);
    assert_eq!(
        progress, 10,
        "a fine passata il progresso vale il file (4+3+3)"
    );
    let snap = doc.snapshot().unwrap();
    let progress = AtomicU64::new(0);
    let esito = snap.count_lengths(&AtomicBool::new(true), &progress);
    assert!(matches!(esito, Err(SearchError::Cancelled)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_histogram_sees_the_unsaved_edits() {
    let (mut doc, p) = file(b"xx\nyy\n");
    doc.insert(0, b"zzz\n").unwrap();
    doc.delete(4, 1).unwrap();
    let (hist, _) = conta(&doc, None);
    assert_eq!(hist, oracolo(&["zzz", "x", "yy"]));
    let _ = std::fs::remove_file(p);
}

#[test]
fn random_lines_match_the_naive_count_with_crlf_and_tiny_blocks() {
    let mut r = Lcg(2026_1111);
    let mut righe: Vec<String> = Vec::new();
    for i in 0..400u32 {
        let lunghezza = r.sotto(46) as usize;
        let mut s = format!("{i}");
        while s.len() < lunghezza {
            s.push('a');
        }
        s.truncate(lunghezza);
        righe.push(s);
    }
    let mut dati = String::new();
    for (i, riga) in righe.iter().enumerate() {
        dati.push_str(riga);
        if i + 1 < righe.len() {
            dati.push_str("\r\n");
        }
    }
    let (doc, p) = file(dati.as_bytes());
    let (hist, progress) = conta(&doc, Some(7));
    let attese: HashMap<u64, u64> = {
        let mut m = HashMap::new();
        for riga in &righe {
            *m.entry(riga.len() as u64).or_insert(0) += 1;
        }
        m
    };
    assert_eq!(hist, attese);
    assert_eq!(progress, dati.len() as u64);
    let _ = std::fs::remove_file(p);
}
