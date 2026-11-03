//! I dialoghi della navigazione temporale (F1): Alt+T apre l'intervallo di tempo (da / a), Alt+O il formato dell'orario. Si
//! guidano con le stesse azioni dei tasti. Scritti da ARCH.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::dialogs::DialogView;
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Cento righe, una al minuto dalle 02:00:00 del 2026-11-01, e gli offset di ognuna.
fn log() -> (Vec<u8>, Vec<u64>) {
    let mut dati = String::new();
    let mut offset = Vec::new();
    for i in 0..100u32 {
        offset.push(dati.len() as u64);
        dati.push_str(&format!(
            "2026-11-01 {:02}:{:02}:00 [INFO ] riga {i}\n",
            2 + i / 60,
            i % 60
        ));
    }
    offset.push(dati.len() as u64);
    (dati.into_bytes(), offset)
}

fn app() -> (App, Vec<u64>, PathBuf) {
    let (dati, offset) = log();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tdlg_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(100, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, offset, p)
}

fn alt(a: &mut App, c: char) {
    a.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT));
}

fn tasto(a: &mut App, c: KeyCode) {
    a.handle_key(KeyEvent::new(c, KeyModifiers::NONE));
}

#[test]
fn alt_t_opens_the_range_dialog_starting_from_the_time_of_the_top_row() {
    let (mut a, _, p) = app();
    alt(&mut a, 't');
    assert_eq!(a.dialog_kind(), Some(DialogKind::TimeRange));
    let Some(DialogView::TimeRange {
        from,
        to,
        clear,
        field,
        error,
        ..
    }) = a.dialog_view()
    else {
        panic!("manca la vista");
    };
    assert_eq!(from, "2026-11-01 02:00:00");
    assert_eq!(to, "");
    assert_eq!(clear, None, "senza finestra non c'e' niente da togliere");
    assert_eq!((field, error.as_str()), (0, ""));
    let _ = std::fs::remove_file(p);
}

