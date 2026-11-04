//! I livelli sulla timeline di una scheda (F3): i conteggi per riga della barra (anche raggruppati), la scelta dei livelli e la vista
//! filtrata che si apre. Scritti da ARCH: 144 righe, una ogni mezz'ora dal 2026-11-01, con il livello che gira ERROR, WARN, INFO, DEBUG
//! (la riga `i` ha il livello `i % 4`): 48 righe al giorno, 12 per livello; due righe all'ora.

use mrp_core::timeline::Step;
use mrp_tui::app::timeline::TimelineStatus;
use mrp_tui::app::{App, AppConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn app() -> (App, PathBuf) {
    let mut dati = String::new();
    for i in 0..144u32 {
        let livello = ["ERROR", "WARN", "INFO", "DEBUG"][(i % 4) as usize];
        dati.push_str(&format!(
            "2026-11-{:02} {:02}:{:02}:00 [{livello}] riga {i}\n",
            1 + i / 48,
            (i % 48) / 2,
            (i % 2) * 30
        ));
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlev_app_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

/// Gira finche' le tacche e i livelli sono pronti.
fn pronta(a: &mut App) {
    let fine = Instant::now() + Duration::from_secs(20);
    loop {
        a.timeline_tick();
        let finito = a.timeline_status() == TimelineStatus::Ready && a.timeline_levels_ready();
        if finito || Instant::now() >= fine {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_rows_carry_the_counts_of_every_level_per_day() {
    let (mut a, p) = app();
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    pronta(&mut a);
    assert!(a.timeline_levels_ready());
    let righe = a.timeline_rows(100);
    assert_eq!(righe.len(), 3);
    for (d, r) in righe.iter().enumerate() {
        assert!(r.levels_known, "giorno {d}");
        assert_eq!(
            (r.levels.error, r.levels.warn, r.levels.info, r.levels.debug),
            (12, 12, 12, 12),
            "giorno {d}"
        );
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn grouped_rows_add_up_the_levels_of_their_buckets() {
    let (mut a, p) = app();
    a.set_timeline(true);
    a.set_timeline_step(Step::Hour);
    pronta(&mut a);
    // un'ora: due righe, nelle ore pari ERROR e WARN, nelle dispari INFO e DEBUG
    let ore = a.timeline_rows(1000);
    assert_eq!(ore.len(), 72);
    assert_eq!(
        (ore[0].levels.error, ore[0].levels.warn, ore[0].levels.info),
        (1, 1, 0)
    );
    assert_eq!(
        (ore[1].levels.error, ore[1].levels.info, ore[1].levels.debug),
        (0, 1, 1)
    );
    // 72 ore in al piu' 10 righe: gruppi di 8 ore, quattro pari e quattro dispari: quattro righe di ogni livello
    let gruppi = a.timeline_rows(10);
    assert_eq!(gruppi.len(), 9);
    for r in &gruppi {
        assert_eq!(
            (r.levels.error, r.levels.warn, r.levels.info, r.levels.debug),
            (4, 4, 4, 4)
        );
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn new_ticks_throw_the_old_levels_away_and_switching_off_stops_the_pass() {
    let (mut a, p) = app();
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    pronta(&mut a);
    assert!(a.timeline_levels_ready());
    // un passo diverso: tacche nuove e livelli da ricontare; a lavoro finito le righe sono quelle delle ore, con i loro livelli
    a.set_timeline_step(Step::Hour);
    pronta(&mut a);
    assert!(a.timeline_levels_ready());
    let ore = a.timeline_rows(1000);
    assert_eq!(ore.len(), 72);
    assert!(ore.iter().all(|r| r.levels_known));
    assert_eq!((ore[0].levels.error, ore[0].levels.warn), (1, 1));
    // spenta: non resta niente
    a.set_timeline(false);
    assert!(!a.timeline_levels_ready());
    assert!(a.timeline_levels_progress().is_none());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_chosen_levels_open_a_view_with_only_their_lines() {
    let (mut a, p) = app();
    assert_eq!(a.timeline_filter_levels(), [false; 4]);
    // nessun livello scelto: lo dice e non parte niente
    a.timeline_apply_level_filter();
    assert_eq!(
        a.flash(),
        Some("Choose at least one level first (E, W, I, D)")
    );
    assert!(a.take_filter_done().is_none());
    // ERROR e WARN: 72 righe delle 144
    a.set_timeline_filter_level(0, true);
    a.set_timeline_filter_level(1, true);
    assert_eq!(a.timeline_filter_levels(), [true, true, false, false]);
    a.timeline_apply_level_filter();
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("la vista filtrata");
    assert_eq!(fatto.lines, 72);
    // la scheda si chiama coi livelli scelti, non con l'espressione regolare
    assert_eq!(fatto.pattern, "levels: ERROR, WARN");
    // solo DEBUG: 36 righe
    a.set_timeline_filter_level(0, false);
    a.set_timeline_filter_level(1, false);
    a.set_timeline_filter_level(3, true);
    a.timeline_apply_level_filter();
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("DEBUG");
    assert_eq!((fatto.lines, fatto.pattern.as_str()), (36, "levels: DEBUG"));
    let _ = std::fs::remove_file(p);
}
