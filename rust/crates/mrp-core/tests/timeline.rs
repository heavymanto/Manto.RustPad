//! La timeline (F2): le tacche ai confini dei bucket di un log (minuto, dieci minuti, ora, giorno) trovate SENZA leggere il file
//! per intero (`timeline::build`), e la ricerca di una chiave dentro un intervallo di byte (`timesearch::find_key_in`). Scritti da
//! ARCH prima dell'implementazione: l'oracolo e' un conteggio ingenuo sul `Vec` delle righe di partenza (offset e chiave di ogni
//! riga calcolati a mano), mai la ricerca. Le date dei test stanno tutte a novembre 2026, quindi si scrivono a mano.

use mrp_core::lines::Cancelled;
use mrp_core::timeline::{build, BuildError, Step};
use mrp_core::timemask::TimeMask;
use mrp_core::timesearch::find_key_in;
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

/// Una riga di prova: il testo, la sua chiave (se ha l'orario) e dove comincia nel file.
struct Riga {
    testo: String,
    chiave: Option<i64>,
    offset: u64,
}

/// L'orario di una riga, `ms` dopo la mezzanotte del 2026-11-01 (scritto a mano, senza il codice sotto prova): `yyyy-MM-dd HH:mm:ss`.
fn scritto(ms: i64) -> String {
    let giorno = 1 + ms / GIORNO;
    let t = ms % GIORNO;
    let (h, m, s) = (t / ORA, (t / 60_000) % 60, (t / 1000) % 60);
    format!("2026-11-{giorno:02} {h:02}:{m:02}:{s:02}")
}

