//! Salta a un orario (fase 2, 3.4): `timesearch::find_time` cerca, con una ricerca binaria sui byte (senza indice e senza
//! leggere il file), l'inizio della prima riga il cui orario e' maggiore o uguale a quello cercato, in un log con gli orari in
//! ordine crescente. Le righe senza orario (le righe di una traccia, un'intestazione) si saltano. Scritti da ARCH: gli attesi
//! li sa il test, che scrive il file e si ricorda dove comincia ogni riga e che orario le ha dato.

use mrp_core::timesearch::{find_time, parse_target};
use mrp_core::{Cancelled, Document};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_timesearch_{}_{}.log", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn cerca(d: &Document, target: &str) -> Option<u64> {
    let t = parse_target(target).unwrap_or_else(|| panic!("orario non valido: {target}"));
    find_time(d, &t, &AtomicBool::new(false)).unwrap()
}

/// Secondi dall'inizio del giorno 2026-10-31 come "2026-10-31 HH:MM:SS" (anche oltre le 24 ore: passa ai giorni dopo, con
/// ottobre di 31 giorni e novembre di 30).
fn data_ora(secondi: u64) -> String {
    let giorni = secondi / 86_400;
    let r = secondi % 86_400;
    let (mese, giorno) = match giorni {
        0 => (10, 31),
        g if g <= 30 => (11, g),
        g => (12, g - 30),
    };
    format!(
        "2026-{mese:02}-{giorno:02} {:02}:{:02}:{:02}",
        r / 3600,
        (r / 60) % 60,
        r % 60
    )
}

/// Un log con una riga ogni `passo` secondi; ogni 5a riga e' seguita da una riga di traccia senza orario. Restituisce il
/// testo, e per ogni riga con orario (secondi, offset d'inizio).
fn log(righe: u64, passo: u64, intestazione: bool, fine_riga: &str) -> (Vec<u8>, Vec<(u64, u64)>) {
    let mut t = String::new();
    if intestazione {
        t.push_str(&format!(
            "# log aperto{fine_riga}questa riga non ha orario{fine_riga}"
        ));
    }
    let mut v = Vec::new();
    for i in 0..righe {
        let s = i * passo;
        v.push((s, t.len() as u64));
        t.push_str(&format!("{} [INFO] messaggio {i}{fine_riga}", data_ora(s)));
        if i % 5 == 4 {
            t.push_str(&format!("    at una.traccia.Di(Stack.java:{i}){fine_riga}"));
        }
    }
    (t.into_bytes(), v)
}

#[test]
fn the_targets_that_can_be_typed() {
    for ok in [
        "2026-10-31 09:00:53",
        "2026-10-31T09:00",
        "2026-10-31 09:00:53.250",
        "2026-10-31",
        "09:00",
        "09:00:53",
        "  09:00  ",
    ] {
        assert!(parse_target(ok).is_some(), "{ok:?}");
    }
    for no in [
        "",
        "abc",
        "2026-13-01",
        "2026-10-32",
        "25:00",
        "09:60",
        "9:00:00:00",
        "2026-10-31 25:00",
        "09",
    ] {
        assert!(parse_target(no).is_none(), "{no:?}");
    }
}

