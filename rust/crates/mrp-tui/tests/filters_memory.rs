//! La finestra Filtri si ricorda delle condizioni e la barra di stato dice quali sono attive. Scritti da ARCH sul file di
//! `filters_app.rs`: 144 righe, una ogni mezz'ora dal 2026-11-01, la riga `i` ha il livello `i % 4` (ERROR, WARN, INFO, DEBUG) e il
//! testo `riga i`; i numeri attesi si contano a mano qui, non sul programma.
//! - livelli ERROR e WARN + regex `riga 1[2-5]\b`: le righe 12 (ERROR) e 13 (WARN); la 14 (INFO) e la 15 (DEBUG) non hanno il livello.
//! - ERROR e WARN su tutto il file: le righe con `i % 4` 0 o 1, cioe' 72.
//!
//! Una vista filtrata rifatta con altre condizioni parte sempre dal file ORIGINALE, e prende il posto della vista di prima.

use mrp_tui::app::dialogs::DialogView;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::workspace::Workspace;
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

fn ws() -> (Workspace, PathBuf) {
    let dati: String = (0..144u32).map(riga).collect();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_fmem_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut w = Workspace::new(app, cfg);
    w.resize(100, 24);
    assert!(w.wait_idle(Duration::from_secs(20)));
    (w, p)
}

/// Aspetta che il filtro finisca e che la scheda nuova si apra (e che il filtro rifatto dall'originale parta e finisca).
fn aspetta(w: &mut Workspace) {
    for _ in 0..4 {
        w.tick();
        assert!(w.wait_idle(Duration::from_secs(30)));
    }
}

/// Cosa mostra la finestra Filtri della scheda attiva: livelli, testo, modo, maiuscole, nascondi, casella del tempo, dal e al.
#[allow(clippy::type_complexity)]
fn finestra(w: &Workspace) -> ([bool; 4], String, usize, bool, bool, bool, String, String) {
    match w.active().dialog_view() {
        Some(DialogView::Time {
            levels,
            ftext,
            fmode,
            fcase,
            finvert,
            check,
            from,
            to,
            ..
        }) => (
            levels,
            ftext,
            usize::from(fmode),
            fcase,
            finvert,
            check,
            from,
            to,
        ),
        _ => panic!("la finestra Filtri"),
    }
}

/// ERROR e WARN + regex `riga 1[2-5]\b`, confermato dalla scheda attiva.
fn filtra_errori_e_warn_con_regex(w: &mut Workspace) {
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_level(0, true);
    a.dialog_filters_set_level(1, true);
    a.dialog_filters_set_text(r"riga 1[2-5]\b");
    a.dialog_filters_set_mode(2);
    a.dialog_filters_set_case(true);
    a.dialog_submit();
    aspetta(w);
}

