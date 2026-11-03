//! I test del JUN per la timeline (F2): i casi che i test di ARCH non coprono — l'identita' di
//! `find_key_in` con `find_key` (su tutto il documento e con una finestra oltre la fine del file),
//! il tetto dei bucket raggiunto esattamente, la finestra vuota, `bytes` e `bucket_at` fuori range,
//! il `floor` sui multipli esatti e il galloping su un tratto lungo senza orari.

use mrp_core::timeline::{build, BuildError, Step};
use mrp_core::timemask::TimeMask;
use mrp_core::timesearch::{find_key, find_key_in};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// 2026-11-01 00:00:00.000 in millisecondi dal 1970 (calendar.timegm di Python).
const MEZZANOTTE: i64 = 1793491200000;
const ORA: i64 = 3_600_000;

/// L'orario di una riga, `ms` dopo la mezzanotte del 2026-11-01 (scritto a mano, senza il codice
/// sotto prova): `yyyy-MM-dd HH:mm:ss`.
fn scritto(ms: i64) -> String {
    let (h, m, s) = (ms / 3_600_000, (ms / 60_000) % 60, (ms / 1000) % 60);
    format!("2026-11-01 {h:02}:{m:02}:{s:02}")
}

/// Un'ora di log, un minuto per riga, con una traccia senza orario ogni tre righe.
fn log_con_tracce() -> Vec<String> {
    let mut righe = Vec::new();
    for m in 0..60i64 {
        if m % 3 == 2 {
            righe.push("\tat com.example.Main.run(Main.java:41)".to_string());
        } else {
            righe.push(format!("{} [INFO ] riga", scritto(m * 60_000)));
        }
    }
    righe
}

