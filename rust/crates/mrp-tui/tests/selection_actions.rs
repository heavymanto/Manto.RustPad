//! Le azioni del menu del tasto destro che partono dal testo selezionato (filtra, escludi, cerca). Scritti da ARCH sul file di
//! `filters_memory.rs`: 144 righe, la riga `i` ha il livello `i % 4` (ERROR, WARN, INFO, DEBUG) e il testo `riga i`; il nome del
//! livello comincia al byte 21 della riga (`2026-11-01 00:00:00 [ERROR] riga 0\n`), e le prime righe (una cifra) sono lunghe 35 byte
//! (ERROR, DEBUG) o 34 (WARN, INFO): un giro dei quattro livelli e' di 138 byte. I numeri attesi si contano a mano: 36 righe per
//! livello, 108 senza uno.

use mrp_core::pattern::Mode;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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

fn cfg() -> AppConfig {
    AppConfig {
        chrome: false,
        ..AppConfig::default()
    }
}

fn scrivi(dati: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_selact_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    p
}

fn ws_su(dati: &str) -> (Workspace, PathBuf) {
    let p = scrivi(dati);
    let app = App::open(&p, cfg()).unwrap();
    let mut w = Workspace::new(app, cfg());
    w.resize(100, 24);
    assert!(w.wait_idle(Duration::from_secs(20)));
    (w, p)
}

fn ws() -> (Workspace, PathBuf) {
    ws_su(&(0..144u32).map(riga).collect::<String>())
}

fn tasto(c: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(c, m)
}

/// Seleziona `quanti` caratteri a partire dalla colonna `colonna` della riga `riga` (contando dalla prima riga visibile): il cursore
/// parte dall'inizio, scende e va a destra, poi la selezione cresce con Maiusc+Destra.
fn seleziona(a: &mut App, riga: usize, colonna: usize, quanti: usize) {
    a.handle_key(tasto(KeyCode::Home, KeyModifiers::CONTROL));
    for _ in 0..riga {
        a.handle_key(tasto(KeyCode::Down, KeyModifiers::NONE));
    }
    for _ in 0..colonna {
        a.handle_key(tasto(KeyCode::Right, KeyModifiers::NONE));
    }
    for _ in 0..quanti {
        a.handle_key(tasto(KeyCode::Right, KeyModifiers::SHIFT));
    }
}

/// Aspetta che il filtro finisca e che la scheda nuova si apra (e che un filtro rifatto dall'originale parta e finisca).
fn aspetta(w: &mut Workspace) {
    for _ in 0..4 {
        w.tick();
        assert!(w.wait_idle(Duration::from_secs(30)));
    }
}

fn testo(a: &App) -> String {
    String::from_utf8(a.document_text()).unwrap()
}

/// Le righe `i` per cui `tiene(i)`, una dopo l'altra.
fn righe_con(tiene: impl Fn(u32) -> bool) -> String {
    (0..144u32).filter(|&i| tiene(i)).map(riga).collect()
}