#[test]
fn an_exact_time_a_time_between_lines_before_the_start_and_after_the_end() {
    let (dati, righe) = log(1000, 30, false, "\n");
    let t = TempFile::new(&dati);
    let d = Document::open(&t.0).unwrap();
    // l'ora esatta di ogni 37a riga
    for (k, &(s, off)) in righe.iter().enumerate().step_by(37) {
        assert_eq!(cerca(&d, &data_ora(s)), Some(off), "riga {k}");
        // un secondo dopo: la riga dopo
        if k + 1 < righe.len() {
            assert_eq!(
                cerca(&d, &data_ora(s + 1)),
                Some(righe[k + 1].1),
                "riga {k} + 1 s"
            );
        }
        // un secondo prima: la riga stessa
        if s > 0 {
            assert_eq!(cerca(&d, &data_ora(s - 1)), Some(off), "riga {k} - 1 s");
        }
    }
    // prima di tutto: la prima riga con orario; dopo tutto: niente
    assert_eq!(cerca(&d, "2026-10-30 00:00:00"), Some(0));
    assert_eq!(cerca(&d, "2026-10-31 00:00:00"), Some(0));
    let (ultimo, off) = righe[999];
    assert_eq!(cerca(&d, &data_ora(ultimo)), Some(off));
    assert_eq!(cerca(&d, &data_ora(ultimo + 1)), None);
    assert_eq!(cerca(&d, "2027-01-01"), None);
}

#[test]
fn header_lines_without_a_time_are_skipped_and_crlf_works_the_same() {
    for fine in ["\n", "\r\n"] {
        let (dati, righe) = log(300, 60, true, fine);
        let t = TempFile::new(&dati);
        let d = Document::open(&t.0).unwrap();
        // prima di tutto: la prima riga con orario, che non e' a 0 (ci sono due righe di intestazione)
        assert_ne!(righe[0].1, 0);
        assert_eq!(cerca(&d, "2026-10-31 00:00:00"), Some(righe[0].1));
        for k in [0usize, 1, 5, 99, 150, 299] {
            assert_eq!(
                cerca(&d, &data_ora(righe[k].0)),
                Some(righe[k].1),
                "{fine:?} riga {k}"
            );
        }
        assert_eq!(cerca(&d, &data_ora(righe[299].0 + 1)), None);
    }
}

#[test]
fn equal_times_give_the_first_line_of_the_group() {
    let mut t = String::new();
    let mut inizi = Vec::new();
    for i in 0..200u64 {
        // quattro righe per ogni secondo
        if i % 4 == 0 {
            inizi.push(t.len() as u64);
        }
        t.push_str(&format!("{} riga {i}\n", data_ora(i / 4)));
    }
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    for s in [0u64, 1, 7, 25, 49] {
        assert_eq!(
            cerca(&d, &data_ora(s)),
            Some(inizi[s as usize]),
            "secondo {s}"
        );
    }
}

#[test]
fn times_only_logs_and_a_target_without_a_date() {
    // "HH:MM:SS msg", una riga al secondo da 09:00:00
    let mut t = String::new();
    let mut inizi = Vec::new();
    for i in 0..3600u64 {
        inizi.push(t.len() as u64);
        t.push_str(&format!(
            "{:02}:{:02}:{:02} msg {i}\n",
            9 + i / 3600,
            (i / 60) % 60,
            i % 60
        ));
    }
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    assert_eq!(cerca(&d, "09:30:00"), Some(inizi[1800]));
    assert_eq!(cerca(&d, "09:30"), Some(inizi[1800]));
    assert_eq!(cerca(&d, "09:59:59"), Some(inizi[3599]));
    assert_eq!(cerca(&d, "10:00:00"), None);
    assert_eq!(cerca(&d, "08:00"), Some(0));
}

#[test]
fn a_time_only_target_on_a_log_with_dates_uses_the_date_of_its_first_line() {
    // un giorno solo, una riga al minuto da 2026-10-31 00:00
    let (dati, righe) = log(600, 60, false, "\n");
    let f = TempFile::new(&dati);
    let d = Document::open(&f.0).unwrap();
    // 03:20:00 = 12000 s = riga 200
    assert_eq!(cerca(&d, "03:20"), Some(righe[200].1));
}

