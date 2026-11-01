//! La colorazione dei log nell'editor (fase 2, 3.2): Alt+L accende i colori dei livelli, degli orari e degli indirizzi (il
//! modulo `loghl` del core li trova, il tema li colora); la scelta vale per tutte le schede e si salva (`log_colors=1`).
//! Scritti da ARCH: gli attesi sono i colori che il tema Console dichiara (Cyan per l'ora, Magenta per l'indirizzo, LightRed
//! per l'errore, Giallo per l'avviso, Verde per l'info) tradotti dalla tavolozza ANSI; il testo ha 4 righe.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::cells::{colore, render_grid_workspace, FG_PREDEFINITO};
use mrp_tui::commands::{self, Cmd};
use mrp_tui::settings::Settings;
use mrp_tui::theme::ThemeId;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const TESTO: &str = "2026-10-31 09:00:53 ERROR 10.0.0.1 boom\n2026-10-31 09:00:54 WARN slow query\n2026-10-31 09:00:55 INFO all good\nplain text 12\n";

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_logcol_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ws_con(cfg: AppConfig, testo: &str) -> (PathBuf, Workspace) {
    let d = cartella();
    let p = d.join("a.log");
    std::fs::write(&p, testo).unwrap();
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut ws = Workspace::new(app, cfg);
    ws.resize(60, 10);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    (d, ws)
}

fn alt_l() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT)
}

fn cfg(tema: ThemeId) -> AppConfig {
    AppConfig {
        theme: tema,
        ..AppConfig::default()
    }
}

/// Dove comincia il testo (colonna dopo il gutter, riga dello schermo della prima riga di testo): e' dov'e' il cursore finche'
/// sta a 0. Cosi' vale con una scheda sola o con la barra delle schede, con qualunque larghezza del gutter.
fn origine(ws: &mut Workspace) -> (u16, u16) {
    render_grid_workspace(ws, 60, 10).cursore.expect("cursore")
}

/// Il colore di primo piano della cella (colonna di testo `x`, riga di testo `riga`), con il cursore a 0.
fn fg(ws: &mut Workspace, x: u16, riga: u16) -> [u8; 3] {
    let g = render_grid_workspace(ws, 60, 10);
    let (cx, cy) = g.cursore.expect("cursore");
    g.cella(cx + x, cy + riga).expect("cella").fg
}

fn ansi(c: Color) -> [u8; 3] {
    colore(c, FG_PREDEFINITO)
}

