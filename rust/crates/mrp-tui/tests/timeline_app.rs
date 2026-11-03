//! La timeline di una scheda (F2): spenta di fabbrica, si accende, un thread cerca le tacche, le righe della barra (raggruppate se
//! sono troppe), dov'e' la parte che si vede, il salto a un punto e l'orario sotto il mouse. Scritti da ARCH: tre giorni di righe
//! una ogni mezz'ora (144 righe), quindi gli offset e i volumi si calcolano a mano sul testo di partenza.

use mrp_core::timeline::Step;
use mrp_tui::app::timeline::{TimelineMode, TimelineStatus};
use mrp_tui::app::{App, AppConfig};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// 144 righe, una ogni mezz'ora dal 2026-11-01 00:00: `2026-11-DD HH:MM:00 [INFO ] riga N`; gli offset di ogni riga.
fn app() -> (App, Vec<u64>, PathBuf) {
    let mut dati = String::new();
    let mut offset = Vec::new();
    for i in 0..144u32 {
        offset.push(dati.len() as u64);
        dati.push_str(&format!(
            "2026-11-{:02} {:02}:{:02}:00 [INFO ] riga {i}\n",
            1 + i / 48,
            (i % 48) / 2,
            (i % 2) * 30
        ));
    }
    offset.push(dati.len() as u64);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlapp_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, offset, p)
}

/// Fa girare la timeline finche' e' pronta (o passa il tempo).
fn pronta(a: &mut App) {
    let fine = Instant::now() + Duration::from_secs(20);
    loop {
        a.timeline_tick();
        match a.timeline_status() {
            TimelineStatus::Building(_) if Instant::now() < fine => {
                std::thread::sleep(Duration::from_millis(5))
            }
            _ => return,
        }
    }
}

