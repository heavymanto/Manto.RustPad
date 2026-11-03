//! Test del junior per i livelli dei log per bucket (F3): casi oltre a quelli di
//! `tests/timeline_levels.rs` — le righe CRLF (il `\r` finale tolto dalla riga), una riga non UTF-8
//! (niente livello, ma la passata continua), il livello oltre i 64 KiB di una riga lunghissima (non
//! si guarda) e l'ultima riga della finestra senza a-capo (conta lo stesso). Gli attesi sono scritti
//! a mano sui dati di prova, mai copiati dall'output del codice sotto test.

use mrp_core::timeline::{build, count_levels, LevelCounts, Step};
use mrp_core::timemask::TimeMask;
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Scrive `dati` in un file temporaneo e lo apre come documento.
fn documento(dati: &[u8]) -> (Document, PathBuf) {
    // un nome per file: i test girano in parallelo
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlevjun_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    (Document::open(&p).unwrap(), p)
}

fn maschera() -> TimeMask {
    TimeMask::parse("yyyy-MM-dd HH:mm:ss", 0).unwrap()
}

fn libero() -> AtomicBool {
    AtomicBool::new(false)
}

/// La somma dei conteggi di tutti i bucket, come quadrupla (error, warn, info, debug).
fn tot(conteggi: &[LevelCounts]) -> (u32, u32, u32, u32) {
    conteggi.iter().fold((0, 0, 0, 0), |a, c| {
        (a.0 + c.error, a.1 + c.warn, a.2 + c.info, a.3 + c.debug)
    })
}

#[test]
fn crlf_lines_are_counted_like_plain_ones() {
    // quattro righe CRLF, tutte nell'ora 05 (un solo bucket); l'ultima finisce col livello
    let dati = b"2026-11-01 05:00:00 [ERROR] uno\r\n\
                 2026-11-01 05:00:01 [WARN] due\r\n\
                 2026-11-01 05:00:02 [INFO] tre\r\n\
                 2026-11-01 05:00:03 DEBUG\r\n"
        .to_vec();
    let (doc, p) = documento(&dati);
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
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].error, c[0].warn, c[0].info, c[0].debug), (1, 1, 1, 1));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_non_utf8_line_has_no_level_but_the_pass_goes_on() {
    // la riga di mezzo ha due byte invalidi e un ERROR dentro: non e' UTF-8, niente livello;
    // la riga dopo conta lo stesso
    let mut dati = Vec::new();
    dati.extend_from_slice(b"2026-11-01 05:00:00 [INFO] prima\n");
    dati.extend_from_slice(b"2026-11-01 05:00:01 \xFF\xFE ERROR nel mezzo\n");
    dati.extend_from_slice(b"2026-11-01 05:00:02 [WARN] terza\n");
    let (doc, p) = documento(&dati);
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
    assert_eq!(tot(&c), (0, 1, 1, 0));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_level_beyond_the_first_64_kib_of_a_long_line_is_not_looked_at() {
    // una riga di ~66 KB col livello oltre i 64 KiB: non si guarda; la riga corta prima conta
    let mut dati = Vec::new();
    dati.extend_from_slice(b"2026-11-01 05:00:00 [WARN] corta\n");
    dati.extend_from_slice(b"2026-11-01 05:00:01 ");
    dati.extend_from_slice(&vec![b'x'; 66_000]);
    dati.extend_from_slice(b" [ERROR] in fondo\n");
    let (doc, p) = documento(&dati);
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
    assert_eq!(tot(&c), (0, 1, 0, 0));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_last_line_of_the_window_without_a_newline_counts() {
    // il file non finisce con l'a-capo: l'ultima riga conta lo stesso, col suo livello
    let dati = b"2026-11-01 05:00:00 [INFO] prima\n\
                 2026-11-01 05:00:01 [ERROR] ultima senza a-capo"
        .to_vec();
    let (doc, p) = documento(&dati);
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
    assert_eq!(tot(&c), (1, 0, 1, 0));
    let _ = std::fs::remove_file(p);
}
