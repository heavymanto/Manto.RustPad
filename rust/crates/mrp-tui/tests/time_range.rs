//! La navigazione temporale in una scheda (F1): l'`App` riconosce la maschera dell'orario del suo file, dice l'orario della prima
//! riga visibile e puo' RISTRINGERE la vista a un intervallo di tempo (da / a): la finestra di tempo. Dentro la finestra tutto si
//! comporta come se il file fosse solo quell'intervallo: scorrimento, tasti di inizio e fine, scrollbar, selezione, ricerca. Scritti da
//! ARCH: gli offset delle righe si calcolano a mano sul testo di partenza.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Cento righe, una al minuto dalle 02:00:00 del 2026-11-01: `2026-11-01 HH:MM:00 [INFO ] riga N`. Gli offset di ogni riga.
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

fn app_con(dati: &[u8], righe: u16) -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_trange_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(80, righe);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn tasto(a: &mut App, c: KeyCode, m: KeyModifiers) {
    a.handle_key(KeyEvent::new(c, m));
}

fn schermo(a: &mut App, righe: u16) -> String {
    render_lines(a, 80, righe).join("\n")
}

#[test]
fn the_mask_is_detected_and_the_time_of_the_top_row_is_known() {
    let (dati, offset) = log();
    let (mut a, p) = app_con(&dati, 24);
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss")
    );
    assert_eq!(a.time_at_top().as_deref(), Some("2026-11-01 02:00:00"));
    // scorrendo, l'orario della prima riga segue
    a.scroll_to_fraction(0.5);
    let atteso = {
        // la riga che comincia alla cima della vista
        let i = offset.iter().rposition(|&o| o <= a.top()).unwrap();
        format!("2026-11-01 {:02}:{:02}:00", 2 + i / 60, i % 60)
    };
    assert_eq!(a.time_at_top(), Some(atteso));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_file_without_times_has_no_mask_and_refuses_a_range() {
    let (mut a, p) = app_con(b"niente\norari\nqui\n", 24);
    assert_eq!(a.time_mask_pattern(), None);
    assert_eq!(a.time_at_top(), None);
    assert!(a.apply_time_range("02:10", "02:20").is_err());
    assert_eq!(a.time_window(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_range_restricts_the_view_to_the_lines_from_the_first_time_up_to_the_second_excluded() {
    let (dati, offset) = log();
    let (mut a, p) = app_con(&dati, 24);
    a.apply_time_range("2026-11-01 02:10", "2026-11-01 02:20")
        .unwrap();
    // dalla riga delle 02:10 (la 10) fino alla riga prima di quella delle 02:20 (la 19)
    assert_eq!(a.time_window(), Some((offset[10], offset[20])));
    assert_eq!(a.top(), offset[10]);
    assert_eq!(a.cursor(), offset[10]);
    let s = schermo(&mut a, 24);
    assert!(s.contains("02:10:00") && s.contains("02:19:00"), "{s}");
    assert!(!s.contains("02:09:00") && !s.contains("02:20:00"), "{s}");
    assert_eq!(a.time_at_top().as_deref(), Some("2026-11-01 02:10:00"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_open_end_and_an_open_start() {
    let (dati, offset) = log();
    let (mut a, p) = app_con(&dati, 24);
    // solo "da": fino alla fine del file
    a.apply_time_range("2026-11-01 03:30", "").unwrap();
    assert_eq!(a.time_window(), Some((offset[90], offset[100])));
    // solo "a": dall'inizio
    a.apply_time_range("", "2026-11-01 02:05").unwrap();
    assert_eq!(a.time_window(), Some((offset[0], offset[5])));
    // un orario senza data prende la data del file
    a.apply_time_range("02:10", "02:12").unwrap();
    assert_eq!(a.time_window(), Some((offset[10], offset[12])));
    let _ = std::fs::remove_file(p);
}

#[test]
fn bad_ranges_are_refused_and_change_nothing() {
    let (dati, offset) = log();
    let (mut a, p) = app_con(&dati, 24);
    a.apply_time_range("02:10", "02:20").unwrap();
    for (da, a_) in [
        ("02:30", "02:20"), // da dopo a
        ("02:30", "02:30"), // vuoto
        ("foo", "02:20"),   // non e' un orario
        ("02:10", "boh"),
        ("2030-01-01 00:00", ""), // dopo l'ultima riga: nessuna riga
    ] {
        assert!(a.apply_time_range(da, a_).is_err(), "{da:?} {a_:?}");
        // la finestra di prima resta
        assert_eq!(
            a.time_window(),
            Some((offset[10], offset[20])),
            "{da:?} {a_:?}"
        );
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn inside_the_window_the_keys_stop_at_its_edges() {
    let (dati, offset) = log();
    let (mut a, p) = app_con(&dati, 24);
    a.apply_time_range("02:10", "02:20").unwrap();
    // la fine: l'ultima riga della finestra (in lettura il cursore non va sull'a-capo finale)
    tasto(&mut a, KeyCode::End, KeyModifiers::CONTROL);
    assert_eq!(a.cursor(), offset[20] - 1);
    // l'inizio
    tasto(&mut a, KeyCode::Home, KeyModifiers::CONTROL);
    assert_eq!(a.cursor(), offset[10]);
    // Su all'inizio non esce dalla finestra
    for _ in 0..5 {
        tasto(&mut a, KeyCode::Up, KeyModifiers::NONE);
    }
    assert_eq!(a.cursor(), offset[10]);
    assert_eq!(a.top(), offset[10]);
    // Giu' tante volte: si ferma sull'ultima riga della finestra, mai oltre
    for _ in 0..50 {
        tasto(&mut a, KeyCode::Down, KeyModifiers::NONE);
    }
    assert_eq!(a.cursor(), offset[19]);
    // Pag.Giu' e Pag.Su' restano dentro
    tasto(&mut a, KeyCode::PageDown, KeyModifiers::NONE);
    assert!(a.cursor() >= offset[10] && a.cursor() < offset[20]);
    tasto(&mut a, KeyCode::PageUp, KeyModifiers::NONE);
    tasto(&mut a, KeyCode::PageUp, KeyModifiers::NONE);
    assert!(a.cursor() >= offset[10] && a.cursor() < offset[20]);
    assert_eq!(a.top(), offset[10]);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_big_window_scrolls_by_page_and_the_scrollbar_maps_to_it() {
    let (dati, offset) = log();
    // 10 righe di schermo: 7 di testo (titolo, stato e piede ne tolgono 3)
    let (mut a, p) = app_con(&dati, 10);
    a.apply_time_range("2026-11-01 02:10", "2026-11-01 03:30")
        .unwrap();
    assert_eq!(a.time_window(), Some((offset[10], offset[90])));
    // alla fine della finestra: l'ultima riga (la 89) e' in fondo allo schermo, la prima visibile e' la 83
    a.scroll_to_fraction(1.0);
    assert_eq!(a.top(), offset[83]);
    let s = schermo(&mut a, 10);
    assert!(s.contains("03:29:00") && !s.contains("03:30:00"), "{s}");
    a.scroll_to_fraction(0.0);
    assert_eq!(a.top(), offset[10]);
    // la scrollbar: dove sta la finestra come frazione della finestra, non del file
    let info = a.scroll_info();
    assert_eq!(info.start, 0.0);
    a.scroll_to_fraction(1.0);
    let info = a.scroll_info();
    assert!(
        info.start > 0.8 && info.start + info.size > 0.999,
        "{info:?}"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn select_all_and_search_stay_inside_the_window() {
    let (dati, offset) = log();
    let (mut a, p) = app_con(&dati, 24);
    a.apply_time_range("02:10", "02:20").unwrap();
    tasto(&mut a, KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(a.selection(), Some((offset[10], offset[20] - 1)));
    tasto(&mut a, KeyCode::Esc, KeyModifiers::NONE);
    // una riga fuori dalla finestra non si trova
    let prima = a.cursor();
    tasto(&mut a, KeyCode::Char('f'), KeyModifiers::CONTROL);
    a.dialog_set_text("riga 50");
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(
        a.cursor() >= offset[10] && a.cursor() < offset[20],
        "il cursore e' uscito dalla finestra: {}",
        a.cursor()
    );
    assert_eq!(a.cursor(), prima);
    // una dentro si'
    tasto(&mut a, KeyCode::Char('f'), KeyModifiers::CONTROL);
    a.dialog_set_text("riga 15");
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(
        a.cursor() >= offset[15] && a.cursor() < offset[16],
        "{}",
        a.cursor()
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn editing_is_refused_inside_a_window_and_possible_after_leaving_it() {
    let (dati, _) = log();
    let (mut a, p) = app_con(&dati, 24);
    a.apply_time_range("02:10", "02:20").unwrap();
    tasto(&mut a, KeyCode::F(4), KeyModifiers::NONE);
    assert!(!a.is_edit(), "in una finestra di tempo non si modifica");
    assert!(a.flash().is_some());
    a.clear_time_window();
    assert_eq!(a.time_window(), None);
    tasto(&mut a, KeyCode::F(4), KeyModifiers::NONE);
    assert!(a.is_edit());
    let _ = std::fs::remove_file(p);
}

#[test]
fn leaving_the_window_gives_the_whole_file_back() {
    let (dati, offset) = log();
    let (mut a, p) = app_con(&dati, 24);
    a.apply_time_range("02:10", "02:20").unwrap();
    a.clear_time_window();
    tasto(&mut a, KeyCode::End, KeyModifiers::CONTROL);
    assert_eq!(a.cursor(), offset[100] - 1);
    tasto(&mut a, KeyCode::Home, KeyModifiers::CONTROL);
    assert_eq!(a.cursor(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn another_mask_can_be_chosen_by_hand() {
    // un log nello stile syslog: la maschera ISO non lo legge, quella a mano si'
    let mut dati = String::new();
    for i in 0..30 {
        dati.push_str(&format!("Nov  1 02:{:02}:00 host app[1]: riga {i}\n", i));
    }
    let (mut a, p) = app_con(dati.as_bytes(), 24);
    // riconosciuta da sola
    assert_eq!(a.time_mask_pattern().as_deref(), Some("MMM d HH:mm:ss"));
    assert!(a.set_time_mask("yyyy-MM-dd HH:mm:ss", 0).is_ok());
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss")
    );
    assert_eq!(
        a.time_at_top(),
        None,
        "con la maschera sbagliata non c'e' un orario"
    );
    assert!(a.set_time_mask("MMM d HH:mm:ss", 0).is_ok());
    assert_eq!(a.time_at_top().as_deref(), Some("2000-11-01 02:00:00"));
    assert!(a.set_time_mask("yyyyy", 0).is_err());
    // la maschera sbagliata non ha cambiato niente
    assert_eq!(a.time_mask_pattern().as_deref(), Some("MMM d HH:mm:ss"));
    let _ = std::fs::remove_file(p);
}
