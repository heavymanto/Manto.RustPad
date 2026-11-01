//! La vista filtrata (fase 2, 3.3, nucleo): `Snapshot::filter_lines` scrive su un `Write` le righe che corrispondono a un
//! `Pattern` (o quelle che NON corrispondono, con `invert`) e restituisce dove cominciava ciascuna nel documento. Legge a
//! blocchi di righe intere, si puo' annullare e si ferma a un numero massimo di righe. Scritti da ARCH: l'oracolo e' un ciclo
//! ingenuo su `str::lines`-simile (spezza sul `\n` conservando gli a-capo) con `contains`/`to_lowercase` per il testo.

use mrp_core::docsearch::SearchError;
use mrp_core::document::Document;
use mrp_core::pattern::{FilterStats, Mode, Pattern};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_filter_test_{}_{}.log", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn no() -> AtomicBool {
    AtomicBool::new(false)
}

fn prog() -> AtomicU64 {
    AtomicU64::new(0)
}

fn pat(t: &str, m: Mode, case: bool) -> Pattern {
    Pattern::new(t, m, case).unwrap()
}

/// Le righe di `dati` con il loro a-capo (l'ultima puo' non averlo) e l'offset d'inizio di ciascuna.
fn righe(dati: &[u8]) -> Vec<(u64, Vec<u8>)> {
    let mut v = Vec::new();
    let mut inizio = 0usize;
    while inizio < dati.len() {
        let fine = dati[inizio..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(dati.len(), |i| inizio + i + 1);
        v.push((inizio as u64, dati[inizio..fine].to_vec()));
        inizio = fine;
    }
    v
}

/// Il risultato atteso: le righe per cui `vale` e' `!invert`, con l'a-capo aggiunto all'ultima se manca.
fn atteso(dati: &[u8], vale: impl Fn(&str) -> bool, invert: bool) -> (Vec<u8>, Vec<u64>) {
    let mut fuori = Vec::new();
    let mut offset = Vec::new();
    for (o, r) in righe(dati) {
        let testo = String::from_utf8_lossy(&r).to_string();
        if vale(testo.trim_end_matches(['\n', '\r'])) != invert {
            fuori.extend_from_slice(&r);
            if !r.ends_with(b"\n") {
                fuori.push(b'\n');
            }
            offset.push(o);
        }
    }
    (fuori, offset)
}

fn filtra(
    dati: &[u8],
    p: &Pattern,
    invert: bool,
    max: u64,
    chunk: usize,
) -> (Vec<u8>, Vec<u64>, FilterStats) {
    let t = TempFile::new(dati);
    let d = Document::open(&t.0).unwrap();
    let snap = d.snapshot().unwrap().with_chunk(chunk);
    let mut fuori = Vec::new();
    let mut offset = Vec::new();
    let st = snap
        .filter_lines(p, invert, max, &mut fuori, &mut offset, &no(), &prog())
        .unwrap();
    (fuori, offset, st)
}

const LOG: &[u8] = b"2026 INFO avvio\n2026 ERROR disco pieno\n2026 INFO ok\n2026 error minuscolo\n\n  WARN lento\nERROR ancora";

#[test]
fn the_matching_lines_and_where_they_start() {
    let p = pat("ERROR", Mode::Text, true);
    let (fuori, offset, st) = filtra(LOG, &p, false, u64::MAX, 1 << 20);
    let (f, o) = atteso(LOG, |r| r.contains("ERROR"), false);
    assert_eq!(fuori, f);
    assert_eq!(offset, o);
    assert_eq!(offset, vec![16, 87]);
    // l'ultima riga non finisce con un a-capo: l'uscita lo aggiunge
    assert!(fuori.ends_with(b"ERROR ancora\n"));
    assert_eq!(
        st,
        FilterStats {
            lines: 2,
            truncated: false
        }
    );
}

#[test]
fn invert_gives_the_other_lines() {
    let p = pat("ERROR", Mode::Text, true);
    let (fuori, offset, st) = filtra(LOG, &p, true, u64::MAX, 1 << 20);
    let (f, o) = atteso(LOG, |r| r.contains("ERROR"), true);
    assert_eq!(fuori, f);
    assert_eq!(offset, o);
    // 7 righe in tutto (compresa la vuota), 2 corrispondono
    assert_eq!(st.lines, 5);
    assert!(offset.contains(&73), "la riga vuota comincia a 73");
}

#[test]
fn case_glob_and_regex_modes() {
    // senza maiuscole: ERROR e error
    let (f, o, st) = filtra(
        LOG,
        &pat("error", Mode::Text, false),
        false,
        u64::MAX,
        1 << 20,
    );
    let (fa, oa) = atteso(LOG, |r| r.to_lowercase().contains("error"), false);
    assert_eq!((f, o), (fa, oa));
    assert_eq!(st.lines, 3);
    // con maiuscole: solo error
    let (_, o, _) = filtra(
        LOG,
        &pat("error", Mode::Text, true),
        false,
        u64::MAX,
        1 << 20,
    );
    assert_eq!(o.len(), 1);
    // glob: "2026*ok" (per riga)
    let (f, _, st) = filtra(
        LOG,
        &pat("2026*ok", Mode::Glob, true),
        false,
        u64::MAX,
        1 << 20,
    );
    assert_eq!(f, b"2026 INFO ok\n");
    assert_eq!(st.lines, 1);
    // regex: le righe che cominciano con due spazi
    let (f, _, _) = filtra(
        LOG,
        &pat(r"^\s+WARN", Mode::Regex, true),
        false,
        u64::MAX,
        1 << 20,
    );
    assert_eq!(f, b"  WARN lento\n");
}

#[test]
fn crlf_lines_keep_their_line_ending() {
    let dati = b"uno ERROR\r\ndue\r\ntre ERROR\r\n";
    let (f, o, _) = filtra(
        dati,
        &pat("ERROR", Mode::Text, true),
        false,
        u64::MAX,
        1 << 20,
    );
    assert_eq!(f, b"uno ERROR\r\ntre ERROR\r\n");
    assert_eq!(o, vec![0, 16]);
}

#[test]
fn nothing_matches_and_empty_files() {
    let (f, o, st) = filtra(LOG, &pat("zzz", Mode::Text, true), false, u64::MAX, 1 << 20);
    assert!(f.is_empty() && o.is_empty());
    assert_eq!(
        st,
        FilterStats {
            lines: 0,
            truncated: false
        }
    );
    let (f, o, st) = filtra(b"", &pat("a", Mode::Text, true), false, u64::MAX, 1 << 20);
    assert!(f.is_empty() && o.is_empty() && st.lines == 0);
}

#[test]
fn the_result_does_not_depend_on_the_block_size() {
    let big: Vec<u8> = (0..400)
        .flat_map(|i| {
            format!(
                "riga numero {i} {}\n",
                if i % 7 == 0 { "ERROR" } else { "ok" }
            )
            .into_bytes()
        })
        .collect();
    let p = pat("ERROR", Mode::Text, true);
    let base = filtra(&big, &p, false, u64::MAX, 1 << 20);
    for chunk in [1usize, 3, 7, 50, 333, 1000] {
        let r = filtra(&big, &p, false, u64::MAX, chunk);
        assert_eq!((&r.0, &r.1), (&base.0, &base.1), "blocco {chunk}");
    }
    let (f, o) = atteso(&big, |r| r.contains("ERROR"), false);
    assert_eq!((base.0, base.1), (f, o));
}

#[test]
fn the_line_limit_stops_the_filter_and_says_so() {
    let big: Vec<u8> = (0..100)
        .flat_map(|i| format!("ERROR {i}\n").into_bytes())
        .collect();
    let p = pat("ERROR", Mode::Text, true);
    let (f, o, st) = filtra(&big, &p, false, 10, 64);
    assert_eq!(
        st,
        FilterStats {
            lines: 10,
            truncated: true
        }
    );
    assert_eq!(o.len(), 10);
    assert_eq!(f, big[..o[9] as usize + "ERROR 9\n".len()].to_vec());
    // il limite esatto: 100 righe con limite 100 non e' troncato
    let (_, o, st) = filtra(&big, &p, false, 100, 64);
    assert_eq!((o.len(), st.truncated), (100, false));
}

#[test]
fn a_cancelled_filter_says_so() {
    let t = TempFile::new(LOG);
    let d = Document::open(&t.0).unwrap();
    let snap = d.snapshot().unwrap();
    let c = AtomicBool::new(true);
    let mut fuori = Vec::new();
    let mut offset = Vec::new();
    let r = snap.filter_lines(
        &pat("ERROR", Mode::Text, true),
        false,
        u64::MAX,
        &mut fuori,
        &mut offset,
        &c,
        &prog(),
    );
    assert!(matches!(r, Err(SearchError::Cancelled)));
}

#[test]
fn progress_reaches_the_length() {
    let t = TempFile::new(LOG);
    let d = Document::open(&t.0).unwrap();
    let snap = d.snapshot().unwrap().with_chunk(10);
    let p = prog();
    let mut fuori = Vec::new();
    let mut offset = Vec::new();
    snap.filter_lines(
        &pat("ERROR", Mode::Text, true),
        false,
        u64::MAX,
        &mut fuori,
        &mut offset,
        &no(),
        &p,
    )
    .unwrap();
    assert_eq!(p.load(Ordering::Relaxed), LOG.len() as u64);
}

struct Rotto;

impl Write for Rotto {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("disco pieno"))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_failing_writer_is_an_io_error() {
    let t = TempFile::new(LOG);
    let d = Document::open(&t.0).unwrap();
    let snap = d.snapshot().unwrap();
    let mut offset = Vec::new();
    let r = snap.filter_lines(
        &pat("ERROR", Mode::Text, true),
        false,
        u64::MAX,
        &mut Rotto,
        &mut offset,
        &no(),
        &prog(),
    );
    assert!(matches!(r, Err(SearchError::Io(_))));
}

#[test]
fn unsaved_edits_are_in_the_filtered_view() {
    let t = TempFile::new(b"uno\ndue\ntre\n");
    let mut d = Document::open(&t.0).unwrap();
    d.insert(0, b"ERROR nuova\n").unwrap();
    d.insert(d.len(), b"altra ERROR\n").unwrap();
    let snap = d.snapshot().unwrap();
    let mut fuori = Vec::new();
    let mut offset = Vec::new();
    snap.filter_lines(
        &pat("ERROR", Mode::Text, true),
        false,
        u64::MAX,
        &mut fuori,
        &mut offset,
        &no(),
        &prog(),
    )
    .unwrap();
    assert_eq!(fuori, b"ERROR nuova\naltra ERROR\n");
    assert_eq!(offset, vec![0, 24]);
}

#[test]
fn a_big_log_matches_the_naive_filter() {
    // ~15 MB, una riga su 97 con ERROR, qualche riga vuota
    let mut dati = Vec::new();
    for i in 0..300_000u32 {
        if i % 1000 == 999 {
            dati.push(b'\n');
        }
        let liv = if i % 97 == 0 { "ERROR" } else { "INFO " };
        dati.extend_from_slice(
            format!("2026-10-31 09:00:00 [{liv}] messaggio numero {i}\n").as_bytes(),
        );
    }
    let p = pat("[ERROR]", Mode::Text, true);
    let (f, o, st) = filtra(&dati, &p, false, u64::MAX, 1 << 20);
    let (fa, oa) = atteso(&dati, |r| r.contains("[ERROR]"), false);
    assert_eq!(f.len(), fa.len());
    assert!(f == fa && o == oa);
    assert_eq!(st.lines, oa.len() as u64);
}
