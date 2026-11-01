//! I numeri di riga accesi e spenti (Alt+N, o il menu Visualizza nella finestra): di fabbrica si vedono; spenti il margine
//! sparisce del tutto (niente numeri e niente filetto) e le colonne del testo, il cursore e il mouse partono dal bordo.
//! La scelta vale per tutte le schede, si salva (`line_numbers=0`) e si rispetta all'avvio. Scritti da ARCH.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::{self, Cmd};
use mrp_tui::settings::Settings;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_lineno_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ws_con(cfg: AppConfig) -> (PathBuf, Workspace) {
    let d = cartella();
    let p = d.join("a.txt");
    std::fs::write(&p, "uno\ndue\ntre\n").unwrap();
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut w = Workspace::new(app, cfg);
    w.resize(40, 10);
    assert!(w.wait_idle(Duration::from_secs(20)));
    (d, w)
}

fn alt_n() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT)
}

#[test]
fn the_command_is_alt_n_and_numbers_are_on_by_default() {
    let c = Cmd::LineNumbers.info();
    assert_eq!((c.label, c.shortcut), ("gui.line_numbers", "Alt+N"));
    assert_eq!(c.key, Some(alt_n()));
    assert_eq!(commands::from_key(alt_n()), Some(Cmd::LineNumbers));
    let (d, mut w) = ws_con(AppConfig::default());
    assert!(w.line_numbers());
    let righe = w.render_lines(40, 10);
    assert!(
        righe[1].starts_with("1│uno") || righe[1].starts_with(" 1│uno"),
        "{righe:?}"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn off_the_margin_is_gone_and_the_text_starts_at_the_edge() {
    let (d, mut w) = ws_con(AppConfig::default());
    w.handle_key(alt_n());
    assert!(!w.line_numbers());
    let righe = w.render_lines(40, 10);
    assert!(righe[1].starts_with("uno"), "{:?}", righe[1]);
    assert!(righe[2].starts_with("due"), "{:?}", righe[2]);
    assert!(!righe.join("\n").contains('│'), "nessun filetto: {righe:?}");
    // il cursore e' sulla prima colonna dello schermo
    assert_eq!(w.active().cursor_xy().map(|(x, _)| x), Some(0));
    // di nuovo: tornano
    w.handle_key(alt_n());
    assert!(w.line_numbers());
    assert!(w.render_lines(40, 10)[1].contains('│'));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_mouse_and_the_cursor_follow_the_new_edge() {
    let (d, mut w) = ws_con(AppConfig::default());
    w.handle_key(alt_n());
    let _ = w.render_lines(40, 10);
    // un clic sulla seconda lettera della seconda riga (colonna 1, riga 2 dello schermo): byte 4 + 1
    w.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 2,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(w.active().cursor(), 5);
    let _ = w.render_lines(40, 10);
    assert_eq!(w.active().cursor_xy(), Some((1, 2)));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn it_applies_to_every_tab_the_new_ones_and_is_saved() {
    let prof = std::env::temp_dir().join(format!("mrp_lineno_prof_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&prof);
    let settings = prof.join("settings.txt");
    let cfg = AppConfig {
        settings_path: Some(settings.clone()),
        ..AppConfig::default()
    };
    let (d, mut w) = ws_con(cfg);
    let altro = d.join("b.txt");
    std::fs::write(&altro, "x\n").unwrap();
    w.open(&altro).unwrap();
    w.handle_key(alt_n());
    assert!(w.tabs().iter().all(|a| !a.line_numbers()));
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("line_numbers=0"), "{scritto}");
    let terzo = d.join("c.txt");
    std::fs::write(&terzo, "y\n").unwrap();
    w.open(&terzo).unwrap();
    assert!(w.tabs().iter().all(|a| !a.line_numbers()), "anche la nuova");
    w.handle_key(alt_n());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(!scritto.contains("line_numbers"), "{scritto}");
    // all'avvio la scelta salvata si rispetta
    let cfg = AppConfig {
        line_numbers: false,
        ..AppConfig::default()
    };
    let (d2, mut w2) = ws_con(cfg);
    assert!(!w2.line_numbers());
    assert!(w2.render_lines(40, 10)[1].starts_with("uno"));
    let _ = std::fs::remove_dir_all(prof);
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_dir_all(d2);
}

#[test]
fn a_dialog_keeps_alt_n_for_itself() {
    let (d, mut w) = ws_con(AppConfig::default());
    w.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
    w.handle_key(alt_n());
    assert!(w.line_numbers());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_setting_is_on_by_default_written_only_when_off_and_read_back() {
    assert!(Settings::default().line_numbers);
    assert!(!Settings::default().to_text().contains("line_numbers"));
    let s = Settings {
        line_numbers: false,
        theme_set: true,
        ..Settings::default()
    };
    let t = s.to_text();
    let dati: Vec<&str> = t.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(dati, vec!["theme=console", "line_numbers=0"]);
    assert_eq!(Settings::from_text(&t), s);
    for no in ["line_numbers=0", "line_numbers = off", "line_numbers=false"] {
        assert!(!Settings::from_text(no).line_numbers, "{no}");
    }
    for si in [
        "line_numbers=1",
        "line_numbers=on",
        "LINE_NUMBERS=0",
        "line_numbers=boh",
    ] {
        assert!(Settings::from_text(si).line_numbers, "{si}");
    }
}
