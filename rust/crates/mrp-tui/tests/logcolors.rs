//! La colorazione dei log nell'editor (fase 2, 3.2): di fabbrica e' accesa, Alt+L la spegne e la riaccende; colora le parole dei
//! livelli, gli orari e gli indirizzi (il modulo `loghl` del core li trova, il tema li colora) e il filetto del margine a inizio
//! riga nel colore del livello; il resto della riga resta com'e'. La scelta vale per tutte le schede e si salva (`log_colors=0`
//! quando e' spenta). Scritti da ARCH: gli attesi sono i colori che il tema Console dichiara (Cyan per l'ora, Magenta per
//! l'indirizzo, LightRed per l'errore, Giallo per l'avviso, Verde per l'info) tradotti dalla tavolozza ANSI; il testo ha 4 righe.

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

/// Il colore del filetto del margine (la cella subito prima del testo) alla riga di testo `riga`.
fn filetto(ws: &mut Workspace, riga: u16) -> [u8; 3] {
    let g = render_grid_workspace(ws, 60, 10);
    let (cx, cy) = g.cursore.expect("cursore");
    let c = g.cella(cx - 1, cy + riga).expect("cella");
    assert_eq!(c.simbolo, "│", "il filetto del margine");
    c.fg
}

fn ansi(c: Color) -> [u8; 3] {
    colore(c, FG_PREDEFINITO)
}

