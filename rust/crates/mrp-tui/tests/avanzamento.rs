//! L'avanzamento dei lavori lunghi per i file grandi: compare dopo 2 secondi e solo da 64 MiB in su, con la velocita' degli ultimi
//! secondi e il tempo che resta; nella riga di stato si allarga con lo schermo. Scritti da ARCH.

use mrp_core::lang::{registry_in, Catalog};
use mrp_tui::app::{Avanzamento, Busy, Misura, MOSTRA_DA_BYTE};
use mrp_tui::ui::{dettaglio_avanzamento, testo_avanzamento};
use std::time::{Duration, Instant};

const MIB: u64 = 1024 * 1024;

fn secondi(t0: Instant, s: f64) -> Instant {
    t0 + Duration::from_secs_f64(s)
}

fn catalogo() -> Catalog {
    Catalog::english()
}

#[test]
fn nothing_shows_before_two_seconds_nor_for_a_small_file() {
    let t0 = Instant::now();
    let mut m = Misura::default();
    let grande = 1000 * MIB;
    assert_eq!(m.registra(Some(Busy::Indexing), 0, grande, t0), None);
    assert_eq!(
        m.registra(Some(Busy::Indexing), 10 * MIB, grande, secondi(t0, 1.9)),
        None,
        "meno di 2 secondi"
    );
    assert!(m
        .registra(Some(Busy::Indexing), 20 * MIB, grande, secondi(t0, 2.0))
        .is_some());
    // un file piccolo non lo mostra mai, per quanto ci metta
    let mut p = Misura::default();
    let piccolo = MOSTRA_DA_BYTE - 1;
    p.registra(Some(Busy::Indexing), 0, piccolo, t0);
    assert_eq!(
        p.registra(Some(Busy::Indexing), 1, piccolo, secondi(t0, 30.0)),
        None
    );
}

#[test]
fn speed_and_time_left_come_from_the_last_seconds_not_from_the_start() {
    let t0 = Instant::now();
    let mut m = Misura::default();
    let totale = 1000 * MIB;
    // i primi secondi: 100 MiB/s
    for s in 0..=4 {
        m.registra(
            Some(Busy::Indexing),
            s * 100 * MIB,
            totale,
            secondi(t0, s as f64),
        );
    }
    // poi il disco rallenta a 25 MiB/s (non piu' dalla cache)
    let mut ultimo = None;
    for s in 5..=12 {
        let fatti = 400 * MIB + (s - 4) * 25 * MIB;
        ultimo = m.registra(Some(Busy::Indexing), fatti, totale, secondi(t0, s as f64));
    }
    let a = ultimo.unwrap();
    let v = a.velocita.unwrap() / MIB;
    assert_eq!(v, 25, "la velocita' segue gli ultimi 3 secondi: {v} MiB/s");
    // restano 1000 - 600 = 400 MiB a 25 MiB/s: 16 secondi (non i 4 che direbbe la media dall'inizio)
    assert_eq!(a.resta, Some(Duration::from_secs(16)));
    assert_eq!(a.percento(), 60);
    assert_eq!(a.trascorso, Duration::from_secs(12));
}

#[test]
fn the_time_left_is_not_guessed_at_first_nor_while_the_job_stands_still() {
    let t0 = Instant::now();
    let mut m = Misura::default();
    let totale = 1000 * MIB;
    m.registra(Some(Busy::Searching), 0, totale, t0);
    // a 2 secondi c'e' l'avanzamento ma la stima ha solo mezzo secondo di misure sopra il minimo: arriva con i campioni
    let a = m
        .registra(Some(Busy::Searching), 50 * MIB, totale, secondi(t0, 2.0))
        .unwrap();
    assert!(a.velocita.is_some());
    // fermo (l'indicizzazione in pausa mentre si salta nel file): niente velocita' e niente stima, non un tempo infinito
    let mut f = Misura::default();
    f.registra(Some(Busy::Indexing), 300 * MIB, totale, t0);
    for s in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
        f.registra(Some(Busy::Indexing), 300 * MIB, totale, secondi(t0, s));
    }
    let a = f
        .registra(Some(Busy::Indexing), 300 * MIB, totale, secondi(t0, 7.0))
        .unwrap();
    assert_eq!((a.velocita, a.resta), (None, None));
}

