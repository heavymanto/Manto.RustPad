//! La ricerca di un orario con una maschera (F1): `key_of_line`, `first_key`, `last_key` e `find_key` di `timesearch`, che
//! generalizzano il salto a un orario a qualunque formato riconosciuto da `TimeMask`. Scritti da ARCH: l'oracolo e' un conteggio
//! ingenuo sul `Vec` delle righe di partenza (offset e chiave di ogni riga calcolati a mano), mai la ricerca.

use mrp_core::lines::Cancelled;
use mrp_core::timemask::TimeMask;
use mrp_core::timesearch::{find_key, first_key, key_of_line, last_key};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// 2026-11-01 02:00:00.000 in millisecondi dal 1970 (calendar.timegm di Python).
const BASE: i64 = 1793498400000;

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

/// L'orario di una riga nel giorno 2026-11-01, `ms` dopo le 02:00:00.000 (scritto a mano, senza il codice sotto prova).
fn scritto(ms: i64) -> String {
    let t = 2 * 3600 * 1000 + ms;
    let (h, m, s, f) = (t / 3_600_000, (t / 60_000) % 60, (t / 1000) % 60, t % 1000);
    format!("2026-11-01 {h:02}:{m:02}:{s:02}.{f:03}")
}

/// Un log di prova: orari non decrescenti (con ripetizioni e buchi), righe senza orario in mezzo (tracce, vuote, lunghe).
fn log(seme: u64, n: usize) -> Vec<Riga> {
    let mut r = Lcg(seme);
    let mut ms = 0i64;
    let mut righe = Vec::new();
    let mut offset = 0u64;
    for i in 0..n {
        let (testo, chiave) = match r.sotto(10) {
            0 => ("\tat com.example.Main.run(Main.java:41)".to_string(), None),
            1 => (String::new(), None),
            2 if i % 7 == 0 => (format!("riga lunga {}", "x".repeat(900)), None),
            _ => {
                ms += match r.sotto(4) {
                    0 => 0,
                    1 => 1,
                    2 => r.sotto(500) as i64,
                    _ => r.sotto(60_000) as i64,
                };
                (
                    format!("{} [INFO ] riga {i} e un po' di testo", scritto(ms)),
                    Some(BASE + ms),
                )
            }
        };
        let lunga = testo.len() as u64 + 1;
        righe.push(Riga {
            testo,
            chiave,
            offset,
        });
        offset += lunga;
    }
    righe
}

fn scrivi(righe: &[Riga]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_timekey_{}_{}.log", std::process::id(), n));
    let mut dati = String::new();
    for r in righe {
        dati.push_str(&r.testo);
        dati.push('\n');
    }
    std::fs::write(&p, dati).unwrap();
    p
}

fn maschera() -> TimeMask {
    TimeMask::parse("yyyy-MM-dd HH:mm:ss.SSS", 0).unwrap()
}

fn libero() -> AtomicBool {
    AtomicBool::new(false)
}

/// La risposta attesa di `find_key`: l'inizio della prima riga (in ordine di file) con chiave >= `t`.
fn atteso(righe: &[Riga], t: i64) -> Option<u64> {
    righe
        .iter()
        .find(|r| r.chiave.is_some_and(|k| k >= t))
        .map(|r| r.offset)
}

