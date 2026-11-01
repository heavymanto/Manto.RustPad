//! Test del junior per `timesearch` (fase F3): i casi limite del PLAN che il test di accettazione non copre.
//! La riga lunghissima senza orario in mezzo al log, l'ultima riga senza a-capo, il target con la frazione (e
//! la virgola come separatore sulla riga), la data e l'orario separati da due spazi, i secondi 60 e il target
//! senza data oltre un'intestazione. Gli attesi sono gli offset calcolati mentre il test scrive il file.

use mrp_core::timesearch::{find_time, parse_target};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!(
            "mrp_timesearch_jun_{}_{}.log",
            std::process::id(),
            n
        ));
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

#[test]
fn a_very_long_line_without_a_time_is_crossed() {
    let mut t = String::new();
    let mut inizi = Vec::new();
    for i in 0..3u32 {
        inizi.push(t.len() as u64);
        t.push_str(&format!("2026-10-31 09:00:{i:02} prima {i}\n"));
    }
    // una riga di quindicimila byte senza orario, in mezzo al log
    t.push_str(&"x".repeat(15_000));
    t.push('\n');
    for i in 0..3u32 {
        inizi.push(t.len() as u64);
        t.push_str(&format!("2026-10-31 09:01:{i:02} dopo {i}\n"));
    }
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    assert_eq!(cerca(&d, "2026-10-31 09:00:00"), Some(inizi[0]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:02"), Some(inizi[2]));
    // il primo orario oltre le righe di prima e' quello oltre la riga lunga
    assert_eq!(cerca(&d, "2026-10-31 09:00:03"), Some(inizi[3]));
    assert_eq!(cerca(&d, "2026-10-31 09:01:01"), Some(inizi[4]));
    assert_eq!(cerca(&d, "2026-10-31 09:01:05"), None);
}

#[test]
fn the_last_line_without_a_newline_is_found() {
    let mut t = String::new();
    let mut inizi = Vec::new();
    for riga in ["2026-10-31 09:00:00 a\n", "2026-10-31 09:00:05 b\n"] {
        inizi.push(t.len() as u64);
        t.push_str(riga);
    }
    inizi.push(t.len() as u64);
    t.push_str("2026-10-31 09:00:10 senza a-capo");
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    assert_eq!(cerca(&d, "2026-10-31 08:00"), Some(inizi[0]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:07"), Some(inizi[2]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:10"), Some(inizi[2]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:11"), None);
}

#[test]
fn a_target_with_a_fraction_finds_the_millisecond() {
    let mut t = String::new();
    let mut inizi = Vec::new();
    for riga in [
        "2026-10-31 09:00:53.000 a\n",
        "2026-10-31 09:00:53.250 b\n",
        "2026-10-31 09:00:53,500 c\n",
        "2026-10-31 09:00:54 d\n",
    ] {
        inizi.push(t.len() as u64);
        t.push_str(riga);
    }
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    assert_eq!(cerca(&d, "2026-10-31 09:00:53"), Some(inizi[0]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:53.250"), Some(inizi[1]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:53.251"), Some(inizi[2]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:53.600"), Some(inizi[3]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:53.999"), Some(inizi[3]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:54.001"), None);
    // le frazioni corte valgono come le lunghe: ".25" e' ".250", e "09:00" e' "09:00:00"
    assert_eq!(
        parse_target("2026-10-31 09:00:53.25"),
        parse_target("2026-10-31 09:00:53.250")
    );
    assert_eq!(parse_target("09:00"), parse_target("09:00:00"));
    assert_ne!(parse_target("09:00"), parse_target("09:00:00.001"));
}

#[test]
fn a_line_with_the_date_and_the_time_separated_by_two_spaces() {
    let mut t = String::new();
    let mut inizi = Vec::new();
    for riga in [
        "2026-10-31  09:00:00 due spazi\n",
        "2026-10-31  09:00:01 ancora\n",
    ] {
        inizi.push(t.len() as u64);
        t.push_str(riga);
    }
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    // senza l'unione dei due span la prima riga non verrebbe trovata (la data sola vale mezzanotte)
    assert_eq!(cerca(&d, "2026-10-31 08:59"), Some(inizi[0]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:00"), Some(inizi[0]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:01"), Some(inizi[1]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:02"), None);
}

#[test]
fn a_line_with_sixty_seconds_counts_as_fifty_nine_and_a_half() {
    let mut t = String::new();
    let mut inizi = Vec::new();
    for riga in [
        "2026-10-31 09:00:59 a\n",
        "2026-10-31 09:00:60 b\n",
        "2026-10-31 09:01:00 c\n",
    ] {
        inizi.push(t.len() as u64);
        t.push_str(riga);
    }
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    // 09:00:60 vale 09:00:59.500: dopo .400 e .500, prima di .600
    assert_eq!(cerca(&d, "2026-10-31 09:00:59.400"), Some(inizi[1]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:59.500"), Some(inizi[1]));
    assert_eq!(cerca(&d, "2026-10-31 09:00:59.600"), Some(inizi[2]));
}

#[test]
fn a_time_only_target_takes_the_date_after_a_header() {
    let mut t = String::from("# log aperto\nquesta riga non ha orario\n");
    let mut inizi = Vec::new();
    for i in 0..5u32 {
        inizi.push(t.len() as u64);
        t.push_str(&format!("2026-10-31 09:{i:02}:00 messaggio {i}\n"));
    }
    let f = TempFile::new(t.as_bytes());
    let d = Document::open(&f.0).unwrap();
    // il target senza data usa la data della prima riga con orario, oltre l'intestazione
    assert_eq!(cerca(&d, "08:00"), Some(inizi[0]));
    assert_eq!(cerca(&d, "09:03"), Some(inizi[3]));
    assert_eq!(cerca(&d, "09:05"), None);
}