#[test]
fn the_view_and_the_original_show_the_same_conditions_when_the_window_opens_again() {
    let (mut w, p) = ws();
    filtra_errori_e_warn_con_regex(&mut w);
    assert_eq!(w.len(), 2);
    assert!(w.active().is_filter_view());
    let testo = String::from_utf8(w.active().document_text()).unwrap();
    assert_eq!(testo, format!("{}{}", riga(12), riga(13)));
    // nella vista: livelli, testo, modo e maiuscole sono quelli che l'hanno fatta
    w.active_mut().open_time_dialog();
    let attese = (
        [true, true, false, false],
        r"riga 1[2-5]\b".to_string(),
        2,
        true,
        false,
        false,
        String::new(),
        String::new(),
    );
    assert_eq!(finestra(&w), attese);
    w.active_mut().dialog_cancel();
    // nel file originale: le stesse
    w.select(0);
    w.active_mut().open_time_dialog();
    assert_eq!(finestra(&w), attese);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_date_filter_comes_back_with_its_dates() {
    let (mut w, p) = ws();
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_time_set_check(true);
    a.dialog_set_field(2, "2026-11-02 00:00:00");
    a.dialog_set_field(3, "2026-11-02 23:59:59");
    a.dialog_filters_set_level(2, true);
    a.dialog_submit();
    aspetta(&mut w);
    assert!(w.active().is_filter_view());
    w.active_mut().open_time_dialog();
    let (levels, _, _, _, _, check, from, to) = finestra(&w);
    assert_eq!(levels, [false, false, true, false]);
    assert!(check, "la casella del filtro per data torna accesa");
    assert_eq!(
        (from.as_str(), to.as_str()),
        ("2026-11-02 00:00:00", "2026-11-02 23:59:59")
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_status_lists_the_filters_that_are_on() {
    let (mut w, p) = ws();
    assert_eq!(w.active_mut().status().filters, None);
    filtra_errori_e_warn_con_regex(&mut w);
    // la vista dice le condizioni che l'hanno fatta, il modo compreso
    assert_eq!(
        w.active_mut().status().filters.as_deref(),
        Some(r#"Levels: ERROR, WARN · Text: "riga 1[2-5]\b" (regex)"#)
    );
    // il file originale mostra tutto: niente filtri
    w.select(0);
    assert_eq!(w.active_mut().status().filters, None);
    // un filtro per data sul file (senza livelli ne' testo) e' un filtro attivo della scheda
    w.active_mut()
        .apply_time_range("2026-11-01 00:00:00", "2026-11-01 11:59:59")
        .unwrap();
    assert_eq!(
        w.active_mut().status().filters.as_deref(),
        Some("Time: 2026-11-01 00:00:00 to 2026-11-01 11:59:59")
    );
    w.active_mut().clear_time_window();
    assert_eq!(w.active_mut().status().filters, None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn new_conditions_chosen_in_a_view_filter_the_original_and_replace_the_view() {
    let (mut w, p) = ws();
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_level(0, true);
    a.dialog_submit();
    aspetta(&mut w);
    assert_eq!(w.len(), 2);
    let errori: String = (0..144u32).filter(|i| i % 4 == 0).map(riga).collect();
    assert_eq!(
        String::from_utf8(w.active().document_text()).unwrap(),
        errori
    );
    let vecchio = w.active().filter_scratch().unwrap().to_path_buf();
    assert!(vecchio.exists());
    // nella vista si aggiunge WARN: il risultato e' ERROR + WARN dell'originale (72 righe), non ERROR della vista
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_level(1, true);
    a.dialog_submit();
    aspetta(&mut w);
    assert_eq!(w.len(), 2, "la vista nuova prende il posto della vecchia");
    assert!(w.active().is_filter_view());
    let attese: String = (0..144u32).filter(|i| i % 4 < 2).map(riga).collect();
    assert_eq!(
        String::from_utf8(w.active().document_text()).unwrap(),
        attese
    );
    assert!(
        !vecchio.exists(),
        "il file di appoggio della vista di prima se ne va"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn taking_every_condition_off_in_a_view_goes_back_to_the_whole_file() {
    let (mut w, p) = ws();
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_level(0, true);
    a.dialog_submit();
    aspetta(&mut w);
    assert_eq!(w.len(), 2);
    let vecchio = w.active().filter_scratch().unwrap().to_path_buf();
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_level(0, false);
    a.dialog_submit();
    aspetta(&mut w);
    assert_eq!(w.len(), 1, "senza condizioni la vista non serve piu'");
    assert!(!w.active().is_filter_view());
    assert!(!vecchio.exists());
    assert_eq!(w.active_mut().status().filters, None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_bad_regex_in_a_view_stays_in_its_window_and_changes_nothing() {
    let (mut w, p) = ws();
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_level(0, true);
    a.dialog_submit();
    aspetta(&mut w);
    let vecchio = w.active().filter_scratch().unwrap().to_path_buf();
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_text("(");
    a.dialog_filters_set_mode(2);
    a.dialog_submit();
    aspetta(&mut w);
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::Time));
    assert!(!w.active().time_error_text().is_empty());
    assert_eq!(w.len(), 2);
    assert!(vecchio.exists());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_view_is_replaced_only_when_the_new_filter_finds_lines() {
    let (mut w, p) = ws();
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_level(0, true);
    a.dialog_submit();
    aspetta(&mut w);
    let vecchio = w.active().filter_scratch().unwrap().to_path_buf();
    // nessuna riga ha questo testo: la vista di prima resta com'e'
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_text("non c'e' da nessuna parte");
    a.dialog_submit();
    aspetta(&mut w);
    assert_eq!(w.len(), 2);
    assert!(vecchio.exists());
    assert_eq!(w.tabs()[1].filter_scratch(), Some(vecchio.as_path()));
    // e un filtro qualsiasi fatto dopo, dal file originale, non porta via la vista di prima
    w.select(0);
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_text("");
    a.dialog_filters_set_level(0, false);
    a.dialog_filters_set_level(3, true);
    a.dialog_submit();
    aspetta(&mut w);
    assert_eq!(w.len(), 3);
    assert!(vecchio.exists());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_level_buttons_of_the_timeline_in_a_view_also_filter_the_original() {
    let (mut w, p) = ws();
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_filters_set_level(0, true);
    a.dialog_submit();
    aspetta(&mut w);
    let vecchio = w.active().filter_scratch().unwrap().to_path_buf();
    // il pulsante WARN della timeline, dentro la vista ERROR: ERROR + WARN del file intero
    w.active_mut().set_timeline_filter_level(1, true);
    w.active_mut().timeline_apply_level_filter();
    aspetta(&mut w);
    assert_eq!(w.len(), 2);
    let attese: String = (0..144u32).filter(|i| i % 4 < 2).map(riga).collect();
    assert_eq!(
        String::from_utf8(w.active().document_text()).unwrap(),
        attese
    );
    assert!(!vecchio.exists());
    assert_eq!(
        w.active_mut().status().filters.as_deref(),
        Some("Levels: ERROR, WARN")
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_date_filter_switched_off_by_hand_comes_back_unchecked_with_its_dates() {
    let (mut w, p) = ws();
    let a = w.active_mut();
    a.open_time_dialog();
    a.dialog_time_set_check(true);
    a.dialog_set_field(2, "2026-11-02 00:00:00");
    a.dialog_set_field(3, "2026-11-02 23:59:59");
    a.dialog_submit();
    assert!(w.active().time_filter_on());
    w.active_mut().toggle_time_filter();
    assert!(!w.active().time_filter_on());
    w.active_mut().open_time_dialog();
    let (_, _, _, _, _, check, from, to) = finestra(&w);
    assert!(!check, "spento a mano: la casella e' spenta");
    assert_eq!(
        (from.as_str(), to.as_str()),
        ("2026-11-02 00:00:00", "2026-11-02 23:59:59")
    );
    let _ = std::fs::remove_file(p);
}