#[test]
fn a_date_only_target_finds_the_first_line_of_that_day() {
    // una riga ogni 10 minuti per tre giorni (432 righe): 2026-10-31, 2026-11-01, 2026-11-02
    let (dati, righe) = log(432, 600, false, "\n");
    let f = TempFile::new(&dati);
    let d = Document::open(&f.0).unwrap();
    assert_eq!(cerca(&d, "2026-10-31"), Some(righe[0].1));
    assert_eq!(cerca(&d, "2026-11-01"), Some(righe[144].1));
    assert_eq!(cerca(&d, "2026-11-02"), Some(righe[288].1));
    assert_eq!(cerca(&d, "2026-11-03"), None);
    assert_eq!(cerca(&d, "2026-11-01 12:00"), Some(righe[144 + 72].1));
}

#[test]
fn iso_times_with_fractions_and_zones() {
    let mut t = String::new();
    let mut inizi = Vec::new();
    for i in 0..400u64 {
        inizi.push(t.len() as u64);
        let ms = i * 250;
        t.push_str(&format!(
            "2026-10-31T09:{:02}:{:02}.{:03}Z evento {i}\n",
            (ms / 60_000) % 60,
            (ms / 1000) % 60,
            ms % 1000
        ));
    }
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    // 09:00:50 = 50000 ms = riga 200
    assert_eq!(cerca(&d, "2026-10-31T09:00:50"), Some(inizi[200]));
    assert_eq!(cerca(&d, "2026-10-31 09:01:00"), Some(inizi[240]));
}

#[test]
fn a_file_without_times_or_empty_gives_nothing() {
    let f = TempFile::new(b"niente\naltro\nancora\n");
    let d = Document::open(&f.0).unwrap();
    assert_eq!(cerca(&d, "09:00"), None);
    let v = TempFile::new(b"");
    let d = Document::open(&v.0).unwrap();
    assert_eq!(cerca(&d, "09:00"), None);
}

#[test]
fn a_cancelled_search_says_so() {
    let (dati, _) = log(100, 30, false, "\n");
    let t = TempFile::new(&dati);
    let d = Document::open(&t.0).unwrap();
    let c = AtomicBool::new(true);
    let target = parse_target("2026-10-31 00:20:00").unwrap();
    assert_eq!(find_time(&d, &target, &c), Err(Cancelled));
}

#[test]
fn a_big_log_is_searched_without_reading_it() {
    // circa 20 MB: 400.000 righe da 50 byte. L'oracolo e' la conoscenza del test: la riga k ha l'orario k*2 secondi.
    let (dati, righe) = log(400_000, 2, false, "\n");
    assert!(dati.len() > 15_000_000);
    let t = TempFile::new(&dati);
    let d = Document::open(&t.0).unwrap();
    let mut seme: u64 = 99;
    for _ in 0..25 {
        seme = seme
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let k = ((seme >> 33) % 399_999) as usize;
        assert_eq!(
            cerca(&d, &data_ora(righe[k].0)),
            Some(righe[k].1),
            "riga {k}"
        );
        assert_eq!(
            cerca(&d, &data_ora(righe[k].0 + 1)).map(|o| o > righe[k].1),
            Some(true)
        );
    }
}

#[test]
fn a_very_long_trace_without_times_does_not_hide_the_lines_after_it() {
    // 200 righe con orario, poi una traccia di 500 righe senza orario, poi 200 righe con orario: la ricerca puo' cadere dentro la
    // traccia e deve comunque trovare le righe dopo
    let mut t = String::new();
    let mut inizi = Vec::new();
    for i in 0..400u64 {
        if i == 200 {
            for k in 0..500 {
                t.push_str(&format!("    at una.traccia.Lunga(Stack.java:{k})\n"));
            }
        }
        inizi.push(t.len() as u64);
        t.push_str(&format!("{} riga {i}\n", data_ora(i * 30)));
    }
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    for k in [0usize, 100, 199, 200, 201, 300, 399] {
        assert_eq!(
            cerca(&d, &data_ora(k as u64 * 30)),
            Some(inizi[k]),
            "riga {k}"
        );
    }
    assert_eq!(cerca(&d, &data_ora(199 * 30 + 1)), Some(inizi[200]));
}