#[test]
fn the_command_is_alt_l_and_it_starts_on() {
    let c = Cmd::LogColors.info();
    assert_eq!(c.label, "gui.log_colors");
    assert_eq!(c.shortcut, "Alt+L");
    assert_eq!(c.key, Some(alt_l()));
    assert_eq!(commands::from_key(alt_l()), Some(Cmd::LogColors));
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    assert!(ws.log_colors());
    assert_eq!(fg(&mut ws, 20, 0), ansi(Color::LightRed), "ERROR");
    // spento: tutto il testo e' del colore del documento (Bianco)
    ws.handle_key(alt_l());
    assert!(!ws.log_colors());
    for riga in 0..4 {
        for x in [0u16, 5, 20, 26, 36] {
            assert_eq!(fg(&mut ws, x, riga), ansi(Color::White), "{x},{riga}");
        }
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_levels_times_and_addresses_take_the_theme_colours_and_the_rest_of_the_row_stays() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    assert!(ws.log_colors());
    // riga 0: "2026-10-31 09:00:53 ERROR 10.0.0.1 boom": ora 0..19, ERROR 20..25, indirizzo 26..34, "boom" 35..39
    assert_eq!(fg(&mut ws, 0, 0), ansi(Color::Cyan), "l'ora");
    assert_eq!(fg(&mut ws, 18, 0), ansi(Color::Cyan));
    assert_eq!(fg(&mut ws, 20, 0), ansi(Color::LightRed), "ERROR");
    assert_eq!(fg(&mut ws, 24, 0), ansi(Color::LightRed));
    assert_eq!(fg(&mut ws, 26, 0), ansi(Color::Magenta), "l'indirizzo");
    assert_eq!(fg(&mut ws, 33, 0), ansi(Color::Magenta));
    assert_eq!(
        fg(&mut ws, 35, 0),
        ansi(Color::White),
        "il resto della riga d'errore non si colora"
    );
    // riga 1: avviso, la parola e' gialla (l'ora resta ciano) e il resto no
    assert_eq!(fg(&mut ws, 0, 1), ansi(Color::Cyan));
    assert_eq!(fg(&mut ws, 20, 1), ansi(Color::Yellow), "WARN");
    assert_eq!(fg(&mut ws, 26, 1), ansi(Color::White), "il resto");
    // riga 2: info, solo la parola e' verde
    assert_eq!(fg(&mut ws, 20, 2), ansi(Color::Green), "INFO");
    assert_eq!(fg(&mut ws, 25, 2), ansi(Color::White), "il resto");
    // riga 3: niente da colorare
    for x in [0u16, 6, 11, 12] {
        assert_eq!(fg(&mut ws, x, 3), ansi(Color::White), "{x}");
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_margin_line_takes_the_colour_of_the_level_of_the_row() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    assert_eq!(filetto(&mut ws, 0), ansi(Color::LightRed), "ERROR");
    assert_eq!(filetto(&mut ws, 1), ansi(Color::Yellow), "WARN");
    assert_eq!(filetto(&mut ws, 2), ansi(Color::Green), "INFO");
    // una riga senza livello ha il filetto del margine di sempre
    let senza = filetto(&mut ws, 3);
    for colore_livello in [Color::LightRed, Color::Yellow, Color::Green, Color::Cyan] {
        assert_ne!(senza, ansi(colore_livello));
    }
    // spenti i colori, il filetto torna uguale a quello della riga senza livello
    ws.handle_key(alt_l());
    for riga in 0..3 {
        assert_eq!(filetto(&mut ws, riga), senza, "riga {riga}");
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_margin_line_follows_the_first_level_when_a_row_has_more_than_one() {
    let (d, mut ws) = ws_con(
        cfg(ThemeId::Console),
        "x WARN then ERROR later\nERROR then WARN\n",
    );
    assert_eq!(filetto(&mut ws, 0), ansi(Color::Yellow));
    assert_eq!(filetto(&mut ws, 1), ansi(Color::LightRed));
    // le parole sono colorate tutte e due: "x WARN then ERROR later", WARN in 2..6 ed ERROR in 12..17
    assert_eq!(fg(&mut ws, 2, 0), ansi(Color::Yellow));
    assert_eq!(fg(&mut ws, 12, 0), ansi(Color::LightRed));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn an_address_before_the_level_does_not_make_the_margin_line() {
    let (d, mut ws) = ws_con(
        cfg(ThemeId::Console),
        "10.0.0.1 ERROR boom\n10.0.0.2 only\n",
    );
    assert_eq!(
        filetto(&mut ws, 0),
        ansi(Color::LightRed),
        "il livello, non l'indirizzo"
    );
    // l'indirizzo e' colorato lo stesso, ma una riga con il solo indirizzo non ha il filetto colorato
    assert_eq!(fg(&mut ws, 0, 0), ansi(Color::Magenta));
    let senza = filetto(&mut ws, 1);
    assert_ne!(senza, ansi(Color::Magenta));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn without_line_numbers_there_is_no_margin_and_the_words_are_still_coloured() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    ws.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT));
    assert!(!ws.tabs()[0].line_numbers());
    assert_eq!(fg(&mut ws, 20, 0), ansi(Color::LightRed));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_error_word_is_bold_in_console_and_the_light_theme_uses_its_rgb_colours() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    let g = render_grid_workspace(&mut ws, 60, 10);
    let (cx, cy) = g.cursore.unwrap();
    assert!(
        g.cella(cx + 20, cy).unwrap().grassetto,
        "ERROR in grassetto"
    );
    assert!(!g.cella(cx + 20, cy + 2).unwrap().grassetto, "INFO no");
    assert!(
        !g.cella(cx + 35, cy).unwrap().grassetto,
        "il resto della riga no"
    );
    let _ = std::fs::remove_dir_all(d);
    let (d, mut ws) = ws_con(cfg(ThemeId::Light), TESTO);
    assert_eq!(fg(&mut ws, 20, 0), [0xa3, 0x15, 0x15]);
    assert_eq!(fg(&mut ws, 0, 0), [0x26, 0x7f, 0x99]);
    assert_eq!(fg(&mut ws, 26, 0), [0xaf, 0x00, 0xdb]);
    assert_eq!(fg(&mut ws, 20, 1), [0x98, 0x68, 0x01]);
    assert_eq!(fg(&mut ws, 20, 2), [0x09, 0x86, 0x58]);
    assert_eq!(filetto(&mut ws, 0), [0xa3, 0x15, 0x15]);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_selection_wins_over_the_log_colours() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
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
    assert_eq!(fg(&mut ws, 5, 0), ansi(Color::LightRed));
    assert_eq!(fg(&mut ws, 9, 0), ansi(Color::LightRed));
    assert_eq!(fg(&mut ws, 11, 0), ansi(Color::White));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn it_applies_to_every_tab_and_new_ones() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    let altro = d.join("b.log");
    std::fs::write(&altro, "x\n").unwrap();
    ws.open(&altro).unwrap();
    assert!(ws.tabs().iter().all(|a| a.log_colors()));
    ws.handle_key(alt_l());
    assert!(ws.tabs().iter().all(|a| !a.log_colors()));
    let terzo = d.join("c.log");
    std::fs::write(&terzo, "ERROR e\n").unwrap();
    ws.open(&terzo).unwrap();
    assert!(ws.tabs().iter().all(|a| !a.log_colors()), "anche la nuova");
    ws.resize(60, 10);
    // con tre schede la barra delle schede e' la riga 0: il testo comincia alla riga 1 come prima
    assert_eq!(fg(&mut ws, 0, 0), ansi(Color::White));
    ws.handle_key(alt_l());
    assert!(ws.tabs().iter().all(|a| a.log_colors()));
    assert_eq!(fg(&mut ws, 0, 0), ansi(Color::LightRed));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_menu_command_does_what_the_key_does() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    assert!(commands::enabled(&ws, Cmd::LogColors));
    assert!(ws.exec(Cmd::LogColors));
    assert!(!ws.log_colors());
    assert!(ws.exec(Cmd::LogColors));
    assert!(ws.log_colors());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_dialog_keeps_alt_l_for_itself() {
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), TESTO);
    ws.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
    ws.handle_key(alt_l());
    assert!(ws.log_colors(), "con un dialogo aperto non cambia");
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
    assert!(scritto.contains("log_colors=0"), "{scritto}");
    ws.handle_key(alt_l());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(!scritto.contains("log_colors"), "{scritto}");
    let c = AppConfig {
        log_colors: false,
        ..AppConfig::default()
    };
    let (d2, mut ws2) = ws_con(c, TESTO);
    assert!(!ws2.log_colors());
    assert_eq!(fg(&mut ws2, 20, 0), ansi(Color::White));
    let _ = std::fs::remove_dir_all(prof);
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_dir_all(d2);
}

#[test]
fn the_setting_is_on_by_default_written_only_when_off_and_read_back() {
    assert!(Settings::default().log_colors);
    assert!(!Settings::default().to_text().contains("log_colors"));
    assert!(Settings::from_text("").log_colors, "senza la voce: acceso");
    let s = Settings {
        log_colors: false,
        theme_set: true,
        ..Settings::default()
    };
    let t = s.to_text();
    let dati: Vec<&str> = t.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(dati, vec!["theme=console", "log_colors=0"]);
    assert_eq!(Settings::from_text(&t), s);
    for no in ["log_colors=0", "log_colors=off", "log_colors = false"] {
        assert!(!Settings::from_text(no).log_colors, "{no}");
    }
    // un file di prima (`log_colors=1`) e una voce che non si capisce lasciano il valore di fabbrica
    for si in [
        "log_colors=1",
        "log_colors = on",
        "log_colors=true",
        "LOG_COLORS=0",
        "log_colors=forse",
    ] {
        assert!(Settings::from_text(si).log_colors, "{si}");
    }
}

#[test]
fn a_time_is_coloured_only_where_the_format_of_the_file_reads_it() {
    // il formato del file e' "yyyy-MM-dd HH:mm:ss" all'inizio della riga: un'altra data a meta' riga, o una scritta come
    // un orario che il formato non legge, non si colora (prima si colorava tutto cio' che "somigliava" a una data)
    let testo =
        "2026-10-31 09:00:53 INFO a\n2026-10-31 09:00:54 INFO b\n2026-10-31 09:00:55 INFO c\n\
                 2026-10-31 09:00:56 INFO see 2019-01-01 00:00:00 and 12:30:45 here\n";
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), testo);
    // le righe con l'orario al suo posto: colorato (i 19 caratteri)
    for riga in 0..4 {
        assert_eq!(fg(&mut ws, 0, riga), ansi(Color::Cyan), "riga {riga}");
        assert_eq!(fg(&mut ws, 18, riga), ansi(Color::Cyan), "riga {riga}");
        assert_eq!(fg(&mut ws, 19, riga), ansi(Color::White), "riga {riga}");
    }
    // la data e l'orario dentro la quarta riga: "INFO see " finisce in colonna 28, la data comincia in 29 e l'orario in 52
    assert_eq!(fg(&mut ws, 29, 3), ansi(Color::White), "data a meta' riga");
    assert_eq!(
        fg(&mut ws, 52, 3),
        ansi(Color::White),
        "orario a meta' riga"
    );
    // l'orario non fa il filetto del margine: lo fa il livello (INFO)
    assert_eq!(filetto(&mut ws, 0), ansi(Color::Green));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn without_a_recognised_format_there_is_no_time_colour_but_levels_and_addresses_still_are() {
    let testo = "ERROR boom 10.0.0.1
plain 2019 text
";
    let (d, mut ws) = ws_con(cfg(ThemeId::Console), testo);
    assert!(ws.log_colors());
    assert_eq!(ws.active_mut().time_mask_pattern(), None);
    assert_eq!(ws.active_mut().time_mask_for_colors(), None);
    // il livello e l'indirizzo si colorano come sempre
    assert_eq!(fg(&mut ws, 0, 0), ansi(Color::LightRed), "ERROR");
    assert_eq!(fg(&mut ws, 11, 0), ansi(Color::Magenta), "l'indirizzo");
    let _ = std::fs::remove_dir_all(d);
}