#[test]
fn it_is_off_by_default_and_switches_on_and_off() {
    let (mut a, _, p) = app();
    assert!(!a.timeline_on());
    assert_eq!(a.timeline_status(), TimelineStatus::Off);
    assert_eq!(a.timeline_step(), Step::Day);
    assert_eq!(a.timeline_mode(), TimelineMode::Time);
    assert!(a.timeline_rows(100).is_empty());
    a.toggle_timeline();
    assert!(a.timeline_on());
    pronta(&mut a);
    assert_eq!(a.timeline_status(), TimelineStatus::Ready);
    a.toggle_timeline();
    assert!(!a.timeline_on());
    assert_eq!(a.timeline_status(), TimelineStatus::Off);
    assert!(a.timeline_rows(100).is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_rows_are_the_days_with_their_offsets_volumes_and_labels() {
    let (mut a, offset, p) = app();
    a.set_timeline(true);
    pronta(&mut a);
    let righe = a.timeline_rows(100);
    assert_eq!(righe.len(), 3);
    for (d, r) in righe.iter().enumerate() {
        assert_eq!(r.offset, offset[48 * d], "giorno {d}");
        assert_eq!(r.bytes, offset[48 * (d + 1)] - offset[48 * d], "giorno {d}");
        assert_eq!(r.label, format!("2026-11-{:02}", d + 1));
        // in modo tempo ogni giorno e' alto un terzo
        assert!((r.y0 - d as f32 / 3.0).abs() < 1e-6 && (r.y1 - (d + 1) as f32 / 3.0).abs() < 1e-6);
    }
    // la lunghezza e' il volume rispetto al giorno piu' pesante (le righe hanno 1, 2 o 3 cifre: i giorni pesano un po' diverso)
    let massimo = righe.iter().map(|r| r.bytes).max().unwrap() as f32;
    for r in &righe {
        assert!((r.weight - r.bytes as f32 / massimo).abs() < 1e-5, "{r:?}");
    }
    assert!(righe.iter().any(|r| (r.weight - 1.0).abs() < 1e-6));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_shorter_step_gives_more_rows_and_labels_with_the_hour() {
    let (mut a, offset, p) = app();
    a.set_timeline(true);
    a.set_timeline_step(Step::Hour);
    pronta(&mut a);
    let righe = a.timeline_rows(1000);
    // 3 giorni da mezzanotte a mezzanotte: 72 ore, due righe per ora
    assert_eq!(righe.len(), 72);
    assert_eq!(righe[5].offset, offset[10]);
    assert_eq!(righe[5].bytes, offset[12] - offset[10]);
    assert_eq!(righe[5].label, "11-01 05:00");
    let _ = std::fs::remove_file(p);
}

#[test]
fn too_many_buckets_for_the_height_are_grouped() {
    let (mut a, offset, p) = app();
    a.set_timeline(true);
    a.set_timeline_step(Step::Hour);
    pronta(&mut a);
    // 72 ore in al piu' 10 righe: gruppi di 8 ore, quindi 9 righe da 16 righe di testo l'una
    let righe = a.timeline_rows(10);
    assert_eq!(righe.len(), 9);
    assert_eq!(righe[1].offset, offset[16]);
    assert_eq!(righe[1].bytes, offset[32] - offset[16]);
    let somma: u64 = righe.iter().map(|r| r.bytes).sum();
    assert_eq!(somma, *offset.last().unwrap());
    // il raggruppamento non cambia dove sta la riga nell'altezza
    assert!((righe[1].y0 - 8.0 / 72.0).abs() < 1e-6);
    let _ = std::fs::remove_file(p);
}

#[test]
fn in_byte_mode_the_height_is_the_bytes_and_empty_days_vanish() {
    let (mut a, offset, p) = app();
    a.set_timeline(true);
    a.set_timeline_mode(TimelineMode::Bytes);
    pronta(&mut a);
    let len = *offset.last().unwrap() as f32;
    let righe = a.timeline_rows(100);
    assert_eq!(righe.len(), 3);
    for (d, r) in righe.iter().enumerate() {
        assert!(
            (r.y0 - offset[48 * d] as f32 / len).abs() < 1e-5,
            "giorno {d}"
        );
        assert!((r.y1 - offset[48 * (d + 1)] as f32 / len).abs() < 1e-5);
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_jumps_to_the_line_and_the_time_under_the_pointer_is_known() {
    let (mut a, offset, p) = app();
    a.set_timeline(true);
    pronta(&mut a);
    // l'inizio del secondo giorno: un terzo dell'altezza
    assert_eq!(
        a.timeline_time_at(1.0 / 3.0, 100).as_deref(),
        Some("2026-11-02 00:00:00")
    );
    // a meta' dell'altezza: mezzogiorno del secondo giorno (a meno dell'arrotondamento dei decimali)
    let t = a.timeline_time_at(0.5, 100).unwrap();
    assert!(
        t.starts_with("2026-11-02 12:00:00") || t.starts_with("2026-11-02 11:59:59"),
        "{t}"
    );
    a.timeline_jump(1.0 / 3.0, 100);
    assert_eq!(a.cursor(), offset[48]);
    // piu' avanti nello stesso giorno, il salto cade nel mezzo dei suoi byte
    a.timeline_jump(0.5, 100);
    assert!(a.cursor() > offset[48] && a.cursor() < offset[96]);
    // il riquadro segue: dopo il salto la parte che si vede comincia a meta' del file (il secondo giorno e' fra un terzo e due)
    // (la parte che si vede si conosce dopo un disegno)
    mrp_tui::ui::render_lines(&mut a, 100, 24);
    let (da, fine) = a.timeline_viewport(100).unwrap();
    assert!(
        da > 0.4 && da < 0.6 && fine > da && fine <= 0.7,
        "{da} {fine}"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_window_restricts_the_timeline_to_its_range() {
    let (mut a, offset, p) = app();
    a.set_timeline(true);
    pronta(&mut a);
    assert_eq!(a.timeline_rows(100).len(), 3);
    // il filtro per data: solo il secondo giorno
    a.apply_time_range("2026-11-02 00:00:00", "2026-11-02 23:59:59")
        .unwrap();
    pronta(&mut a);
    let righe = a.timeline_rows(100);
    assert_eq!(righe.len(), 1);
    assert_eq!(righe[0].offset, offset[48]);
    assert_eq!(righe[0].bytes, offset[96] - offset[48]);
    let _ = std::fs::remove_file(p);
}

#[test]
fn without_a_time_format_or_with_unsaved_edits_it_says_so() {
    // senza orari
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlapp_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, "uno\ndue\ntre\n").unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.set_timeline(true);
    pronta(&mut a);
    assert_eq!(a.timeline_status(), TimelineStatus::NoTimes);
    assert!(a.timeline_rows(100).is_empty());
    let _ = std::fs::remove_file(&p);
    // un documento modificato non si calcola
    let (mut b, _, q) = app();
    b.handle_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE));
    b.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(b.is_modified());
    b.set_timeline(true);
    pronta(&mut b);
    assert_eq!(b.timeline_status(), TimelineStatus::Unavailable);
    let _ = std::fs::remove_file(q);
}

#[test]
fn a_minute_step_over_three_days_is_under_the_cap() {
    let (mut a, _, p) = app();
    a.set_timeline(true);
    a.set_timeline_step(Step::Minute);
    pronta(&mut a);
    // dalla prima riga (2026-11-01 00:00) all'ultima (2026-11-03 23:30): 4290 minuti, quindi 4291 bucket, sotto il tetto
    assert_eq!(a.timeline_status(), TimelineStatus::Ready);
    assert_eq!(a.timeline_rows(100_000).len(), 4291);
    let _ = std::fs::remove_file(p);
}
