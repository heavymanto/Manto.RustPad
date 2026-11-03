//! Test del junior per la ricerca con la maschera (task H2): due formati che il test di accettazione
//! di ARCH (`timekey.rs`) non usa (il log4j con la virgola, il syslog con l'anno fissato). Gli
//! offset sono le somme delle lunghezze delle righe e le chiavi i millisecondi dal 1970 contati a
//! mano (2026-11-01 00:00:00 = 1_793_491_200_000), mai letti dalla
//! ricerca.

use mrp_core::timemask::TimeMask;
use mrp_core::timesearch::{find_key, first_key, key_of_line, last_key};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// 2026-11-01 00:00:00.000 (mezzanotte) in millisecondi dal 1970 (calendar.timegm di Python; i commenti sopra erano sbagliati
/// di due ore: 1_793_498_400_000 e' gia' le 02:00).
const BASE: i64 = 1_793_491_200_000;

struct TempFile(PathBuf);

impl TempFile {
    fn new(testo: &str) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_timekey_jun_{}_{}.log", std::process::id(), n));
        std::fs::write(&p, testo).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn libero() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn the_jump_with_the_comma_of_the_milliseconds() {
    // un log4j: la frazione con la virgola e una riga senza orario in mezzo; le righe sono lunghe
    // 30, 22, 32 e 30 byte con l'a-capo, quindi gli inizi sono 0, 30, 52 e 84
    let mut t = String::new();
    t.push_str("2026-11-01 02:00:00,100 prima\n");
    t.push_str("una riga senza orario\n");
    t.push_str("2026-11-01 02:00:00,250 seconda\n");
    t.push_str("2026-11-01 03:00:00,000 terza\n");
    let f = TempFile::new(&t);
    let doc = Document::open(&f.0).unwrap();
    let m = TimeMask::parse("yyyy-MM-dd HH:mm:ss,SSS", 0).unwrap();
    assert_eq!(
        find_key(&doc, &m, BASE + 7_199_999, &libero()).unwrap(),
        Some(0)
    );
    assert_eq!(
        find_key(&doc, &m, BASE + 7_200_250, &libero()).unwrap(),
        Some(52)
    );
    assert_eq!(
        find_key(&doc, &m, BASE + 7_200_251, &libero()).unwrap(),
        Some(84)
    );
    assert_eq!(
        find_key(&doc, &m, BASE + 10_800_001, &libero()).unwrap(),
        None
    );
    assert_eq!(
        first_key(&doc, &m, &libero()).unwrap(),
        Some((0, BASE + 7_200_100))
    );
    assert_eq!(
        last_key(&doc, &m, &libero()).unwrap(),
        Some((84, BASE + 10_800_000))
    );
    // la riga senza orario non ha chiave da nessuna posizione interna, e oltre la fine non c'e' niente
    assert_eq!(key_of_line(&doc, &m, 40), None);
    assert_eq!(key_of_line(&doc, &m, 55), Some(BASE + 7_200_250));
    assert_eq!(key_of_line(&doc, &m, 1000), None);
}

#[test]
fn the_jump_with_syslog_and_a_fixed_year() {
    // syslog senza anno: l'anno lo da' with_year (il 2024, bisestile); 2024-02-28 = 19781 giorni dal
    // 1970 e 2024-02-29 = 19782 (calcolati a mano); le righe sono lunghe 23 byte con l'a-capo
    let mut t = String::new();
    t.push_str("Feb 28 23:59:59 host a\n");
    t.push_str("Feb 29 00:00:00 host b\n");
    t.push_str("Feb 29 00:00:01 host c\n");
    let f = TempFile::new(&t);
    let doc = Document::open(&f.0).unwrap();
    let m = TimeMask::parse("MMM d HH:mm:ss", 0)
        .unwrap()
        .with_year(2024);
    assert_eq!(
        find_key(&doc, &m, 1_709_164_799_001, &libero()).unwrap(),
        Some(23)
    );
    assert_eq!(
        find_key(&doc, &m, 1_709_164_800_000, &libero()).unwrap(),
        Some(23)
    );
    assert_eq!(
        find_key(&doc, &m, 1_709_164_801_000, &libero()).unwrap(),
        Some(46)
    );
    assert_eq!(
        find_key(&doc, &m, 1_709_164_801_001, &libero()).unwrap(),
        None
    );
    assert_eq!(
        first_key(&doc, &m, &libero()).unwrap(),
        Some((0, 1_709_164_799_000))
    );
    assert_eq!(
        last_key(&doc, &m, &libero()).unwrap(),
        Some((46, 1_709_164_801_000))
    );
}
