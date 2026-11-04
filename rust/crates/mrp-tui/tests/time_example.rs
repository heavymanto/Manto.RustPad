//! "Insegnare con un esempio": da un pezzo di riga che contiene la data che interessa si ricava la regola (formato, marcatore, entro
//! quanti caratteri), provata sul file. Scritti da ARCH: trenta righe `2026-11-03 11:19:SS.350 +01:00 [INF] XC4 funding @ DATA rate=1`,
//! con DATA ogni otto ore dal 2019-09-10 08:00 (a mano: l'ultima, la riga 29, e' il 2019-09-20 00:00).

use mrp_tui::app::{App, AppConfig, ExampleProblem, SourceKind};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Il testo (con `date` in ordine, o come dice `ordine`) e gli offset delle righe.
fn log(ordine: impl Fn(u32) -> u32) -> (String, Vec<u64>) {
    let mut dati = String::new();
    let mut offset = Vec::new();
    for i in 0..30u32 {
        offset.push(dati.len() as u64);
        let ore = 8 + 8 * ordine(i);
        let (giorno, ora) = (10 + ore / 24, ore % 24);
        dati.push_str(&format!(
            "2026-11-03 11:19:{i:02}.350 +01:00 [INF] XC4 funding @ 2019-09-{giorno:02} {ora:02}:00 rate=1\n"
        ));
    }
    offset.push(dati.len() as u64);
    (dati, offset)
}

fn app_con(dati: &str) -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_texample_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(120, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn app() -> (App, Vec<u64>, PathBuf) {
    let (dati, offset) = log(|i| i);
    let (a, p) = app_con(&dati);
    (a, offset, p)
}

#[test]
fn a_piece_of_line_with_the_text_before_the_date_gives_the_rule_and_how_it_goes() {
    let (mut a, offset, p) = app();
    a.open_time_dialog();
    a.dialog_time_set_example("funding @ 2019-09-10 08:00");
    // senza cercare non c'e' niente
    assert!(a.time_example_result().is_none());
    a.dialog_time_find_rule();
    let esito = a.time_example_result().expect("l'esito").clone();
    assert_eq!(esito.problem, None);
    assert_eq!(esito.candidates.len(), 1);
    let c = &esito.candidates[0];
    // il marcatore piu' corto che basta: "@ " (un solo spazio non basta a essere un marcatore)
    assert_eq!(c.pattern, "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!((c.within, c.skip), (200, 0));
    assert_eq!(c.kind, SourceKind::After("@ ".to_string()));
    // come va sul file: 30 righe, dal 10 settembre alle 08:00 al 20 alle 00:00, in ordine
    assert_eq!(c.lines, 30);
    assert_eq!(c.first, "2019-09-10 08:00:00");
    assert_eq!(c.last, "2019-09-20 00:00:00");
    assert!(c.ordered);
    // sceglierla mette il pattern nei campi; con OK vale (e l'11 settembre ha tre righe: 00:00, 08:00 e 16:00)
    assert_eq!(a.time_example_selected(), None);
    a.dialog_time_use_example(0);
    assert_eq!(a.time_example_selected(), Some(0));
    assert_eq!(a.time_field_text(0), "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(a.time_field_text(1), "200");
    a.dialog_set_field(2, "2019-09-11");
    a.dialog_set_field(3, "2019-09-11");
    a.dialog_submit();
    assert_eq!(
        a.time_window(),
        Some((offset[2], offset[5])),
        "{}",
        a.time_error_text()
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_whole_line_has_two_dates_and_gives_one_rule_for_each() {
    let (mut a, _, p) = app();
    a.open_time_dialog();
    a.dialog_time_set_example(
        "2026-11-03 11:19:00.350 +01:00 [INF] XC4 funding @ 2019-09-10 08:00 rate=1",
    );
    a.dialog_time_find_rule();
    let esito = a.time_example_result().unwrap().clone();
    assert_eq!(esito.candidates.len(), 2);
    // la prima data e' all'inizio della riga: l'orario all'inizio della riga, nel suo formato preciso
    assert_eq!(esito.candidates[0].kind, SourceKind::Start);
    assert_eq!(esito.candidates[0].pattern, "yyyy-MM-dd HH:mm:ss.SSS");
    assert_eq!(esito.candidates[0].lines, 30);
    assert_eq!(esito.candidates[0].first, "2026-11-03 11:19:00.350");
    // la seconda: dopo "@ "
    assert_eq!(esito.candidates[1].pattern, "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(esito.candidates[1].lines, 30);
    // scegliere la seconda: quella e' la scelta
    a.dialog_time_use_example(1);
    assert_eq!(a.time_example_selected(), Some(1));
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_example_without_a_date_or_with_a_text_that_is_not_in_the_file_says_so() {
    let (mut a, _, p) = app();
    a.open_time_dialog();
    a.dialog_time_set_example("niente date qui");
    a.dialog_time_find_rule();
    let e = a.time_example_result().unwrap();
    assert_eq!(e.problem, Some(ExampleProblem::NoDate));
    assert!(e.candidates.is_empty());
    // la data c'e' ma quel testo nel file no
    a.dialog_time_set_example("zzz 2030-01-01 00:00");
    a.dialog_time_find_rule();
    let e = a.time_example_result().unwrap();
    assert_eq!(e.problem, Some(ExampleProblem::NotInFile));
    // cambiare l'esempio toglie l'esito di prima
    a.dialog_time_set_example("altro");
    assert!(a.time_example_result().is_none());
    let _ = std::fs::remove_file(p);
}

#[test]
fn dates_that_are_not_in_order_give_the_rule_but_say_so() {
    // le date vanno avanti e indietro: la regola legge le righe ma non e' "in ordine"
    let (dati, _) = log(|i| (i * 7) % 30);
    let (mut a, p) = app_con(&dati);
    a.open_time_dialog();
    a.dialog_time_set_example("@ 2019-09-10 08:00");
    a.dialog_time_find_rule();
    let e = a.time_example_result().unwrap().clone();
    assert_eq!(e.candidates.len(), 1);
    assert_eq!(e.candidates[0].lines, 30);
    assert!(!e.candidates[0].ordered, "{:?}", e.candidates[0]);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_line_selected_in_the_document_is_the_example_when_the_window_opens() {
    let (mut a, _, p) = app();
    // la prima riga selezionata: Home e poi Maiusc+Fine
    a.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    a.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::SHIFT));
    assert!(a.selection().is_some());
    a.open_time_dialog();
    assert_eq!(
        a.time_example(),
        "2026-11-03 11:19:00.350 +01:00 [INF] XC4 funding @ 2019-09-10 08:00 rate=1"
    );
    // e le regole sono gia' ricavate (senza premere il pulsante)
    let e = a.time_example_result().expect("ricavate da sole");
    assert_eq!(e.candidates.len(), 2);
    // senza selezione l'esempio parte vuoto
    a.dialog_cancel();
    a.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let _ = std::fs::remove_file(p);
}
