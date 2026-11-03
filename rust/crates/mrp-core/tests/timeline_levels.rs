//! I livelli dei log per bucket (F3): `loghl::first_level` (il livello di una riga senza costruire gli span) e `timeline::count_levels`
//! (quante righe di ERROR, WARN, INFO e DEBUG ci sono in ogni bucket della timeline, con una passata sul file che si puo' annullare e
//! mettere in pausa). Scritti da ARCH prima dell'implementazione: l'oracolo e' un conteggio ingenuo sul `Vec` delle righe di partenza
//! (livello e offset di ogni riga scritti a mano), mai la passata. Le date dei test stanno a novembre 2026.

use mrp_core::lines::Cancelled;
use mrp_core::loghl::{first_level, line_level, Kind};
use mrp_core::timeline::{build, count_levels, LevelCounts, Step};
use mrp_core::timemask::TimeMask;
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// 2026-11-01 00:00:00.000 in millisecondi dal 1970 (calendar.timegm di Python).
const MEZZANOTTE: i64 = 1793491200000;
const GIORNO: i64 = 86_400_000;
const ORA: i64 = 3_600_000;

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

/// Una riga di prova: il testo, la sua chiave (se ha l'orario), il livello che ci aspettiamo e dove comincia nel file.
struct Riga {
    testo: String,
    /// La chiave dell'orario (non serve ai conteggi: resta per chi legge i dati di prova).
    #[allow(dead_code)]
    chiave: Option<i64>,
    livello: Option<Kind>,
    offset: u64,
}

fn scritto(ms: i64) -> String {
    let giorno = 1 + ms / GIORNO;
    let t = ms % GIORNO;
    format!(
        "2026-11-{giorno:02} {:02}:{:02}:{:02}",
        t / ORA,
        (t / 60_000) % 60,
        (t / 1000) % 60
    )
}

/// Un log con orari non decrescenti e livelli di ogni tipo (anche le sigle `INF`, `WRN`, `DBG`), righe senza orario (tabelle, tracce) che
/// qualche volta nominano un livello, e righe con un livello finto (`ERROR_CODE`, `info` minuscolo).
fn log(seme: u64, n: usize) -> Vec<Riga> {
    let mut r = Lcg(seme);
    let mut ms = 5 * ORA;
    let mut righe = Vec::new();
    let mut offset = 0u64;
    for _ in 0..n {
        let (testo, chiave, livello) = match r.sotto(12) {
            0 => ("| tabella | senza | livello |".to_string(), None, None),
            1 => (
                "\tat com.example.Main.run(Main.java:41)".to_string(),
                None,
                None,
            ),
            2 => (
                "ERROR in una riga senza orario".to_string(),
                None,
                Some(Kind::Error),
            ),
            _ => {
                ms += (r.sotto(900) as i64) * 1000;
                let (etichetta, livello) = match r.sotto(10) {
                    0 => ("[ERROR]", Some(Kind::Error)),
                    1 => ("[ERR]", Some(Kind::Error)),
                    2 => ("[WARN]", Some(Kind::Warn)),
                    3 => ("[WRN]", Some(Kind::Warn)),
                    4 => ("[INFO]", Some(Kind::Info)),
                    5 => ("[INF]", Some(Kind::Info)),
                    6 => ("[DEBUG]", Some(Kind::Debug)),
                    7 => ("[DBG]", Some(Kind::Debug)),
                    8 => ("ERROR_CODE=7 info minuscolo", None),
                    _ => ("solo testo", None),
                };
                (
                    format!("{} {etichetta} riga", scritto(ms)),
                    Some(MEZZANOTTE + ms),
                    livello,
                )
            }
        };
        let lung = testo.len() as u64 + 1;
        righe.push(Riga {
            testo,
            chiave,
            livello,
            offset,
        });
        offset += lung;
    }
    righe
}