#[test]
fn find_key_gives_the_first_line_at_or_after_the_time_for_every_target() {
    for seme in 1..=25u64 {
        let n = 1 + (seme as usize * 9) % 220;
        let righe = log(seme, n);
        let p = scrivi(&righe);
        let doc = Document::open(&p).unwrap();
        let m = maschera();
        // i bersagli: prima di tutto, ogni chiave e il suo vicino da una parte e dall'altra, e dopo l'ultima
        let mut bersagli: Vec<i64> = vec![BASE - 1_000_000, BASE - 1];
        for r in &righe {
            if let Some(k) = r.chiave {
                bersagli.extend([k - 1, k, k + 1]);
            }
        }
        bersagli.push(BASE + 100_000_000);
        for t in bersagli {
            assert_eq!(
                find_key(&doc, &m, t, &libero()).unwrap(),
                atteso(&righe, t),
                "seme {seme} bersaglio {t}"
            );
        }
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn key_of_line_reads_the_line_that_contains_the_position() {
    let righe = log(7, 120);
    let p = scrivi(&righe);
    let doc = Document::open(&p).unwrap();
    let m = maschera();
    for r in &righe {
        let fine = r.offset + r.testo.len() as u64; // la posizione dell'a-capo
        for pos in [r.offset, r.offset + (r.testo.len() as u64) / 2, fine] {
            assert_eq!(
                key_of_line(&doc, &m, pos),
                r.chiave,
                "pos {pos} nella riga che comincia a {}",
                r.offset
            );
        }
    }
    // oltre la fine del documento non c'e' niente
    assert_eq!(key_of_line(&doc, &m, doc.len() + 5), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn first_and_last_key_skip_the_lines_without_a_time() {
    for seme in [3u64, 11, 19] {
        let righe = log(seme, 150);
        let p = scrivi(&righe);
        let doc = Document::open(&p).unwrap();
        let m = maschera();
        let prima = righe.iter().find(|r| r.chiave.is_some()).unwrap();
        let ultima = righe.iter().rev().find(|r| r.chiave.is_some()).unwrap();
        assert_eq!(
            first_key(&doc, &m, &libero()).unwrap(),
            Some((prima.offset, prima.chiave.unwrap())),
            "seme {seme}"
        );
        assert_eq!(
            last_key(&doc, &m, &libero()).unwrap(),
            Some((ultima.offset, ultima.chiave.unwrap())),
            "seme {seme}"
        );
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn a_document_without_times_or_empty_has_nothing() {
    let senza = vec![
        Riga {
            testo: "solo testo".into(),
            chiave: None,
            offset: 0,
        },
        Riga {
            testo: "ancora testo".into(),
            chiave: None,
            offset: 11,
        },
    ];
    let p = scrivi(&senza);
    let doc = Document::open(&p).unwrap();
    let m = maschera();
    assert_eq!(find_key(&doc, &m, BASE, &libero()).unwrap(), None);
    assert_eq!(first_key(&doc, &m, &libero()).unwrap(), None);
    assert_eq!(last_key(&doc, &m, &libero()).unwrap(), None);
    let _ = std::fs::remove_file(p);
    let vuoto = scrivi(&[]);
    let doc = Document::open(&vuoto).unwrap();
    assert_eq!(find_key(&doc, &m, BASE, &libero()).unwrap(), None);
    assert_eq!(first_key(&doc, &m, &libero()).unwrap(), None);
    assert_eq!(last_key(&doc, &m, &libero()).unwrap(), None);
    let _ = std::fs::remove_file(vuoto);
}

#[test]
fn a_cancel_stops_the_search() {
    let righe = log(5, 80);
    let p = scrivi(&righe);
    let doc = Document::open(&p).unwrap();
    let m = maschera();
    let annullato = AtomicBool::new(true);
    assert!(matches!(
        find_key(&doc, &m, BASE + 5000, &annullato),
        Err(Cancelled)
    ));
    assert!(matches!(first_key(&doc, &m, &annullato), Err(Cancelled)));
    assert!(matches!(last_key(&doc, &m, &annullato), Err(Cancelled)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn unsaved_edits_are_searched_too() {
    let mut righe = log(9, 60);
    let p = scrivi(&righe);
    let mut doc = Document::open(&p).unwrap();
    let m = maschera();
    // si inserisce, all'inizio della riga 30, una riga con un orario fra quelli vicini (tenendo l'ordine)
    let i = 30usize;
    let k_prima = righe[..i]
        .iter()
        .rev()
        .find_map(|r| r.chiave)
        .unwrap_or(BASE);
    let nuova_ms = k_prima - BASE;
    let testo = format!("{} [WARN ] inserita a mano", scritto(nuova_ms));
    let mut inserita = testo.clone().into_bytes();
    inserita.push(b'\n');
    doc.insert(righe[i].offset, &inserita).unwrap();
    // l'oracolo: la stessa modifica sul Vec
    let lunga = inserita.len() as u64;
    for r in righe.iter_mut().skip(i) {
        r.offset += lunga;
    }
    let offset_nuova = righe[i].offset - lunga;
    righe.insert(
        i,
        Riga {
            testo,
            chiave: Some(BASE + nuova_ms),
            offset: offset_nuova,
        },
    );
    for t in [
        BASE + nuova_ms - 1,
        BASE + nuova_ms,
        BASE + nuova_ms + 1,
        BASE + 100_000_000,
    ] {
        assert_eq!(
            find_key(&doc, &m, t, &libero()).unwrap(),
            atteso(&righe, t),
            "bersaglio {t}"
        );
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_mask_with_a_window_finds_times_that_are_not_at_the_start() {
    // righe nello stile di un access log: l'orario sta dopo un indirizzo di lunghezza variabile
    let ip = ["127.0.0.1", "10.20.4.137", "192.168.50.8", "::1"];
    let mut dati = String::new();
    let mut offsets = Vec::new();
    for s in 0..40u32 {
        offsets.push(dati.len() as u64);
        dati.push_str(&format!(
            "{} - - [01/Nov/2026:02:{:02}:{:02} +0100] \"GET /{s} HTTP/1.1\" 200 12\n",
            ip[(s % 4) as usize],
            s / 60,
            s % 60
        ));
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_timekey_acc_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let doc = Document::open(&p).unwrap();
    let m = TimeMask::parse("dd/MMM/yyyy:HH:mm:ss", 40).unwrap();
    // il secondo 17 dopo le 02:00:00
    let t = BASE + 17_000;
    assert_eq!(find_key(&doc, &m, t, &libero()).unwrap(), Some(offsets[17]));
    assert_eq!(
        find_key(&doc, &m, t + 1, &libero()).unwrap(),
        Some(offsets[18])
    );
    assert_eq!(first_key(&doc, &m, &libero()).unwrap(), Some((0, BASE)));
    assert_eq!(
        last_key(&doc, &m, &libero()).unwrap(),
        Some((offsets[39], BASE + 39_000))
    );
    let _ = std::fs::remove_file(p);
}
