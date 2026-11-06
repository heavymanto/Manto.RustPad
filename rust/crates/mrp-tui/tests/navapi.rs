//! L'API dei salti del `Workspace` (B3, parte di ARCH): `jump`, `go_start`, `go_end`, `jump_pages`, e `senza_nav_celle` (la
//! finestra disegna la barra dei salti con i suoi pulsanti, non a celle). Scritti da ARCH: gli attesi sono calcolati a mano.
//! Il file ha 30.000 righe "riga NNNNN\n" di 11 byte; con un terminale alto 24 senza barre il testo ha 21 righe: una pagina e' 20.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::Cmd;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const RIGA: u64 = 11;

fn file() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_navapi_test_{}_{}.txt", std::process::id(), n));
    let dati: Vec<u8> = (0..30_000)
        .flat_map(|i| format!("riga {i:05}\n").into_bytes())
        .collect();
    std::fs::write(&p, dati).unwrap();
    p
}

fn ws(cfg: AppConfig, celle: bool) -> (Workspace, PathBuf) {
    let p = file();
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut ws = Workspace::new(app, cfg);
    if !celle {
        ws = ws.senza_nav_celle();
    }
    ws.resize(80, 23);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    let _ = ws.render_lines(80, 23);
    (ws, p)
}

#[test]
fn jump_moves_by_whole_pages_in_either_direction_and_stops_at_the_ends() {
    let (mut w, p) = ws(AppConfig::default(), true);
    w.jump(true, 1);
    assert_eq!(w.active().top(), 20 * RIGA);
    w.jump(true, 10);
    assert_eq!(w.active().top(), 220 * RIGA);
    w.jump(false, 10);
    assert_eq!(w.active().top(), 20 * RIGA);
    w.jump(false, 5);
    assert_eq!(w.active().top(), 0, "oltre l'inizio: si ferma");
    w.jump(true, 0);
    assert_eq!(w.active().top(), 0, "zero pagine non fa niente");
    w.jump(true, 10_000);
    // l'ultima pagina: 21 righe visibili, l'ultima riga e' la 29999, la prima visibile la 29979
    assert_eq!(w.active().top(), 29_979 * RIGA);
    let _ = std::fs::remove_file(p);
}

#[test]
fn go_start_and_go_end() {
    let (mut w, p) = ws(AppConfig::default(), true);
    w.go_end();
    assert_eq!(w.active().top(), 29_979 * RIGA);
    w.go_start();
    assert_eq!((w.active().top(), w.active().cursor()), (0, 0));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_home_and_end_commands_do_the_same() {
    let (mut w, p) = ws(AppConfig::default(), true);
    assert!(w.exec(Cmd::End));
    assert_eq!(w.active().top(), 29_979 * RIGA);
    assert!(w.exec(Cmd::Home));
    assert_eq!(w.active().top(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn jump_pages_are_the_configured_multipliers() {
    let (w, p) = ws(AppConfig::default(), true);
    assert_eq!(w.jump_pages(), [10, 100, 1000]);
    let cfg = AppConfig {
        jump_pages: [2, 3, 4],
        ..AppConfig::default()
    };
    let (w2, q) = ws(cfg, true);
    assert_eq!(w2.jump_pages(), [2, 3, 4]);
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn without_the_cell_bar_the_jump_bar_flag_takes_no_row() {
    let (mut w, p) = ws(AppConfig::default(), false);
    w.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
    assert!(w.nav_bar(), "il flag c'e': lo legge la finestra");
    let righe = w.render_lines(80, 23);
    assert!(
        righe[0].contains("riga 00000"),
        "nessuna riga di pulsanti a celle: la prima riga e' il testo: {:?}",
        righe[0]
    );
    // l'altezza del testo non cambia: una pagina e' 20 righe (con la barra a celle sarebbe 19)
    w.jump(true, 1);
    assert_eq!(w.active().top(), 20 * RIGA);
    // e un clic in alto non e' un pulsante
    let _ = std::fs::remove_file(p);
}

#[test]
fn with_the_cell_bar_a_page_is_nineteen_rows() {
    let (mut w, p) = ws(AppConfig::default(), true);
    w.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
    let righe = w.render_lines(80, 23);
    assert!(righe[0].starts_with("[Home]"));
    w.jump(true, 1);
    assert_eq!(w.active().top(), 19 * RIGA);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_cell_bar_can_be_switched_off_and_on_at_run_time() {
    let (mut w, p) = ws(AppConfig::default(), true);
    w.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
    w.jump(true, 1);
    assert_eq!(
        w.active().top(),
        19 * RIGA,
        "con la barra a celle: 19 righe"
    );
    w.go_start();
    w.set_nav_cells(false);
    assert!(w.nav_bar());
    w.jump(true, 1);
    assert_eq!(w.active().top(), 20 * RIGA, "senza: lo spazio e' tornato");
    w.go_start();
    w.set_nav_cells(true);
    let righe = w.render_lines(80, 23);
    assert!(righe[0].starts_with("[Home]"));
    w.jump(true, 1);
    assert_eq!(w.active().top(), 19 * RIGA);
    let _ = std::fs::remove_file(p);
}
