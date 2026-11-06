//! I salti di pagina (E39): Alt+PgUp/PgDn x10, Alt+Maiusc x100, Ctrl+Alt x1000 (i moltiplicatori sono nelle impostazioni).
//! Scritti da ARCH: gli attesi sono calcolati a mano. Il file ha 10.000 righe "riga NNNNN\n" di 11 byte; con un terminale
//! alto 10 le righe di testo sono 7 e una pagina vale 6 righe. L'ultima pagina comincia alla riga 9993 (9993..9999 sono le
//! sette righe dell'ultimo schermo).

use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const RIGA: u64 = 11;

fn testo(righe: usize) -> Vec<u8> {
    (0..righe)
        .flat_map(|i| format!("riga {i:05}\n").into_bytes())
        .collect()
}

fn app_con(dati: &[u8], cfg: AppConfig) -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_jump_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let _ = render_lines(&mut a, 40, 9);
    (a, p)
}

fn app(righe: usize) -> (App, PathBuf) {
    app_con(&testo(righe), AppConfig::default())
}

fn tasto(c: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(c, m)
}

fn premi(a: &mut App, c: KeyCode, m: KeyModifiers) {
    a.handle_key(tasto(c, m));
    let _ = render_lines(a, 40, 9);
}

const ALT: KeyModifiers = KeyModifiers::ALT;
const NESSUNO: KeyModifiers = KeyModifiers::NONE;

#[test]
fn a_plain_page_is_six_rows_here() {
    let (mut a, p) = app(10_000);
    premi(&mut a, KeyCode::PageDown, NESSUNO);
    assert_eq!((a.top(), a.cursor()), (6 * RIGA, 6 * RIGA));
    let _ = std::fs::remove_file(p);
}

#[test]
fn alt_page_keys_jump_ten_hundred_and_a_thousand_pages() {
    let (mut a, p) = app(10_000);
    premi(&mut a, KeyCode::PageDown, ALT);
    assert_eq!((a.top(), a.cursor()), (60 * RIGA, 60 * RIGA), "x10");
    premi(&mut a, KeyCode::PageDown, ALT | KeyModifiers::SHIFT);
    assert_eq!((a.top(), a.cursor()), (660 * RIGA, 660 * RIGA), "x100");
    premi(&mut a, KeyCode::PageDown, ALT | KeyModifiers::CONTROL);
    assert_eq!((a.top(), a.cursor()), (6660 * RIGA, 6660 * RIGA), "x1000");
    // e indietro, con gli stessi passi
    premi(&mut a, KeyCode::PageUp, ALT | KeyModifiers::CONTROL);
    assert_eq!((a.top(), a.cursor()), (660 * RIGA, 660 * RIGA));
    premi(&mut a, KeyCode::PageUp, ALT | KeyModifiers::SHIFT);
    assert_eq!((a.top(), a.cursor()), (60 * RIGA, 60 * RIGA));
    premi(&mut a, KeyCode::PageUp, ALT);
    assert_eq!((a.top(), a.cursor()), (0, 0));
    let _ = std::fs::remove_file(p);
}