/// Scrive le righe in un file temporaneo (una per riga, con l'a-capo) e lo apre.
fn documento(righe: &[String]) -> (Document, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlinej_{}_{}.log", std::process::id(), n));
    let mut dati = String::new();
    for r in righe {
        dati.push_str(r);
        dati.push('\n');
    }
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
fn the_floor_of_a_bucket_start_is_itself() {
    // l'inizio di un bucket resta fisso, per ogni passo e anche sotto zero
    for (step, ms) in [
        (Step::Minute, 60_000),
        (Step::TenMinutes, 600_000),
        (Step::Hour, ORA),
        (Step::Day, 86_400_000),
    ] {
        assert_eq!(step.ms(), ms);
        assert_eq!(step.floor(0), 0);
        assert_eq!(step.floor(ms), ms);
        assert_eq!(step.floor(2 * ms), 2 * ms);
        assert_eq!(step.floor(-1), -ms);
        assert_eq!(step.floor(-ms), -ms);
        assert_eq!(step.floor(-ms - 1), -2 * ms);
    }
}

#[test]
fn find_key_in_over_the_whole_document_is_find_key() {
    let righe = log_con_tracce();
    let (doc, p) = documento(&righe);
    let m = maschera();
    for t in [
        0,
        MEZZANOTTE - 1,
        MEZZANOTTE,
        MEZZANOTTE + 29 * 60_000,
        MEZZANOTTE + 59 * 60_000,
        MEZZANOTTE + 61 * 60_000,
    ] {
        assert_eq!(
            find_key_in(&doc, &m, t, 0, doc.len(), &libero()),
            find_key(&doc, &m, t, &libero()),
            "bersaglio {t}"
        );
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_window_past_the_end_of_the_document_stops_at_the_end() {
    let righe = log_con_tracce();
    let (doc, p) = documento(&righe);
    let m = maschera();
    for t in [MEZZANOTTE, MEZZANOTTE + 30 * 60_000, MEZZANOTTE + 2 * ORA] {
        assert_eq!(
            find_key_in(&doc, &m, t, 0, doc.len() + 10_000, &libero()),
            find_key(&doc, &m, t, &libero()),
            "bersaglio {t}"
        );
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_bucket_count_equal_to_the_cap_is_accepted() {
    // due righe, 05:30 e 06:30: i bucket dell'ora sono due (quello delle 05 e quello delle 06)
    let righe = vec![
        format!("{} [INFO ] a", scritto(5 * ORA + 30 * 60_000)),
        format!("{} [INFO ] b", scritto(6 * ORA + 30 * 60_000)),
    ];
    let (doc, p) = documento(&righe);
    let t = build(
        &doc,
        &maschera(),
        Step::Hour,
        0,
        doc.len(),
        &libero(),
        2,
        None,
    )
    .unwrap();
    assert_eq!(t.ticks.len(), 2);
    assert_eq!(t.first, MEZZANOTTE + 5 * ORA + 30 * 60_000);
    assert_eq!(t.last, MEZZANOTTE + 6 * ORA + 30 * 60_000);
    // un tetto piu' basso dice quanti bucket servirebbero
    let e = build(
        &doc,
        &maschera(),
        Step::Hour,
        0,
        doc.len(),
        &libero(),
        1,
        None,
    )
    .unwrap_err();
    assert_eq!(e, BuildError::TooManyBuckets(2));
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_empty_window_has_no_times() {
    let righe = vec![format!("{} [INFO ] x", scritto(ORA))];
    let (doc, p) = documento(&righe);
    let e = build(&doc, &maschera(), Step::Hour, 7, 7, &libero(), 10, None).unwrap_err();
    assert_eq!(e, BuildError::NoTimes);
    let _ = std::fs::remove_file(p);
}

#[test]
fn bytes_and_bucket_at_out_of_range() {
    // due ore di log, un minuto per riga; la finestra parte dalla riga 10 e finisce alla 110
    let mut righe = Vec::new();
    for m in 0..120i64 {
        righe.push(format!("{} [INFO ] r{m}", scritto(m * 60_000)));
    }
    let mut off = 0u64;
    let (mut lo, mut hi) = (0u64, 0u64);
    for (i, r) in righe.iter().enumerate() {
        if i == 10 {
            lo = off;
        }
        if i == 110 {
            hi = off;
        }
        off += r.len() as u64 + 1;
    }
    let (doc, p) = documento(&righe);
    let t = build(&doc, &maschera(), Step::Hour, lo, hi, &libero(), 100, None).unwrap();
    let ultimo = t.ticks.len() - 1;
    // `bytes` fuori range vale zero; l'ultimo bucket copre fino a hi
    assert_eq!(t.bytes(t.ticks.len()), 0);
    assert_eq!(t.bytes(999), 0);
    assert_eq!(t.bytes(ultimo), hi - t.ticks[ultimo].offset);
    // `bucket_at` dentro la finestra trova il primo bucket, fuori no
    assert_eq!(t.bucket_at(lo), Some(0));
    assert_eq!(t.bucket_at(lo - 1), None);
    assert_eq!(t.bucket_at(hi), None);
    assert_eq!(t.bucket_at(hi + 1), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_gallop_skips_a_long_stretch_without_times() {
    // 05:00, poi tremila righe di traccia senza orario (circa 200 KB), poi 07:00: i bucket dei
    // dieci minuti attraversano il tratto e il galloping deve saltarlo con poche letture
    let mut righe = vec![format!("{} [INFO ] prima", scritto(5 * ORA))];
    for i in 0..3000 {
        righe.push(format!(
            "\tat com.example.Main.run(Main.java:{i}) e un po' di testo a riempire"
        ));
    }
    righe.push(format!("{} [INFO ] dopo", scritto(7 * ORA)));
    let (doc, p) = documento(&righe);
    let t = build(
        &doc,
        &maschera(),
        Step::TenMinutes,
        0,
        doc.len(),
        &libero(),
        100,
        None,
    )
    .unwrap();
    // l'oracolo: le tacche vanno da floor(05:00) a floor(07:00) ogni dieci minuti; la prima a 0, le
    // altre tutte sulla riga delle 07:00 (l'unica con chiave >= il proprio inizio)
    let ultima = righe.last().unwrap();
    let dopo = doc.len() - (ultima.len() as u64 + 1);
    let mut attesi = Vec::new();
    let mut inizio = MEZZANOTTE + 5 * ORA;
    while inizio <= MEZZANOTTE + 7 * ORA {
        let off = if attesi.is_empty() { 0 } else { dopo };
        attesi.push((inizio, off));
        inizio += 600_000;
    }
    let visti: Vec<(i64, u64)> = t.ticks.iter().map(|k| (k.start, k.offset)).collect();
    assert_eq!(visti, attesi);
    let _ = std::fs::remove_file(p);
}
