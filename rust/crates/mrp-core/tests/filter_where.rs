//! La vista filtrata con piu' condizioni insieme (`Snapshot::filter_lines_where`): un testo (testo, glob o regex, anche "le righe che NON
//! corrispondono"), i livelli (ERROR, WARN, INFO, DEBUG) e un intervallo di byte (la finestra di tempo): una riga entra se le soddisfa
//! TUTTE. Scritti da ARCH prima dell'implementazione: l'oracolo e' un conteggio ingenuo sul `Vec` delle righe di partenza (testo, livello e
//! offset di ogni riga scritti a mano), mai il filtro.

use mrp_core::docsearch::SearchError;
use mrp_core::loghl::Kind;
use mrp_core::pattern::{LineFilter, Mode, Pattern};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn sotto(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Una riga di prova: il testo (senza a-capo), il livello atteso e dove comincia nel file.
struct Riga {
    testo: String,
    livello: Option<Kind>,
    offset: u64,
}

/// Un log con tutti i livelli (anche le sigle), parole che compaiono in qualche riga ("timeout", "retry") e righe senza livello.
fn log(seme: u64, n: usize, crlf: bool) -> Vec<Riga> {
    let mut r = Lcg(seme);
    let mut righe = Vec::new();
    let mut offset = 0u64;
    for i in 0..n {
        let (etichetta, livello) = match r.sotto(9) {
            0 => ("[ERROR]", Some(Kind::Error)),
            1 => ("[ERR]", Some(Kind::Error)),
            2 => ("[WARN]", Some(Kind::Warn)),
            3 => ("[WRN]", Some(Kind::Warn)),
            4 => ("[INFO]", Some(Kind::Info)),
            5 => ("[INF]", Some(Kind::Info)),
            6 => ("[DEBUG]", Some(Kind::Debug)),
            7 => ("[DBG]", Some(Kind::Debug)),
            _ => ("senza livello", None),
        };
        let parola = match r.sotto(4) {
            0 => "timeout",
            1 => "retry",
            _ => "ok",
        };
        let testo = format!(
            "2026-11-01 02:{:02}:00 {etichetta} riga {i} {parola}",
            i % 60
        );
        let lung = testo.len() as u64 + if crlf { 2 } else { 1 };
        righe.push(Riga {
            testo,
            livello,
            offset,
        });
        offset += lung;
    }
    righe
}

fn documento(righe: &[Riga], crlf: bool, a_capo_finale: bool) -> (Document, PathBuf) {
    let mut dati = String::new();
    for (i, r) in righe.iter().enumerate() {
        dati.push_str(&r.testo);
        if i + 1 < righe.len() || a_capo_finale {
            dati.push_str(if crlf { "\r\n" } else { "\n" });
        }
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_fw_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    (Document::open(&p).unwrap(), p)
}

fn filtro(testo: Option<&str>, modo: Mode, invert: bool, livelli: [bool; 4]) -> LineFilter {
    LineFilter {
        pattern: testo.map(|t| Pattern::new(t, modo, true).unwrap()),
        invert,
        levels: livelli,
        lengths: None,
    }
}

/// Esegue il filtro e ritorna (uscita, offsets, righe, troncato).
fn esegui(
    doc: &Document,
    f: &LineFilter,
    lo: u64,
    hi: u64,
    max: u64,
) -> (String, Vec<u64>, u64, bool, u64) {
    let snap = doc.snapshot().unwrap();
    let mut out = Vec::new();
    let mut offsets = Vec::new();
    let progress = AtomicU64::new(0);
    let st = snap
        .filter_lines_where(
            f,
            lo,
            hi,
            max,
            &mut out,
            &mut offsets,
            &AtomicBool::new(false),
            &progress,
        )
        .unwrap();
    (
        String::from_utf8(out).unwrap(),
        offsets,
        st.lines,
        st.truncated,
        progress.load(Ordering::Relaxed),
    )
}

/// L'oracolo: le righe (indici) con inizio in `[lo, hi)` che soddisfano il testo (con `contiene`), l'inversione e i livelli.
fn atteso(
    righe: &[Riga],
    lo: u64,
    hi: u64,
    contiene: Option<&str>,
    invert: bool,
    livelli: [bool; 4],
) -> Vec<usize> {
    let nessun_livello = !livelli.iter().any(|&l| l);
    righe
        .iter()
        .enumerate()
        .filter(|(_, r)| r.offset >= lo && r.offset < hi)
        .filter(|(_, r)| match contiene {
            Some(t) => r.testo.contains(t) != invert,
            None => true,
        })
        .filter(|(_, r)| {
            nessun_livello
                || match r.livello {
                    Some(Kind::Error) => livelli[0],
                    Some(Kind::Warn) => livelli[1],
                    Some(Kind::Info) => livelli[2],
                    Some(Kind::Debug) => livelli[3],
                    _ => false,
                }
        })
        .map(|(i, _)| i)
        .collect()
}

fn uscita_attesa(righe: &[Riga], indici: &[usize], crlf: bool) -> String {
    let mut s = String::new();
    for &i in indici {
        s.push_str(&righe[i].testo);
        s.push_str(if crlf { "\r\n" } else { "\n" });
    }
    s
}

const NESSUNO: [bool; 4] = [false; 4];

#[test]
fn a_text_alone_gives_what_filter_lines_gives() {
    let righe = log(1, 300, false);
    let (doc, p) = documento(&righe, false, true);
    let (out, offsets, n, tronc, _) = esegui(
        &doc,
        &filtro(Some("timeout"), Mode::Text, false, NESSUNO),
        0,
        doc.len(),
        u64::MAX,
    );
    let idx = atteso(&righe, 0, doc.len(), Some("timeout"), false, NESSUNO);
    assert_eq!(out, uscita_attesa(&righe, &idx, false));
    assert_eq!(
        offsets,
        idx.iter().map(|&i| righe[i].offset).collect::<Vec<_>>()
    );
    assert_eq!((n as usize, tronc), (idx.len(), false));
    // lo stesso di `filter_lines` (la funzione di prima) sullo stesso testo
    let snap = doc.snapshot().unwrap();
    let mut out2 = Vec::new();
    let mut off2 = Vec::new();
    snap.filter_lines(
        &Pattern::new("timeout", Mode::Text, true).unwrap(),
        false,
        u64::MAX,
        &mut out2,
        &mut off2,
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    )
    .unwrap();
    assert_eq!(String::from_utf8(out2).unwrap(), out);
    assert_eq!(off2, offsets);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_levels_alone_keep_only_the_lines_of_those_levels() {
    let righe = log(2, 400, false);
    let (doc, p) = documento(&righe, false, true);
    for livelli in [
        [true, false, false, false],
        [true, true, false, false],
        [false, false, true, false],
        [false, false, false, true],
        [true, true, true, true],
    ] {
        let (out, offsets, n, _, _) = esegui(
            &doc,
            &filtro(None, Mode::Text, false, livelli),
            0,
            doc.len(),
            u64::MAX,
        );
        let idx = atteso(&righe, 0, doc.len(), None, false, livelli);
        assert_eq!(out, uscita_attesa(&righe, &idx, false), "{livelli:?}");
        assert_eq!(
            offsets,
            idx.iter().map(|&i| righe[i].offset).collect::<Vec<_>>()
        );
        assert_eq!(n as usize, idx.len());
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn text_and_levels_must_both_be_satisfied() {
    let righe = log(3, 500, false);
    let (doc, p) = documento(&righe, false, true);
    for (testo, modo, invert, livelli) in [
        ("timeout", Mode::Text, false, [true, true, false, false]),
        ("retry", Mode::Text, false, [false, false, true, false]),
        ("timeout", Mode::Text, true, [true, false, false, false]), // le righe che NON hanno "timeout", fra gli errori
        ("tim*ut", Mode::Glob, false, [false, true, false, false]),
    ] {
        let (out, _, n, _, _) = esegui(
            &doc,
            &filtro(Some(testo), modo, invert, livelli),
            0,
            doc.len(),
            u64::MAX,
        );
        // l'oracolo ingenuo usa `contains` (per il glob "tim*ut" si prova con "timeout")
        let cerca = if testo.contains('*') {
            "timeout"
        } else {
            testo
        };
        let idx = atteso(&righe, 0, doc.len(), Some(cerca), invert, livelli);
        assert_eq!(
            out,
            uscita_attesa(&righe, &idx, false),
            "{testo:?} {livelli:?} inv={invert}"
        );
        assert_eq!(n as usize, idx.len());
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn only_the_lines_that_start_inside_the_range_are_considered() {
    let righe = log(4, 600, false);
    let (doc, p) = documento(&righe, false, true);
    let (lo, hi) = (righe[150].offset, righe[420].offset);
    let livelli = [true, true, false, false];
    let (out, offsets, n, _, progresso) = esegui(
        &doc,
        &filtro(Some("ok"), Mode::Text, false, livelli),
        lo,
        hi,
        u64::MAX,
    );
    let idx = atteso(&righe, lo, hi, Some("ok"), false, livelli);
    assert_eq!(out, uscita_attesa(&righe, &idx, false));
    assert_eq!(
        offsets,
        idx.iter().map(|&i| righe[i].offset).collect::<Vec<_>>()
    );
    assert_eq!(n as usize, idx.len());
    assert!(offsets.iter().all(|&o| o >= lo && o < hi));
    // il progresso arriva ai byte dell'intervallo
    assert_eq!(progresso, hi - lo);
    // un intervallo che finisce oltre il documento si ferma al documento
    let (_, _, n2, _, _) = esegui(
        &doc,
        &filtro(None, Mode::Text, false, NESSUNO),
        lo,
        doc.len() + 1000,
        u64::MAX,
    );
    assert_eq!(n2 as usize, righe.iter().filter(|r| r.offset >= lo).count());
    let _ = std::fs::remove_file(p);
}

#[test]
fn no_condition_at_all_gives_every_line_of_the_range() {
    let righe = log(5, 120, false);
    let (doc, p) = documento(&righe, false, true);
    let (out, _, n, _, _) = esegui(
        &doc,
        &filtro(None, Mode::Text, false, NESSUNO),
        0,
        doc.len(),
        u64::MAX,
    );
    assert_eq!(n as usize, righe.len());
    assert_eq!(
        out,
        uscita_attesa(&righe, &(0..righe.len()).collect::<Vec<_>>(), false)
    );
    // invert senza testo non fa niente
    let (out2, _, n2, _, _) = esegui(
        &doc,
        &filtro(None, Mode::Text, true, NESSUNO),
        0,
        doc.len(),
        u64::MAX,
    );
    assert_eq!((out2, n2), (out, n));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_limit_of_lines_truncates_like_filter_lines() {
    let righe = log(6, 300, false);
    let (doc, p) = documento(&righe, false, true);
    let livelli = [true, true, true, true];
    let tutte = atteso(&righe, 0, doc.len(), None, false, livelli);
    assert!(tutte.len() > 20);
    let (out, offsets, n, tronc, _) = esegui(
        &doc,
        &filtro(None, Mode::Text, false, livelli),
        0,
        doc.len(),
        20,
    );
    assert_eq!((n, tronc), (20, true));
    assert_eq!(offsets.len(), 20);
    assert_eq!(out, uscita_attesa(&righe, &tutte[..20], false));
    // il limite uguale al numero di righe: non e' troncato
    let (_, _, n, tronc, _) = esegui(
        &doc,
        &filtro(None, Mode::Text, false, livelli),
        0,
        doc.len(),
        tutte.len() as u64,
    );
    assert_eq!((n as usize, tronc), (tutte.len(), false));
    let _ = std::fs::remove_file(p);
}

#[test]
fn crlf_lines_and_a_last_line_without_a_newline_are_written_right() {
    let righe = log(7, 80, true);
    // con CRLF il livello si trova lo stesso e l'uscita tiene il \r\n
    let (doc, p) = documento(&righe, true, true);
    let livelli = [true, false, false, false];
    let (out, _, n, _, _) = esegui(
        &doc,
        &filtro(None, Mode::Text, false, livelli),
        0,
        doc.len(),
        u64::MAX,
    );
    let idx = atteso(&righe, 0, doc.len(), None, false, livelli);
    assert_eq!(out, uscita_attesa(&righe, &idx, true));
    assert_eq!(n as usize, idx.len());
    let _ = std::fs::remove_file(p);
    // l'ultima riga senza a-capo: se entra, l'uscita lo aggiunge
    let righe = log(8, 40, false);
    let (doc, p) = documento(&righe, false, false);
    let (out, _, n, _, _) = esegui(
        &doc,
        &filtro(None, Mode::Text, false, [true, true, true, true]),
        0,
        doc.len(),
        u64::MAX,
    );
    let idx = atteso(&righe, 0, doc.len(), None, false, [true, true, true, true]);
    assert_eq!(out, uscita_attesa(&righe, &idx, false));
    assert_eq!(n as usize, idx.len());
    assert!(out.ends_with('\n'));
    let _ = std::fs::remove_file(p);
}

#[test]
fn cancel_stops_the_filter() {
    let righe = log(9, 200, false);
    let (doc, p) = documento(&righe, false, true);
    let snap = doc.snapshot().unwrap();
    let r = snap.filter_lines_where(
        &filtro(None, Mode::Text, false, [true, false, false, false]),
        0,
        doc.len(),
        u64::MAX,
        &mut Vec::new(),
        &mut Vec::new(),
        &AtomicBool::new(true),
        &AtomicU64::new(0),
    );
    assert!(matches!(r, Err(SearchError::Cancelled)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_line_that_starts_exactly_at_hi_is_left_out_and_the_one_at_lo_is_in() {
    let righe = log(6, 50, false);
    let (doc, p) = documento(&righe, false, true);
    let (lo, hi) = (righe[10].offset, righe[20].offset);
    let (_, offsets, n, _, _) = esegui(
        &doc,
        &filtro(None, Mode::Text, false, NESSUNO),
        lo,
        hi,
        u64::MAX,
    );
    assert_eq!(n, 10);
    assert_eq!(offsets.first(), Some(&lo));
    assert_eq!(offsets.last(), Some(&righe[19].offset));
    // un `lo` in mezzo a una riga salta quella riga
    let (_, offsets, n, _, _) = esegui(
        &doc,
        &filtro(None, Mode::Text, false, NESSUNO),
        lo + 1,
        hi,
        u64::MAX,
    );
    assert_eq!((n, offsets.first()), (9, Some(&righe[11].offset)));
    let _ = std::fs::remove_file(p);
}