fn documento(righe: &[Riga]) -> (Document, PathBuf) {
    let mut dati = String::new();
    for r in righe {
        dati.push_str(&r.testo);
        dati.push('\n');
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlev_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    (Document::open(&p).unwrap(), p)
}

fn maschera() -> TimeMask {
    TimeMask::parse("yyyy-MM-dd HH:mm:ss", 0).unwrap()
}

fn libero() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn the_abbreviations_inf_wrn_dbg_are_levels_like_the_full_words() {
    assert_eq!(first_level("2026-11-03 11:14:33 [INF] x"), Some(Kind::Info));
    assert_eq!(first_level("2026-11-03 11:14:33 [WRN] x"), Some(Kind::Warn));
    assert_eq!(
        first_level("2026-11-03 11:14:33 [DBG] x"),
        Some(Kind::Debug)
    );
    assert_eq!(
        first_level("2026-11-03 11:14:33 [TRC] x"),
        Some(Kind::Debug)
    );
    assert_eq!(
        first_level("2026-11-03 11:14:33 [ERR] x"),
        Some(Kind::Error)
    );
    // e quelli di prima restano
    assert_eq!(first_level("x ERROR y"), Some(Kind::Error));
    assert_eq!(first_level("x NOTICE y"), Some(Kind::Info));
    assert_eq!(first_level("x TRACE y"), Some(Kind::Debug));
    // il livello finto o minuscolo non conta; senza livello niente
    assert_eq!(first_level("ERROR_CODE=7"), None);
    assert_eq!(first_level("INFORMATION is not a level"), None);
    assert_eq!(first_level("an info line"), None);
    assert_eq!(first_level(""), None);
    // il primo livello della riga vince
    assert_eq!(first_level("WARN poi ERROR"), Some(Kind::Warn));
}

#[test]
fn first_level_agrees_with_line_level_on_every_line_of_a_big_log() {
    let righe = log(31, 3000);
    for r in &righe {
        assert_eq!(first_level(&r.testo), line_level(&r.testo), "{:?}", r.testo);
        assert_eq!(first_level(&r.testo), r.livello, "{:?}", r.testo);
    }
}

#[test]
fn the_counts_per_bucket_match_a_naive_count() {
    for (seme, n, passo) in [
        (1u64, 600usize, Step::Hour),
        (2, 900, Step::TenMinutes),
        (3, 400, Step::Day),
    ] {
        let righe = log(seme, n);
        let (doc, p) = documento(&righe);
        let t = build(
            &doc,
            &maschera(),
            passo,
            0,
            doc.len(),
            &libero(),
            100_000,
            None,
        )
        .unwrap();
        let conteggi = count_levels(&doc, &t, &libero(), None, None).unwrap();
        assert_eq!(conteggi.len(), t.ticks.len());
        // l'oracolo: ogni riga va nel bucket il cui intervallo di byte contiene il suo inizio
        let mut attesi = vec![LevelCounts::default(); t.ticks.len()];
        for r in &righe {
            let Some(i) = (0..t.ticks.len()).find(|&i| {
                t.bytes(i) > 0
                    && r.offset >= t.ticks[i].offset
                    && r.offset < t.ticks[i].offset + t.bytes(i)
            }) else {
                continue;
            };
            match r.livello {
                Some(Kind::Error) => attesi[i].error += 1,
                Some(Kind::Warn) => attesi[i].warn += 1,
                Some(Kind::Info) => attesi[i].info += 1,
                Some(Kind::Debug) => attesi[i].debug += 1,
                _ => {}
            }
        }
        assert_eq!(conteggi, attesi, "seme {seme} passo {passo:?}");
        // niente va perso: la somma e' il numero di righe con un livello
        let tot: u32 = conteggi
            .iter()
            .map(|c| c.error + c.warn + c.info + c.debug)
            .sum();
        assert_eq!(
            tot as usize,
            righe.iter().filter(|r| r.livello.is_some()).count()
        );
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn a_window_is_counted_inside_its_range_only() {
    let righe = log(5, 500);
    let (doc, p) = documento(&righe);
    let (lo, hi) = (righe[100].offset, righe[400].offset);
    let t = build(
        &doc,
        &maschera(),
        Step::Hour,
        lo,
        hi,
        &libero(),
        10_000,
        None,
    )
    .unwrap();
    let conteggi = count_levels(&doc, &t, &libero(), None, None).unwrap();
    let tot: u32 = conteggi
        .iter()
        .map(|c| c.error + c.warn + c.info + c.debug)
        .sum();
    let atteso = righe
        .iter()
        .filter(|r| r.offset >= lo && r.offset < hi && r.livello.is_some())
        .count();
    assert_eq!(tot as usize, atteso);
    let _ = std::fs::remove_file(p);
}

#[test]
fn progress_counts_the_bytes_and_ends_at_the_size_of_the_range() {
    let righe = log(7, 700);
    let (doc, p) = documento(&righe);
    let t = build(
        &doc,
        &maschera(),
        Step::Hour,
        0,
        doc.len(),
        &libero(),
        10_000,
        None,
    )
    .unwrap();
    let fatti = AtomicU64::new(0);
    count_levels(&doc, &t, &libero(), None, Some(&fatti)).unwrap();
    assert_eq!(fatti.load(Ordering::Relaxed), doc.len());
    let _ = std::fs::remove_file(p);
}

#[test]
fn cancel_stops_the_pass_and_a_pause_flag_that_is_off_does_not_slow_it() {
    let righe = log(9, 500);
    let (doc, p) = documento(&righe);
    let t = build(
        &doc,
        &maschera(),
        Step::Hour,
        0,
        doc.len(),
        &libero(),
        10_000,
        None,
    )
    .unwrap();
    // annullato subito
    assert_eq!(
        count_levels(&doc, &t, &AtomicBool::new(true), None, None),
        Err(Cancelled)
    );
    // la pausa spenta: la passata va
    let pausa = AtomicBool::new(false);
    assert!(count_levels(&doc, &t, &libero(), Some(&pausa), None).is_ok());
    // la pausa accesa con l'annullamento: vince l'annullamento (non resta bloccata)
    let pausa = AtomicBool::new(true);
    let annulla = AtomicBool::new(true);
    assert_eq!(
        count_levels(&doc, &t, &annulla, Some(&pausa), None),
        Err(Cancelled)
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn lines_crossing_the_read_chunks_are_counted_once() {
    // righe lunghe (decine di KiB) con il livello all'inizio: nessuna si conta due volte ne' si perde fra un pezzo di lettura e l'altro
    let mut righe = Vec::new();
    let mut offset = 0u64;
    for i in 0..60u32 {
        let livello = match i % 3 {
            0 => ("[ERROR]", Some(Kind::Error)),
            1 => ("[WARN]", Some(Kind::Warn)),
            _ => ("[INFO]", Some(Kind::Info)),
        };
        let testo = format!(
            "2026-11-01 05:{:02}:00 {} {}",
            i % 60,
            livello.0,
            "x".repeat(40_000 + (i as usize * 997) % 30_000)
        );
        let lung = testo.len() as u64 + 1;
        righe.push(Riga {
            testo,
            chiave: Some(MEZZANOTTE + 5 * ORA + i64::from(i) * 60_000),
            livello: livello.1,
            offset,
        });
        offset += lung;
    }
    let (doc, p) = documento(&righe);
    let t = build(
        &doc,
        &maschera(),
        Step::Hour,
        0,
        doc.len(),
        &libero(),
        100,
        None,
    )
    .unwrap();
    let c = count_levels(&doc, &t, &libero(), None, None).unwrap();
    let tot = c.iter().fold(LevelCounts::default(), |a, x| LevelCounts {
        error: a.error + x.error,
        warn: a.warn + x.warn,
        info: a.info + x.info,
        debug: a.debug + x.debug,
    });
    assert_eq!((tot.error, tot.warn, tot.info, tot.debug), (20, 20, 20, 0));
    let _ = std::fs::remove_file(p);
}
