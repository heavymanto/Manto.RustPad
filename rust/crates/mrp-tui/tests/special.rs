//! I caratteri speciali visibili nell'editor (fase 2, 1.2): Alt+W (o la voce del menu Visualizza) mostra spazi, tabulazioni e
//! fine riga; la scelta vale per tutte le schede, si salva nelle impostazioni (`special=1`) e si rispetta all'avvio. Il disegno
//! dei caratteri sta in `mrp-core` (`ViewOptions::special`, provato a parte). Scritti da ARCH: gli attesi sono calcolati a mano.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::{self, Cmd};
use mrp_tui::settings::Settings;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_special_tui_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ws_con(cfg: AppConfig) -> (PathBuf, Workspace) {
    let d = cartella();
    let p = d.join("a.txt");
    std::fs::write(&p, b"a b\tc\nx\n").unwrap();
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut ws = Workspace::new(app, cfg);
    ws.resize(60, 12);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    (d, ws)
}

fn alt_w() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT)
}

fn righe(ws: &mut Workspace) -> String {
    ws.render_lines(60, 12).join("\n")
}

#[test]
fn the_command_is_alt_w_in_the_view_menu_and_the_registry() {
    let c = Cmd::Special.info();
    assert_eq!(c.label, "gui.special");
    assert_eq!(c.shortcut, "Alt+W");
    assert_eq!(c.key, Some(alt_w()));
    assert_eq!(commands::from_key(alt_w()), Some(Cmd::Special));
    assert!(commands::COMMANDS.iter().any(|x| x.cmd == Cmd::Special));
}

#[test]
fn it_is_off_at_the_start_and_alt_w_toggles_it() {
    let (d, mut ws) = ws_con(AppConfig::default());
    assert!(!ws.special_chars());
    let prima = righe(&mut ws);
    assert!(prima.contains("a b c"), "{prima}");
    assert!(!prima.contains('\u{b6}'));
    ws.handle_key(alt_w());
    assert!(ws.special_chars());
    let dopo = righe(&mut ws);
    assert!(dopo.contains("a\u{b7}b\u{2192}c\u{b6}"), "{dopo}");
    assert!(dopo.contains("x\u{b6}"), "{dopo}");
    ws.handle_key(alt_w());
    assert!(!ws.special_chars());
    assert_eq!(righe(&mut ws), prima);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_menu_command_does_what_the_key_does() {
    let (d, mut ws) = ws_con(AppConfig::default());
    assert!(commands::enabled(&ws, Cmd::Special));
    assert!(ws.exec(Cmd::Special));
    assert!(ws.special_chars());
    assert!(righe(&mut ws).contains('\u{b6}'));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn it_applies_to_every_tab_and_to_new_ones() {
    let (d, mut ws) = ws_con(AppConfig::default());
    let altro = d.join("b.txt");
    std::fs::write(&altro, b"q r\n").unwrap();
    ws.open(&altro).unwrap();
    ws.handle_key(alt_w());
    assert!(ws.tabs().iter().all(|a| a.special_chars()));
    let terzo = d.join("c.txt");
    std::fs::write(&terzo, b"m n\n").unwrap();
    ws.open(&terzo).unwrap();
    assert!(
        ws.tabs().iter().all(|a| a.special_chars()),
        "anche la nuova"
    );
    let r = righe(&mut ws);
    assert!(r.contains("m\u{b7}n\u{b6}"), "{r}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_cursor_and_the_mouse_keep_their_columns() {
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let (d, mut ws) = ws_con(AppConfig::default());
    ws.handle_key(alt_w());
    // il testo parte dopo il margine dei numeri: un clic sulla 'c' (colonna 4 del testo) mette il cursore a byte 4
    let riga = ws
        .render_lines(60, 12)
        .iter()
        .position(|r| r.contains("a\u{b7}b\u{2192}c"))
        .unwrap();
    let x = ws.render_lines(60, 12)[riga]
        .chars()
        .position(|c| c == 'c')
        .unwrap() as u16;
    ws.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: riga as u16,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(ws.active().cursor(), 4);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_choice_is_saved_in_the_settings_and_respected_at_the_start() {
    let prof = std::env::temp_dir().join(format!("mrp_special_prof_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&prof);
    let settings = prof.join("settings.txt");
    let cfg = AppConfig {
        settings_path: Some(settings.clone()),
        ..AppConfig::default()
    };
    let (d, mut ws) = ws_con(cfg);
    ws.handle_key(alt_w());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("special=1"), "{scritto}");
    ws.handle_key(alt_w());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(!scritto.contains("special"), "{scritto}");
    // all'avvio la scelta salvata si rispetta
    let cfg = AppConfig {
        special_chars: true,
        ..AppConfig::default()
    };
    let (d2, mut ws2) = ws_con(cfg);
    assert!(ws2.special_chars());
    assert!(righe(&mut ws2).contains('\u{b6}'));
    let _ = std::fs::remove_dir_all(prof);
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_dir_all(d2);
}

#[test]
fn a_theme_change_keeps_the_choice_saved() {
    let prof = std::env::temp_dir().join(format!("mrp_special_prof2_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&prof);
    let settings = prof.join("settings.txt");
    let cfg = AppConfig {
        settings_path: Some(settings.clone()),
        ..AppConfig::default()
    };
    let (d, mut ws) = ws_con(cfg);
    ws.handle_key(alt_w());
    ws.handle_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
    ws.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    ws.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("special=1"), "{scritto}");
    let _ = std::fs::remove_dir_all(prof);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_dialog_keeps_alt_w_for_itself() {
    let (d, mut ws) = ws_con(AppConfig::default());
    ws.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
    ws.handle_key(alt_w());
    assert!(!ws.special_chars());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_setting_is_off_by_default_written_only_when_on_and_read_back() {
    assert!(!Settings::default().special_chars);
    assert!(!Settings::default().to_text().contains("special"));
    let s = Settings {
        special_chars: true,
        theme_set: true,
        ..Settings::default()
    };
    let t = s.to_text();
    let dati: Vec<&str> = t.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(dati, vec!["theme=console", "special=1"]);
    assert_eq!(Settings::from_text(&t), s);
    for si in ["special=1", "special = on", "special=true"] {
        assert!(Settings::from_text(si).special_chars, "{si}");
    }
    for no in [
        "special=0",
        "special=off",
        "special=false",
        "SPECIAL=1",
        "special=si",
    ] {
        assert!(!Settings::from_text(no).special_chars, "{no}");
    }
}