#[test]
fn typing_the_two_times_and_enter_applies_the_range() {
    let (mut a, offset, p) = app();
    alt(&mut a, 't');
    a.dialog_set_field(0, "02:10");
    a.dialog_set_field(1, "02:20");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.time_window(), Some((offset[10], offset[20])));
    // riaperto, il dialogo parte dalla finestra e offre di toglierla
    alt(&mut a, 't');
    let Some(DialogView::TimeRange {
        from, to, clear, ..
    }) = a.dialog_view()
    else {
        panic!("manca la vista");
    };
    assert_eq!(
        (from.as_str(), to.as_str()),
        ("2026-11-01 02:10:00", "2026-11-01 02:20:00")
    );
    assert!(clear.is_some());
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_bad_time_keeps_the_dialog_open_with_the_reason() {
    let (mut a, offset, p) = app();
    alt(&mut a, 't');
    a.dialog_set_field(0, "foo");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), Some(DialogKind::TimeRange), "resta aperto");
    assert!(
        a.time_range_error().contains("foo"),
        "{}",
        a.time_range_error()
    );
    assert_eq!(a.time_window(), None);
    // il testo dell'errore sparisce appena si scrive di nuovo; poi si corregge e va
    a.dialog_set_field(0, "02:10");
    assert_eq!(a.time_range_error(), "");
    a.dialog_set_field(1, "02:20");
    a.dialog_submit();
    assert_eq!(a.time_window(), Some((offset[10], offset[20])));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_keys_type_in_the_focused_field_tab_switches_and_alt_r_removes_the_range() {
    let (mut a, offset, p) = app();
    alt(&mut a, 't');
    // il campo "da" parte pieno: si svuota e si scrive con i tasti
    for _ in 0..30 {
        tasto(&mut a, KeyCode::Backspace);
    }
    for c in "02:30".chars() {
        tasto(&mut a, KeyCode::Char(c));
    }
    tasto(&mut a, KeyCode::Tab);
    assert_eq!(a.time_range_field(), 1);
    for c in "02:35".chars() {
        tasto(&mut a, KeyCode::Char(c));
    }
    assert_eq!(
        (a.time_range_text(0), a.time_range_text(1)),
        ("02:30", "02:35")
    );
    tasto(&mut a, KeyCode::Enter);
    assert_eq!(a.time_window(), Some((offset[30], offset[35])));
    // Alt+R nel dialogo toglie l'intervallo e chiude
    alt(&mut a, 't');
    alt(&mut a, 'r');
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.time_window(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn escape_changes_nothing_and_the_dialog_is_drawn() {
    let (mut a, offset, p) = app();
    a.apply_time_range("02:10", "02:20").unwrap();
    alt(&mut a, 't');
    a.dialog_set_field(0, "03:00");
    let s = render_lines(&mut a, 100, 24).join("\n");
    assert!(
        s.contains("TIME RANGE") && s.contains("From") && s.contains("03:00"),
        "{s}"
    );
    tasto(&mut a, KeyCode::Esc);
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.time_window(), Some((offset[10], offset[20])));
    let _ = std::fs::remove_file(p);
}

#[test]
fn alt_o_opens_the_format_dialog_with_the_detected_mask_and_a_preview() {
    let (mut a, _, p) = app();
    alt(&mut a, 'o');
    assert_eq!(a.dialog_kind(), Some(DialogKind::TimeMask));
    let Some(DialogView::TimeMask {
        pattern,
        within,
        preview,
        error,
        ..
    }) = a.dialog_view()
    else {
        panic!("manca la vista");
    };
    assert_eq!(
        (pattern.as_str(), within.as_str(), error.as_str()),
        ("yyyy-MM-dd HH:mm:ss", "0", "")
    );
    // le prime tre righe, accorciate, con l'orario che il pattern ne legge
    assert_eq!(preview.len(), 3);
    assert_eq!(preview[0].1, "2026-11-01 02:00:00");
    assert_eq!(preview[2].1, "2026-11-01 02:02:00");
    assert!(preview[0]
        .0
        .starts_with("2026-11-01 02:00:00 [INFO ] riga 0"));
    // con un pattern che non legge niente l'anteprima lo dice
    a.dialog_set_field(0, "dd/MM/yyyy HH:mm:ss");
    let nessuno = a.time_mask_preview();
    assert!(
        nessuno.iter().all(|(_, letto)| letto == "(no time)"),
        "{nessuno:?}"
    );
    let s = render_lines(&mut a, 100, 24).join("\n");
    assert!(s.contains("TIME FORMAT") && s.contains("(no time)"), "{s}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_new_mask_is_applied_with_enter_and_a_bad_one_is_refused() {
    let (mut a, _, p) = app();
    alt(&mut a, 'o');
    a.dialog_set_field(0, "yyyyy");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), Some(DialogKind::TimeMask), "resta aperto");
    assert!(
        a.time_mask_error().contains("yyyyy"),
        "{}",
        a.time_mask_error()
    );
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss")
    );
    a.dialog_set_field(0, "yyyy-MM-dd HH:mm");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.time_mask_pattern().as_deref(), Some("yyyy-MM-dd HH:mm"));
    // Alt+A (nel dialogo) la fa riconoscere di nuovo
    alt(&mut a, 'o');
    alt(&mut a, 'a');
    assert_eq!(a.time_mask_text(0), "yyyy-MM-dd HH:mm:ss");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_second_field_of_the_format_takes_only_digits() {
    let (mut a, _, p) = app();
    alt(&mut a, 'o');
    tasto(&mut a, KeyCode::Tab);
    for _ in 0..3 {
        tasto(&mut a, KeyCode::Backspace);
    }
    for c in "4x0z9".chars() {
        tasto(&mut a, KeyCode::Char(c));
    }
    assert_eq!(a.time_mask_text(1), "409");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_status_carries_the_time_of_the_top_row() {
    let (mut a, offset, p) = app();
    assert_eq!(a.status().time.as_deref(), Some("2026-11-01 02:00:00"));
    a.apply_time_range("02:10", "02:20").unwrap();
    assert_eq!(a.status().time.as_deref(), Some("2026-11-01 02:10:00"));
    // dentro la finestra la percentuale e' della finestra: a meta' e' circa 50
    a.goto_offset(offset[15]);
    let p15 = a.status().percent;
    assert!((45..=60).contains(&p15), "{p15}");
    let _ = std::fs::remove_file(p);
}
