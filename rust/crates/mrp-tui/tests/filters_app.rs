//! La finestra "Filtri" dell'`App` (sezioni Tempo, Livelli e Testo insieme, `chrome = false`): le condizioni si combinano con "e" in una
//! vista filtrata sola, l'intervallo di tempo e' una condizione (la finestra di tempo della scheda non cambia), un testo che non vale
//! lascia tutto com'era. Scritti da ARCH: 144 righe, una ogni mezz'ora dal 2026-11-01 (48 al giorno), la riga `i` ha il livello
//! `i % 4` (ERROR, WARN, INFO, DEBUG); i numeri attesi si contano qui sul testo di partenza, non sul programma.

use mrp_tui::app::{App, AppConfig, DialogKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn riga(i: u32) -> String {
    let livello = ["ERROR", "WARN", "INFO", "DEBUG"][(i % 4) as usize];
    format!(
        "2026-11-{:02} {:02}:{:02}:00 [{livello}] riga {i}\n",
        1 + i / 48,
        (i % 48) / 2,
        (i % 2) * 30
    )
}

fn app() -> (App, PathBuf) {
    let dati: String = (0..144u32).map(riga).collect();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_filters_{}_{}.log", std::process::id(), n));
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

/// Quante righe (fra le `i` date) soddisfano `f`.
fn conta(righe: std::ops::Range<u32>, f: impl Fn(u32, &str) -> bool) -> u64 {
    righe.filter(|&i| f(i, &riga(i))).count() as u64
}

#[test]
fn levels_and_text_make_one_view_with_the_lines_that_satisfy_both() {
    let (mut a, p) = app();
    a.open_time_dialog();
    a.dialog_filters_set_level(0, true);
    a.dialog_filters_set_text("riga 1");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("la vista filtrata");
    // ERROR: le righe 0, 4, 8...; con "riga 1": quelle il cui numero comincia per 1
    let attese = conta(0..144, |i, t| i % 4 == 0 && t.contains("riga 1"));
    assert!(attese > 3 && attese < 36, "l'oracolo ha senso: {attese}");
    assert_eq!(fatto.lines, attese);
    assert_eq!(fatto.pattern, "levels: ERROR + \"riga 1\"");
    // la scheda di partenza non cambia: nessuna finestra di tempo
    assert_eq!(a.time_window(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn hiding_the_text_keeps_the_levels_and_the_rest() {
    let (mut a, p) = app();
    a.open_time_dialog();
    a.dialog_filters_set_level(2, true);
    a.dialog_filters_set_level(3, true);
    a.dialog_filters_set_text("riga 1");
    a.dialog_filters_set_invert(true);
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("INFO e DEBUG senza \"riga 1\"");
    let attese = conta(0..144, |i, t| i % 4 >= 2 && !t.contains("riga 1"));
    assert_eq!(fatto.lines, attese);
    assert_eq!(fatto.pattern, "levels: INFO, DEBUG + not \"riga 1\"");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_text_alone_follows_the_chosen_mode_and_case() {
    let (mut a, p) = app();
    a.open_time_dialog();
    a.dialog_filters_set_text("RIGA 3*");
    a.dialog_filters_set_mode(1);
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    // glob senza distinguere le maiuscole: "riga 3" seguito da qualsiasi cosa nella riga (anche "riga 3" e basta)
    let fatto = a.take_filter_done().expect("il glob");
    assert_eq!(
        fatto.lines,
        conta(0..144, |i, _| i.to_string().starts_with('3'))
    );
    // con le maiuscole distinte nessuna riga ha "RIGA"
    a.open_time_dialog();
    a.dialog_filters_set_case(true);
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    // nessuna riga: non si apre niente e la barra lo dice
    assert!(a.take_filter_done().is_none());
    assert_eq!(a.flash(), Some("no line matches"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_time_range_is_a_condition_and_the_window_of_the_tab_does_not_change() {
    let (mut a, p) = app();
    a.open_time_dialog();
    a.dialog_time_set_check(true);
    a.dialog_set_field(2, "2026-11-02 00:00:00");
    a.dialog_set_field(3, "2026-11-02 23:59:59");
    a.dialog_filters_set_level(2, true);
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("INFO del secondo giorno");
    // il secondo giorno sono le righe 48..96; INFO una su quattro
    assert_eq!(fatto.lines, conta(48..96, |i, _| i % 4 == 2));
    assert_eq!(
        fatto.pattern,
        "levels: INFO + 2026-11-02 00:00:00 \u{2026} 2026-11-02 23:59:59"
    );
    assert_eq!(a.time_window(), None, "il tempo e' solo una condizione");
    // con la casella del filtro per data spenta le date si ignorano: tutte le righe INFO
    a.open_time_dialog();
    a.dialog_time_set_check(false);
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("INFO di tutto il file");
    assert_eq!(fatto.lines, conta(0..144, |i, _| i % 4 == 2));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_bad_regex_stays_in_the_window_and_changes_nothing() {
    let (mut a, p) = app();
    a.apply_time_range("2026-11-01 00:00:00", "2026-11-01 11:59:59")
        .unwrap();
    let finestra = a.time_window();
    assert!(finestra.is_some());
    a.open_time_dialog();
    a.dialog_filters_set_text("(");
    a.dialog_filters_set_mode(2);
    a.dialog_submit();
    // la finestra resta aperta con l'errore, nessun lavoro parte, la finestra di tempo e' quella di prima
    assert_eq!(a.dialog_kind(), Some(DialogKind::Time));
    assert!(!a.time_error_text().is_empty());
    assert!(a.take_filter_done().is_none());
    assert_eq!(a.time_window(), finestra);
    let _ = std::fs::remove_file(p);
}

#[test]
fn without_levels_or_text_the_date_filter_still_narrows_the_view_as_before() {
    let (mut a, p) = app();
    a.open_time_dialog();
    a.dialog_time_set_check(true);
    a.dialog_set_field(2, "2026-11-02 00:00:00");
    a.dialog_set_field(3, "2026-11-02 23:59:59");
    a.dialog_submit();
    assert!(a.time_window().is_some());
    assert!(a.take_filter_done().is_none());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_level_buttons_of_the_timeline_stay_inside_the_time_window() {
    let (mut a, p) = app();
    a.apply_time_range("2026-11-02 00:00:00", "2026-11-02 23:59:59")
        .unwrap();
    a.set_timeline_filter_level(0, true);
    a.timeline_apply_level_filter();
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("ERROR del secondo giorno");
    assert_eq!(fatto.lines, conta(48..96, |i, _| i % 4 == 0));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_summary_says_what_ok_will_apply() {
    let (mut a, p) = app();
    a.open_time_dialog();
    let riassunto = |a: &App| match a.dialog_view() {
        Some(mrp_tui::app::dialogs::DialogView::Time { summary, .. }) => summary,
        _ => panic!("la finestra Filtri"),
    };
    assert_eq!(riassunto(&a), "no filter chosen");
    a.dialog_filters_set_level(1, true);
    a.dialog_filters_set_level(0, true);
    assert_eq!(riassunto(&a), "Levels: ERROR, WARN");
    a.dialog_filters_set_text("timeout");
    assert_eq!(
        riassunto(&a),
        "Levels: ERROR, WARN \u{b7} Text: \"timeout\""
    );
    a.dialog_filters_set_invert(true);
    assert_eq!(
        riassunto(&a),
        "Levels: ERROR, WARN \u{b7} Text: not \"timeout\""
    );
    a.dialog_time_set_check(true);
    a.dialog_set_field(2, "2026-11-02 00:00:00");
    assert_eq!(
        riassunto(&a),
        "Time: 2026-11-02 00:00:00 to \u{2026} \u{b7} Levels: ERROR, WARN \u{b7} Text: not \"timeout\""
    );
    let _ = std::fs::remove_file(p);
}