/// Un log di prova con orari non decrescenti (ripetizioni e buchi, anche di giorni interi) e righe senza orario in mezzo. Parte
/// alle 05:00 del primo giorno; `passi` e' il salto massimo in secondi fra due righe con orario.
fn log(seme: u64, n: usize, passi: u64) -> Vec<Riga> {
    let mut r = Lcg(seme);
    let mut ms = 5 * ORA;
    let mut righe = Vec::new();
    let mut offset = 0u64;
    for _ in 0..n {
        let (testo, chiave) = match r.sotto(8) {
            0 => ("\tat com.example.Main.run(Main.java:41)".to_string(), None),
            1 => ("| tabella | senza | orario |".to_string(), None),
            _ => {
                ms += (r.sotto(passi) as i64) * 1000;
                (
                    format!("{} [INFO ] riga", scritto(ms)),
                    Some(MEZZANOTTE + ms),
                )
            }
        };
        let lung = testo.len() as u64 + 1;
        righe.push(Riga {
            testo,
            chiave,
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
    let p = std::env::temp_dir().join(format!("mrp_tline_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    (Document::open(&p).unwrap(), p)
}

fn maschera() -> TimeMask {
    TimeMask::parse("yyyy-MM-dd HH:mm:ss", 0).unwrap()
}

/// L'inizio del bucket di `chiave` (passo `ms`): la chiave tagliata al multiplo, come la mezzanotte per il giorno.
fn piano(chiave: i64, ms: i64) -> i64 {
    chiave - chiave.rem_euclid(ms)
}

/// Il primo offset, fra le righe con orario in `[lo, hi)`, la cui chiave e' >= `inizio`; `hi` se non ce n'e'.
fn atteso_offset(righe: &[Riga], inizio: i64, lo: u64, hi: u64) -> u64 {
    righe
        .iter()
        .filter(|r| r.offset >= lo && r.offset < hi)
        .find(|r| r.chiave.is_some_and(|k| k >= inizio))
        .map_or(hi, |r| r.offset)
}

/// Le tacche attese per `[lo, hi)` col passo `ms`: da `piano(prima)` a `piano(ultima)`, un bucket ogni `ms`; la prima tacca sta in
/// `lo` (le righe senza orario prima della prima con orario sono del primo bucket).
fn atteso(righe: &[Riga], ms: i64, lo: u64, hi: u64) -> Vec<(i64, u64)> {
    let chiavi: Vec<i64> = righe
        .iter()
        .filter(|r| r.offset >= lo && r.offset < hi)
        .filter_map(|r| r.chiave)
        .collect();
    let (prima, ultima) = (chiavi[0], *chiavi.last().unwrap());
    let mut v = Vec::new();
    let mut inizio = piano(prima, ms);
    while inizio <= piano(ultima, ms) {
        let off = if v.is_empty() {
            lo
        } else {
            atteso_offset(righe, inizio, lo, hi)
        };
        v.push((inizio, off));
        inizio += ms;
    }
    v
}

fn libero() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn the_steps_know_their_length_and_where_a_bucket_starts() {
    assert_eq!(Step::Minute.ms(), 60_000);
    assert_eq!(Step::TenMinutes.ms(), 600_000);
    assert_eq!(Step::Hour.ms(), ORA);
    assert_eq!(Step::Day.ms(), GIORNO);
    // 2026-11-01 05:07:09.123 -> l'inizio dell'ora, dei dieci minuti, del minuto e del giorno
    let k = MEZZANOTTE + 5 * ORA + 7 * 60_000 + 9_123;
    assert_eq!(Step::Minute.floor(k), MEZZANOTTE + 5 * ORA + 7 * 60_000);
    assert_eq!(Step::TenMinutes.floor(k), MEZZANOTTE + 5 * ORA);
    assert_eq!(Step::Hour.floor(k), MEZZANOTTE + 5 * ORA);
    assert_eq!(Step::Day.floor(k), MEZZANOTTE);
    // prima del 1970 il passo taglia verso il basso (non verso zero)
    assert_eq!(Step::Day.floor(-1), -GIORNO);
}

#[test]
fn day_buckets_match_a_naive_count_and_cover_the_file() {
    let righe = log(7, 400, 900);
    let (doc, p) = documento(&righe);
    let t = build(
        &doc,
        &maschera(),
        Step::Day,
        0,
        doc.len(),
        &libero(),
        10_000,
        None,
    )
    .unwrap();
    let attesi = atteso(&righe, GIORNO, 0, doc.len());
    assert!(attesi.len() >= 2, "il log deve attraversare piu' giorni");
    let visti: Vec<(i64, u64)> = t.ticks.iter().map(|k| (k.start, k.offset)).collect();
    assert_eq!(visti, attesi);
    // le tacche dividono il file in pezzi contigui: la prima in 0, ogni pezzo e' la differenza, la somma e' la lunghezza
    assert_eq!(t.ticks[0].offset, 0);
    assert_eq!((t.lo, t.hi), (0, doc.len()));
    let somma: u64 = (0..t.ticks.len()).map(|i| t.bytes(i)).sum();
    assert_eq!(somma, doc.len());
    assert_eq!(t.first, righe.iter().find_map(|r| r.chiave).unwrap());
    assert_eq!(t.last, righe.iter().rev().find_map(|r| r.chiave).unwrap());
    let _ = std::fs::remove_file(p);
}

#[test]
fn hour_and_ten_minute_buckets_match_the_naive_count_on_several_logs() {
    for (seme, n, passi, passo) in [
        (1u64, 300usize, 600u64, Step::Hour),
        (2, 500, 60, Step::TenMinutes),
        (3, 700, 30, Step::Minute),
        (4, 250, 7200, Step::Hour),
    ] {
        let righe = log(seme, n, passi);
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
        let attesi = atteso(&righe, passo.ms(), 0, doc.len());
        let visti: Vec<(i64, u64)> = t.ticks.iter().map(|k| (k.start, k.offset)).collect();
        assert_eq!(visti, attesi, "seme {seme} passo {passo:?}");
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn a_day_without_lines_is_an_empty_bucket() {
    // lunedi 05:00 .. poi un salto di quasi tre giorni: i giorni di mezzo non hanno righe
    let mut righe = Vec::new();
    let mut offset = 0u64;
    for ms in [
        5 * ORA,
        5 * ORA + 60_000,
        5 * ORA + 120_000,
        3 * GIORNO + 2 * ORA,
        3 * GIORNO + 2 * ORA + 1000,
    ] {
        let testo = format!("{} [INFO ] x", scritto(ms));
        let lung = testo.len() as u64 + 1;
        righe.push(Riga {
            testo,
            chiave: Some(MEZZANOTTE + ms),
            offset,
        });
        offset += lung;
    }
    let (doc, p) = documento(&righe);
    let t = build(
        &doc,
        &maschera(),
        Step::Day,
        0,
        doc.len(),
        &libero(),
        100,
        None,
    )
    .unwrap();
    let visti: Vec<(i64, u64)> = t.ticks.iter().map(|k| (k.start, k.offset)).collect();
    // quattro bucket (1, 2, 3 e 4 novembre): il primo ha tre righe, il secondo e il terzo nessuna (stesso offset del quarto)
    assert_eq!(
        visti,
        vec![
            (MEZZANOTTE, 0),
            (MEZZANOTTE + GIORNO, righe[3].offset),
            (MEZZANOTTE + 2 * GIORNO, righe[3].offset),
            (MEZZANOTTE + 3 * GIORNO, righe[3].offset),
        ]
    );
    assert_eq!(t.bytes(0), righe[3].offset);
    assert_eq!(t.bytes(1), 0);
    assert_eq!(t.bytes(2), 0);
    assert_eq!(t.bytes(3), doc.len() - righe[3].offset);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_line_exactly_on_the_boundary_belongs_to_the_new_bucket() {
    let mut righe = Vec::new();
    let mut offset = 0u64;
    for ms in [GIORNO - 2000, GIORNO - 1000, GIORNO, GIORNO + 1000] {
        let testo = format!("{} [INFO ] x", scritto(ms));
        let lung = testo.len() as u64 + 1;
        righe.push(Riga {
            testo,
            chiave: Some(MEZZANOTTE + ms),
            offset,
        });
        offset += lung;
    }
    let (doc, p) = documento(&righe);
    let t = build(
        &doc,
        &maschera(),
        Step::Day,
        0,
        doc.len(),
        &libero(),
        100,
        None,
    )
    .unwrap();
    let visti: Vec<(i64, u64)> = t.ticks.iter().map(|k| (k.start, k.offset)).collect();
    assert_eq!(
        visti,
        vec![(MEZZANOTTE, 0), (MEZZANOTTE + GIORNO, righe[2].offset)]
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_window_gets_its_own_buckets_and_they_partition_it() {
    let righe = log(11, 600, 600);
    let (doc, p) = documento(&righe);
    // la finestra: da una riga a una riga piu' avanti
    let (lo, hi) = (righe[150].offset, righe[480].offset);
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
    let attesi = atteso(&righe, ORA, lo, hi);
    let visti: Vec<(i64, u64)> = t.ticks.iter().map(|k| (k.start, k.offset)).collect();
    assert_eq!(visti, attesi);
    assert_eq!(t.ticks[0].offset, lo);
    assert_eq!((t.lo, t.hi), (lo, hi));
    let somma: u64 = (0..t.ticks.len()).map(|i| t.bytes(i)).sum();
    assert_eq!(somma, hi - lo);
    let _ = std::fs::remove_file(p);
}

#[test]
fn bucket_at_maps_an_offset_to_the_bucket_that_holds_it() {
    let righe = log(5, 500, 900);
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
    for (i, k) in t.ticks.iter().enumerate() {
        if t.bytes(i) == 0 {
            continue;
        }
        assert_eq!(t.bucket_at(k.offset), Some(i), "inizio del bucket {i}");
        assert_eq!(
            t.bucket_at(k.offset + t.bytes(i) - 1),
            Some(i),
            "fine del bucket {i}"
        );
    }
    assert_eq!(t.bucket_at(doc.len()), None, "oltre la fine");
    let _ = std::fs::remove_file(p);
}

#[test]
fn errors_no_times_too_many_buckets_and_cancel() {
    // un file senza orari
    let senza: Vec<Riga> = (0..20)
        .map(|i| Riga {
            testo: format!("riga {i} senza orario"),
            chiave: None,
            offset: 0,
        })
        .collect();
    let (doc, p) = documento(&senza);
    let e = build(
        &doc,
        &maschera(),
        Step::Day,
        0,
        doc.len(),
        &libero(),
        100,
        None,
    )
    .unwrap_err();
    assert_eq!(e, BuildError::NoTimes);
    let _ = std::fs::remove_file(p);
    // troppi bucket: il passo al minuto su piu' di tre giorni, con un tetto di 100 tacche; dice quante sarebbero
    let righe = log(9, 600, 1200);
    let (doc, p) = documento(&righe);
    let attese = atteso(&righe, 60_000, 0, doc.len()).len();
    assert!(attese > 100);
    let e = build(
        &doc,
        &maschera(),
        Step::Minute,
        0,
        doc.len(),
        &libero(),
        100,
        None,
    )
    .unwrap_err();
    assert_eq!(e, BuildError::TooManyBuckets(attese));
    // annullato: subito
    let annulla = AtomicBool::new(true);
    let e = build(
        &doc,
        &maschera(),
        Step::Day,
        0,
        doc.len(),
        &annulla,
        100,
        None,
    )
    .unwrap_err();
    assert_eq!(e, BuildError::Cancelled);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_progress_counter_reaches_the_number_of_buckets() {
    let righe = log(13, 400, 900);
    let (doc, p) = documento(&righe);
    let fatte = AtomicUsize::new(0);
    let t = build(
        &doc,
        &maschera(),
        Step::Hour,
        0,
        doc.len(),
        &libero(),
        10_000,
        Some(&fatte),
    )
    .unwrap();
    assert_eq!(fatte.load(Ordering::Relaxed), t.ticks.len());
    let _ = std::fs::remove_file(p);
}

#[test]
fn find_key_in_searches_only_inside_the_given_bytes() {
    let righe = log(21, 500, 600);
    let (doc, p) = documento(&righe);
    let m = maschera();
    let (lo, hi) = (righe[100].offset, righe[400].offset);
    // l'oracolo: la prima riga con chiave >= bersaglio fra le righe con l'inizio in [lo, hi)
    let mut r = Lcg(99);
    for _ in 0..60 {
        let k = MEZZANOTTE + 5 * ORA + (r.sotto(40 * 3600) as i64) * 1000;
        let atteso = righe
            .iter()
            .filter(|x| x.offset >= lo && x.offset < hi)
            .find(|x| x.chiave.is_some_and(|c| c >= k))
            .map(|x| x.offset);
        assert_eq!(
            find_key_in(&doc, &m, k, lo, hi, &libero()),
            Ok(atteso),
            "{k}"
        );
    }
    // un intervallo vuoto non trova niente; annullato da' Err
    assert_eq!(find_key_in(&doc, &m, 0, lo, lo, &libero()), Ok(None));
    assert_eq!(
        find_key_in(&doc, &m, 0, lo, hi, &AtomicBool::new(true)),
        Err(Cancelled)
    );
    let _ = std::fs::remove_file(p);
}