#[test]
fn the_command_is_alt_l_and_it_starts_off() {
    let c = Cmd::LogColors.info();
    assert_eq!(c.label, "gui.log_colors");
    assert_eq!(c.shortcut, "Alt+L");
    assert_eq!(c.key, Some(alt_l()));
    assert_eq!(commands::from_key(alt_l()), Some(Cmd::LogColors));
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    assert!(!ws.log_colors());
    // spento: tutto il testo e' del colore del documento (Bianco)
    for riga in 0..4 {
        for x in [0u16, 5, 20, 26, 36] {
            assert_eq!(fg(&mut ws, x, riga), ansi(Color::White), "{x},{riga}");
        }
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn on_the_levels_times_and_addresses_take_the_theme_colours() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    ws.handle_key(alt_l());
    assert!(ws.log_colors());
    // riga 0: "2026-10-31 09:00:53 ERROR 10.0.0.1 boom": ora 0..19, ERROR 20..25, indirizzo 26..34, "boom" 35..39
    assert_eq!(fg(&mut ws, 0, 0), ansi(Color::Cyan), "l'ora");
    assert_eq!(fg(&mut ws, 18, 0), ansi(Color::Cyan));
    assert_eq!(fg(&mut ws, 20, 0), ansi(Color::LightRed), "ERROR");
    assert_eq!(fg(&mut ws, 26, 0), ansi(Color::Magenta), "l'indirizzo");
    assert_eq!(fg(&mut ws, 33, 0), ansi(Color::Magenta));
    assert_eq!(
        fg(&mut ws, 35, 0),
        ansi(Color::LightRed),
        "il resto della riga d'errore"
    );
    // riga 1: avviso, la riga e' gialla (l'ora resta ciano)
    assert_eq!(fg(&mut ws, 0, 1), ansi(Color::Cyan));
    assert_eq!(fg(&mut ws, 20, 1), ansi(Color::Yellow), "WARN");
    assert_eq!(
        fg(&mut ws, 26, 1),
        ansi(Color::Yellow),
        "il resto dell'avviso"
    );
    // riga 2: info, solo la parola e' verde, il resto e' del colore del documento
    assert_eq!(fg(&mut ws, 20, 2), ansi(Color::Green), "INFO");
    assert_eq!(
        fg(&mut ws, 25, 2),
        ansi(Color::White),
        "il resto resta com'e'"
    );
    // riga 3: niente da colorare
    for x in [0u16, 6, 11, 12] {
        assert_eq!(fg(&mut ws, x, 3), ansi(Color::White), "{x}");
    }
    // Alt+L di nuovo: tutto torna com'era
    ws.handle_key(alt_l());
    assert!(!ws.log_colors());
    assert_eq!(fg(&mut ws, 20, 0), ansi(Color::White));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_error_row_is_bold_in_console_and_the_light_theme_uses_its_rgb_colours() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    ws.handle_key(alt_l());
    let g = render_grid_workspace(&mut ws, 60, 10);
    let (cx, cy) = g.cursore.unwrap();
    assert!(
        g.cella(cx + 20, cy).unwrap().grassetto,
        "ERROR in grassetto"
    );
    assert!(!g.cella(cx + 20, cy + 2).unwrap().grassetto, "INFO no");
    let _ = std::fs::remove_dir_all(d);
    let (d, mut ws) = ws_con(cfg(ThemeId::Light), TESTO);
    ws.handle_key(alt_l());
    assert_eq!(fg(&mut ws, 20, 0), [0xa3, 0x15, 0x15]);
    assert_eq!(fg(&mut ws, 0, 0), [0x26, 0x7f, 0x99]);
    assert_eq!(fg(&mut ws, 26, 0), [0xaf, 0x00, 0xdb]);
    assert_eq!(fg(&mut ws, 20, 1), [0x98, 0x68, 0x01]);
    assert_eq!(fg(&mut ws, 20, 2), [0x09, 0x86, 0x58]);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_selection_wins_over_the_log_colours() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    ws.handle_key(alt_l());
    let (cx, cy) = origine(&mut ws);
    ws.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    // il tema Console: selezione bianco su blu
    let g = render_grid_workspace(&mut ws, 60, 10);
    assert_eq!(g.cella(cx + 20, cy).unwrap().fg, ansi(Color::White));
    assert_eq!(g.cella(cx + 20, cy).unwrap().bg, ansi(Color::Blue));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn wide_characters_before_a_level_keep_the_colour_on_the_right_cells() {
    // "日本" occupa 4 celle, poi uno spazio: ERROR comincia alla cella 5
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), "日本 ERROR fine\n");
    ws.handle_key(alt_l());
    assert_eq!(fg(&mut ws, 5, 0), ansi(Color::LightRed));
    assert_eq!(fg(&mut ws, 9, 0), ansi(Color::LightRed));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn it_applies_to_every_tab_and_new_ones() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    let altro = d.join("b.log");
    std::fs::write(&altro, "x\n").unwrap();
    ws.open(&altro).unwrap();
    ws.handle_key(alt_l());
    assert!(ws.tabs().iter().all(|a| a.log_colors()));
    let terzo = d.join("c.log");
    std::fs::write(&terzo, "ERROR e\n").unwrap();
    ws.open(&terzo).unwrap();
    assert!(ws.tabs().iter().all(|a| a.log_colors()), "anche la nuova");
    ws.resize(60, 10);
    // con tre schede la barra delle schede e' la riga 0: il testo comincia alla riga 1 come prima
    assert_eq!(fg(&mut ws, 0, 0), ansi(Color::LightRed));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_menu_command_does_what_the_key_does() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    assert!(commands::enabled(&ws, Cmd::LogColors));
    assert!(ws.exec(Cmd::LogColors));
    assert!(ws.log_colors());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_dialog_keeps_alt_l_for_itself() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    ws.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
    ws.handle_key(alt_l());
    assert!(!ws.log_colors());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_choice_is_saved_and_respected_at_the_start() {
    let prof = std::env::temp_dir().join(format!("mrp_logcol_prof_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&prof);
    let settings = prof.join("settings.txt");
    let c = AppConfig {
        settings_path: Some(settings.clone()),
        ..AppConfig::default()
    };
    let (d, mut ws) = ws_con(c, TESTO);
    ws.handle_key(alt_l());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("log_colors=1"), "{scritto}");
    ws.handle_key(alt_l());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(!scritto.contains("log_colors"), "{scritto}");
    let c = AppConfig {
        log_colors: true,
        ..AppConfig::default()
    };
    let (d2, mut ws2) = ws_con(c, TESTO);
    assert!(ws2.log_colors());
    assert_eq!(fg(&mut ws2, 20, 0), ansi(Color::LightRed));
    let _ = std::fs::remove_dir_all(prof);
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_dir_all(d2);
}

#[test]
fn the_setting_is_off_by_default_written_only_when_on_and_read_back() {
    assert!(!Settings::default().log_colors);
    assert!(!Settings::default().to_text().contains("log_colors"));
    let s = Settings {
        log_colors: true,
        theme_set: true,
        ..Settings::default()
    };
    let t = s.to_text();
    let dati: Vec<&str> = t.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(dati, vec!["theme=console", "log_colors=1"]);
    assert_eq!(Settings::from_text(&t), s);
    for si in ["log_colors=1", "log_colors = on", "log_colors=true"] {
        assert!(Settings::from_text(si).log_colors, "{si}");
    }
    for no in [
        "log_colors=0",
        "log_colors=off",
        "LOG_COLORS=1",
        "log_colors=si",
    ] {
        assert!(!Settings::from_text(no).log_colors, "{no}");
    }
}