#[test]
fn only_a_selection_that_makes_a_filter_gives_a_text() {
    let (mut w, p) = ws();
    let a = w.active_mut();
    assert_eq!(a.selection_as_text(), None, "senza selezione");
    seleziona(a, 0, 21, 5);
    assert_eq!(a.selection(), Some((21, 26)));
    assert_eq!(a.selection_as_text().as_deref(), Some("ERROR"));
    // spazi e basta: niente
    seleziona(a, 0, 19, 1);
    assert_eq!(a.selection_as_text(), None, "solo uno spazio");
    // a capo dentro: niente (14 caratteri dalla colonna 29 arrivano alla riga dopo)
    seleziona(a, 0, 29, 10);
    assert_eq!(a.selection_as_text(), None, "attraversa la fine della riga");
    // una selezione da una riga e mezza e' un blocco da copiare, non una parola
    seleziona(a, 0, 0, 40);
    assert_eq!(a.selection_as_text(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_selection_longer_than_the_limit_is_not_a_filter_text() {
    // una riga sola di 300 lettere: oltre i 256 byte ammessi; un pezzo di 256 invece va bene
    let (mut w, p) = ws_su(&format!("{}\n", "a".repeat(300)));
    let a = w.active_mut();
    seleziona(a, 0, 0, 256);
    assert_eq!(a.selection_as_text(), Some("a".repeat(256)));
    seleziona(a, 0, 0, 257);
    assert_eq!(a.selection_as_text(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn filter_lines_with_the_selected_text_opens_a_view_with_only_those_lines() {
    let (mut w, p) = ws();
    seleziona(w.active_mut(), 0, 21, 5);
    w.active_mut().filter_by_selection(false);
    aspetta(&mut w);
    assert_eq!(w.len(), 2);
    assert!(w.active().is_filter_view());
    assert_eq!(testo(w.active()), righe_con(|i| i % 4 == 0));
    // la finestra Filtri ricorda il testo, preso alla lettera
    w.active_mut().open_time_dialog();
    assert_eq!(w.active().filters_text(), "ERROR");
    let _ = std::fs::remove_file(p);
}

#[test]
fn hide_lines_with_the_selected_text_opens_a_view_without_them() {
    let (mut w, p) = ws();
    seleziona(w.active_mut(), 0, 21, 5);
    w.active_mut().filter_by_selection(true);
    aspetta(&mut w);
    assert_eq!(w.len(), 2);
    assert_eq!(testo(w.active()), righe_con(|i| i % 4 != 0));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_selected_text_counts_with_its_capitals() {
    let (mut w, p) = ws_su("Timeout uno\ntimeout due\nTimeout tre\nTIMEOUT quattro\n");
    seleziona(w.active_mut(), 1, 0, 7);
    assert_eq!(w.active().selection_as_text().as_deref(), Some("timeout"));
    w.active_mut().filter_by_selection(false);
    aspetta(&mut w);
    assert_eq!(testo(w.active()), "timeout due\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_selected_text_is_a_plain_text_even_if_it_looks_like_a_pattern() {
    let (mut w, p) = ws_su("x a.c y\nabc\na.c\nxx\n");
    // la finestra era rimasta su "espressione regolare": la selezione resta testo semplice
    w.active_mut().set_search_options(Mode::Regex, false);
    seleziona(w.active_mut(), 0, 2, 3);
    assert_eq!(w.active().selection_as_text().as_deref(), Some("a.c"));
    w.active_mut().filter_by_selection(false);
    aspetta(&mut w);
    assert_eq!(testo(w.active()), "x a.c y\na.c\n", "abc non c'entra");
    let _ = std::fs::remove_file(p);
}

#[test]
fn in_a_filtered_view_the_levels_stay_and_the_new_view_takes_the_place_of_the_old_one() {
    let (mut w, p) = ws();
    // la vista ERROR + WARN (72 righe)
    {
        let a = w.active_mut();
        a.open_time_dialog();
        a.dialog_filters_set_level(0, true);
        a.dialog_filters_set_level(1, true);
        a.dialog_submit();
    }
    aspetta(&mut w);
    assert_eq!(w.len(), 2);
    assert_eq!(testo(w.active()), righe_con(|i| i % 4 <= 1));
    // nella vista, la seconda riga e' la 1 (WARN): "nascondi WARN" lascia gli ERROR, e la vista nuova prende il posto della vecchia
    seleziona(w.active_mut(), 1, 21, 4);
    assert_eq!(w.active().selection_as_text().as_deref(), Some("WARN"));
    w.active_mut().filter_by_selection(true);
    aspetta(&mut w);
    assert_eq!(w.len(), 2, "la vista di prima e' stata sostituita");
    assert_eq!(testo(w.active()), righe_con(|i| i % 4 == 0));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_filters_window_filters_a_file_with_no_dates_by_text_without_inventing_a_time_rule() {
    // un file senza orario riconosciuto non ha un formato: il filtro per testo funziona lo stesso (prima l'OK dava un errore
    // "formato incompleto" e la finestra restava aperta)
    let (mut w, p) = ws_su("uno\ndue\nuno e due\ntre\n");
    {
        let a = w.active_mut();
        a.open_time_dialog();
        a.dialog_filters_set_text("uno");
        a.dialog_submit();
    }
    aspetta(&mut w);
    assert_eq!(w.len(), 2);
    assert_eq!(testo(w.active()), "uno\nuno e due\n");
    // nessuna regola di tempo vuota nelle impostazioni
    assert!(w.active().time_rules().is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn in_a_view_made_with_a_pattern_the_selected_text_is_still_plain_text() {
    let (mut w, p) = ws_su("x a.c y\nabc\na.c\nxx\n");
    // la vista fatta con l'espressione regolare `a.` (maiuscole contano): tre righe, "xx" no
    {
        let a = w.active_mut();
        a.open_time_dialog();
        a.dialog_filters_set_text("a.");
        a.dialog_filters_set_mode(2);
        a.dialog_filters_set_case(true);
        a.dialog_submit();
    }
    aspetta(&mut w);
    assert_eq!(testo(w.active()), "x a.c y\nabc\na.c\n");
    // nella vista si seleziona "a.c" e si filtra: il testo e' semplice (il punto e' un punto), non la regex di prima
    seleziona(w.active_mut(), 0, 2, 3);
    assert_eq!(w.active().selection_as_text().as_deref(), Some("a.c"));
    w.active_mut().filter_by_selection(false);
    aspetta(&mut w);
    assert_eq!(w.len(), 2);
    assert_eq!(testo(w.active()), "x a.c y\na.c\n", "abc non c'entra");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_date_filter_that_is_on_stays_with_the_selected_text() {
    let (mut w, p) = ws();
    // il secondo giorno: le righe 48..95; gli ERROR sono quelle con `i % 4 == 0`, cioe' 12
    w.active_mut()
        .apply_time_range("2026-11-02 00:00:00", "2026-11-02 23:59:59")
        .unwrap();
    seleziona(w.active_mut(), 0, 21, 5);
    assert_eq!(w.active().selection_as_text().as_deref(), Some("ERROR"));
    w.active_mut().filter_by_selection(false);
    aspetta(&mut w);
    assert!(w.active().is_filter_view());
    assert_eq!(
        testo(w.active()),
        righe_con(|i| (48..96).contains(&i) && i % 4 == 0)
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn nothing_happens_without_a_useful_selection() {
    let (mut w, p) = ws();
    w.active_mut().filter_by_selection(false);
    w.active_mut().filter_by_selection(true);
    w.active_mut().find_selection();
    aspetta(&mut w);
    assert_eq!(w.len(), 1);
    assert_eq!(w.active().cursor(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn find_the_selected_text_goes_to_the_next_one_after_the_selection() {
    let (mut w, p) = ws();
    seleziona(w.active_mut(), 0, 21, 5);
    w.active_mut().find_selection();
    aspetta(&mut w);
    // il prossimo ERROR e' nella riga 4, che comincia dopo un giro dei livelli (35 + 34 + 34 + 35 = 138 byte): 21 di colonna
    assert_eq!(w.active().cursor(), 138 + 21);
    // F3 prosegue con lo stesso testo: la riga 8, dopo due giri
    w.active_mut()
        .handle_key(tasto(KeyCode::F(3), KeyModifiers::NONE));
    aspetta(&mut w);
    assert_eq!(w.active().cursor(), 2 * 138 + 21);
    let _ = std::fs::remove_file(p);
}

#[test]
fn find_the_selected_text_reads_it_as_plain_text_with_capitals() {
    let (mut w, p) = ws_su("x a.c y\nabc\nA.C\na.c\n");
    w.active_mut().set_search_options(Mode::Regex, false);
    seleziona(w.active_mut(), 0, 2, 3);
    w.active_mut().find_selection();
    aspetta(&mut w);
    // "abc" (regex) e "A.C" (maiuscole) non contano: la prossima e' l'ultima riga, al byte 8 + 4 + 4
    assert_eq!(w.active().cursor(), 16);
    assert_eq!(w.active().search_options(), (Mode::Text, true));
    let _ = std::fs::remove_file(p);
}