#[test]
fn jumps_stop_at_the_last_page_and_at_the_start() {
    let (mut a, p) = app(10_000);
    let fondo = 9993 * RIGA;
    premi(&mut a, KeyCode::PageDown, ALT | KeyModifiers::CONTROL);
    premi(&mut a, KeyCode::PageDown, ALT | KeyModifiers::CONTROL);
    assert_eq!((a.top(), a.cursor()), (fondo, fondo), "ultima pagina");
    // gia' in fondo: non si muove
    premi(&mut a, KeyCode::PageDown, ALT | KeyModifiers::CONTROL);
    assert_eq!((a.top(), a.cursor()), (fondo, fondo));
    // indietro di 6000 righe da 9993, poi oltre l'inizio: si ferma a 0
    premi(&mut a, KeyCode::PageUp, ALT | KeyModifiers::CONTROL);
    assert_eq!((a.top(), a.cursor()), (3993 * RIGA, 3993 * RIGA));
    premi(&mut a, KeyCode::PageUp, ALT | KeyModifiers::CONTROL);
    assert_eq!((a.top(), a.cursor()), (0, 0));
    premi(&mut a, KeyCode::PageUp, ALT | KeyModifiers::CONTROL);
    assert_eq!((a.top(), a.cursor()), (0, 0));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_multipliers_come_from_the_configuration() {
    let cfg = AppConfig {
        jump_pages: [2, 3, 4],
        ..AppConfig::default()
    };
    let (mut a, p) = app_con(&testo(10_000), cfg);
    assert_eq!(a.jump_pages(), [2, 3, 4]);
    premi(&mut a, KeyCode::PageDown, ALT);
    assert_eq!(a.top(), 12 * RIGA, "2 pagine");
    premi(&mut a, KeyCode::PageDown, ALT | KeyModifiers::SHIFT);
    assert_eq!(a.top(), 30 * RIGA, "3 pagine");
    premi(&mut a, KeyCode::PageDown, ALT | KeyModifiers::CONTROL);
    assert_eq!(a.top(), 54 * RIGA, "4 pagine");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_default_multipliers_are_ten_hundred_thousand() {
    let (a, p) = app(10);
    assert_eq!(a.jump_pages(), [10, 100, 1000]);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_jump_is_a_plain_move_and_drops_the_selection() {
    let (mut a, p) = app(10_000);
    premi(&mut a, KeyCode::Right, KeyModifiers::SHIFT);
    assert_eq!(a.selection(), Some((0, 1)));
    premi(&mut a, KeyCode::PageDown, ALT);
    assert_eq!(a.selection(), None);
    // il cursore era alla colonna 1 e ci resta, 60 righe sotto
    assert_eq!(a.cursor(), 60 * RIGA + 1);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_short_file_does_not_move_and_does_not_panic() {
    let (mut a, p) = app(3);
    for m in [ALT, ALT | KeyModifiers::SHIFT, ALT | KeyModifiers::CONTROL] {
        premi(&mut a, KeyCode::PageDown, m);
        assert_eq!((a.top(), a.cursor()), (0, 0));
        premi(&mut a, KeyCode::PageUp, m);
        assert_eq!((a.top(), a.cursor()), (0, 0));
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_jump_works_in_edit_mode_and_typing_goes_there() {
    let (mut a, p) = app(10_000);
    premi(&mut a, KeyCode::F(4), NESSUNO);
    premi(&mut a, KeyCode::PageDown, ALT);
    assert_eq!(a.cursor(), 60 * RIGA);
    a.handle_key(tasto(KeyCode::Char('X'), NESSUNO));
    let t = a.document_text();
    let da = (60 * RIGA) as usize;
    assert_eq!(&t[da..da + 11], b"Xriga 00060");
    let _ = std::fs::remove_file(p);
}

#[test]
fn page_jump_is_public_for_the_buttons() {
    let (mut a, p) = app(10_000);
    a.page_jump(true, 10);
    assert_eq!(a.top(), 60 * RIGA);
    a.page_jump(false, 10);
    assert_eq!(a.top(), 0);
    // zero pagine non fa niente
    a.page_jump(true, 0);
    assert_eq!(a.top(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn changing_the_theme_keeps_the_configured_jumps_in_the_settings_file() {
    let dir = std::env::temp_dir().join(format!("mrp_jump_prof_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let settings = dir.join("settings.txt");
    let cfg = AppConfig {
        jump_pages: [5, 50, 500],
        settings_path: Some(settings.clone()),
        ..AppConfig::default()
    };
    let (mut a, p) = app_con(&testo(20), cfg);
    // F6, un tema piu' su, Invio: il tema cambia e le impostazioni si scrivono
    premi(&mut a, KeyCode::F(6), NESSUNO);
    premi(&mut a, KeyCode::Up, NESSUNO);
    premi(&mut a, KeyCode::Enter, NESSUNO);
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("jump_pages=5,50,500"), "{scritto}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(p);
}