#[test]
fn a_different_job_or_one_that_starts_over_resets_the_clock() {
    let t0 = Instant::now();
    let mut m = Misura::default();
    let totale = 1000 * MIB;
    m.registra(Some(Busy::Indexing), 0, totale, t0);
    assert!(m
        .registra(Some(Busy::Indexing), 100 * MIB, totale, secondi(t0, 3.0))
        .is_some());
    // la ricerca che parte adesso non eredita i 3 secondi dell'indicizzazione
    assert_eq!(
        m.registra(Some(Busy::Searching), 0, totale, secondi(t0, 3.1)),
        None
    );
    // un lavoro che riparte da capo (i byte tornano indietro) neppure
    assert_eq!(
        m.registra(Some(Busy::Searching), 50 * MIB, totale, secondi(t0, 6.0))
            .map(|a| a.trascorso),
        Some(Duration::from_secs_f64(2.9))
    );
    m.registra(Some(Busy::Searching), 10 * MIB, totale, secondi(t0, 6.1));
    assert_eq!(
        m.registra(Some(Busy::Searching), 20 * MIB, totale, secondi(t0, 6.5)),
        None
    );
    // finito il lavoro le misure si azzerano
    assert_eq!(m.registra(None, 0, 0, secondi(t0, 7.0)), None);
}

fn esempio() -> Avanzamento {
    Avanzamento {
        lavoro: Busy::Indexing,
        fatti: 1200 * MIB,
        totale: 2700 * MIB,
        trascorso: Duration::from_secs(14),
        velocita: Some(85 * MIB),
        resta: Some(Duration::from_secs(17)),
    }
}

#[test]
fn the_status_row_piece_grows_with_the_width_of_the_screen() {
    let cat = catalogo();
    let a = esempio();
    // stretto: solo il testo di sempre
    assert_eq!(
        testo_avanzamento(&cat, &a, "indexing 44%", 59),
        ("indexing 44%".to_string(), None)
    );
    // da 60 colonne: la barra, 12 celle e 5 piene (44% di 12)
    let (t, barra) = testo_avanzamento(&cat, &a, "indexing 44%", 60);
    assert_eq!(t, "indexing 44% #####.......");
    assert_eq!(barra, Some((13, 5, 12)));
    // da 90: anche il tempo che resta
    let (t, _) = testo_avanzamento(&cat, &a, "indexing 44%", 90);
    assert_eq!(t, "indexing 44% #####.......  0:17 left");
    // da 140: i byte fatti sui totali e la velocita'
    let (t, _) = testo_avanzamento(&cat, &a, "indexing 44%", 140);
    assert_eq!(
        t,
        "indexing 44% #####.......  1.17 GiB / 2.64 GiB  85.00 MiB/s  0:17 left"
    );
    // senza stima (appena partito) non si inventa il tempo
    let senza = Avanzamento {
        velocita: None,
        resta: None,
        ..a
    };
    let (t, _) = testo_avanzamento(&cat, &senza, "indexing 44%", 140);
    assert_eq!(t, "indexing 44% #####.......  1.17 GiB / 2.64 GiB");
}

#[test]
fn the_window_detail_has_the_bytes_the_speed_and_the_time_left() {
    let cat = catalogo();
    assert_eq!(
        dettaglio_avanzamento(&cat, &esempio()),
        "1.17 GiB / 2.64 GiB \u{b7} 85.00 MiB/s \u{b7} 0:17 left"
    );
}

#[test]
fn every_language_says_the_time_left_with_the_number_in_it() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang");
    let (langs, avvisi) = registry_in(Some(&dir));
    assert!(avvisi.is_empty(), "{avvisi:?}");
    assert_eq!(langs.len(), 7);
    for l in &langs {
        let cat = Catalog::with(l.clone());
        let s = cat.tf("status.eta", &["0:17"]);
        assert!(s.contains("0:17"), "{}: {s:?}", l.code);
    }
}
