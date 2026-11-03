//! Il registro dei comandi (B1): un elenco unico con nome (chiave del catalogo), tasto, scorciatoia scritta e icona, da cui
//! nascono il menu e (poi) la barra degli strumenti della finestra. Scritti da ARCH: gli attesi sono calcolati a mano.

use mrp_core::lang::Lang;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::{self, Cmd, COMMANDS};
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_cmd_test_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un `Workspace` con una scheda su un file di `righe` righe, la sessione propria (le cartelle `untitled` non si mescolano).
fn ws_con_file(d: &std::path::Path, testo: &str) -> Workspace {
    let f = d.join("uno.txt");
    std::fs::write(&f, testo).unwrap();
    let cfg = AppConfig::default();
    let mut ws = Workspace::new(App::open(&f, cfg.clone()).unwrap(), cfg).with_session(d.join("s"));
    ws.resize(70, 12);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    ws
}

fn k(c: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(c, m)
}

const CTRL: KeyModifiers = KeyModifiers::CONTROL;
const NIENTE: KeyModifiers = KeyModifiers::NONE;
const ALT: KeyModifiers = KeyModifiers::ALT;

// ----- la tabella -----

#[test]
fn every_command_has_a_text_in_the_catalog_a_unique_key_and_a_shortcut_that_says_the_key() {
    let en = Lang::english();
    let mut tasti = Vec::new();
    let mut nomi = Vec::new();
    let mut id: Vec<Cmd> = Vec::new();
    for c in COMMANDS {
        assert!(en.get(c.label).is_some(), "{} non e' nel catalogo", c.label);
        assert!(!nomi.contains(&c.label), "{} due volte", c.label);
        nomi.push(c.label);
        assert!(!id.contains(&c.cmd), "{:?} due volte", c.cmd);
        id.push(c.cmd);
        assert_eq!(c.cmd.info().cmd, c.cmd);
        assert!(!c.icon.is_empty(), "{:?} senza icona", c.cmd);
        if let Some(t) = c.key {
            assert_eq!(c.shortcut, commands::shortcut_text(t), "{:?}", c.cmd);
            assert!(!tasti.contains(&t), "{t:?} serve a due comandi");
            tasti.push(t);
            assert_eq!(commands::from_key(t), Some(c.cmd), "{t:?}");
        }
    }
}

#[test]
fn shortcut_text_is_written_by_hand_for_known_keys() {
    let casi = [
        (k(KeyCode::Char('n'), CTRL), "Ctrl+N"),
        (
            k(KeyCode::Char('S'), CTRL | KeyModifiers::SHIFT),
            "Ctrl+Shift+S",
        ),
        (k(KeyCode::F(2), NIENTE), "F2"),
        (k(KeyCode::F(10), NIENTE), "F10"),
        (k(KeyCode::PageDown, CTRL), "Ctrl+PgDn"),
        (k(KeyCode::PageUp, CTRL), "Ctrl+PgUp"),
        (k(KeyCode::PageUp, KeyModifiers::ALT), "Alt+PgUp"),
        (k(KeyCode::Home, CTRL), "Ctrl+Home"),
        (
            k(KeyCode::Char('b'), CTRL | KeyModifiers::ALT),
            "Ctrl+Alt+B",
        ),
    ];
    for (tasto, atteso) in casi {
        assert_eq!(commands::shortcut_text(tasto), atteso);
    }
}

#[test]
fn the_known_commands_have_the_known_keys() {
    let attesi = [
        (Cmd::New, Some(k(KeyCode::Char('n'), CTRL)), "Ctrl+N"),
        (Cmd::Open, Some(k(KeyCode::Char('o'), CTRL)), "Ctrl+O"),
        (Cmd::Save, Some(k(KeyCode::F(2), NIENTE)), "F2"),
        (
            Cmd::SaveAs,
            Some(k(KeyCode::Char('S'), CTRL | KeyModifiers::SHIFT)),
            "Ctrl+Shift+S",
        ),
        (Cmd::CloseTab, Some(k(KeyCode::Char('w'), CTRL)), "Ctrl+W"),
        (Cmd::Quit, Some(k(KeyCode::F(10), NIENTE)), "F10"),
        (Cmd::EditMode, Some(k(KeyCode::F(4), NIENTE)), "F4"),
        (Cmd::Undo, Some(k(KeyCode::Char('z'), CTRL)), "Ctrl+Z"),
        (Cmd::Redo, Some(k(KeyCode::Char('y'), CTRL)), "Ctrl+Y"),
        (Cmd::Cut, Some(k(KeyCode::Char('x'), CTRL)), "Ctrl+X"),
        (Cmd::Copy, Some(k(KeyCode::Char('c'), CTRL)), "Ctrl+C"),
        (Cmd::Paste, Some(k(KeyCode::Char('v'), CTRL)), "Ctrl+V"),
        (Cmd::SelectAll, Some(k(KeyCode::Char('a'), CTRL)), "Ctrl+A"),
        (Cmd::Find, Some(k(KeyCode::Char('f'), CTRL)), "Ctrl+F"),
        (Cmd::Goto, Some(k(KeyCode::Char('g'), CTRL)), "Ctrl+G"),
        (Cmd::Replace, Some(k(KeyCode::Char('h'), CTRL)), "Ctrl+H"),
        (Cmd::Filter, Some(k(KeyCode::Char('g'), ALT)), "Alt+G"),
        (Cmd::ColumnMode, Some(k(KeyCode::F(8), NIENTE)), "F8"),
        (Cmd::Special, Some(k(KeyCode::Char('w'), ALT)), "Alt+W"),
        (Cmd::LineNumbers, Some(k(KeyCode::Char('n'), ALT)), "Alt+N"),
        (Cmd::Follow, Some(k(KeyCode::Char('f'), ALT)), "Alt+F"),
        (Cmd::LogColors, Some(k(KeyCode::Char('l'), ALT)), "Alt+L"),
        (Cmd::NextTab, Some(k(KeyCode::PageDown, CTRL)), "Ctrl+PgDn"),
        (Cmd::PrevTab, Some(k(KeyCode::PageUp, CTRL)), "Ctrl+PgUp"),
        (Cmd::NavBar, Some(k(KeyCode::Char('b'), CTRL)), "Ctrl+B"),
        (Cmd::Theme, Some(k(KeyCode::F(6), NIENTE)), "F6"),
        (Cmd::Language, Some(k(KeyCode::F(7), NIENTE)), "F7"),
        (Cmd::Encoding, Some(k(KeyCode::F(9), NIENTE)), "F9"),
        (Cmd::Help, Some(k(KeyCode::F(1), NIENTE)), "F1"),
        (Cmd::Home, Some(k(KeyCode::Home, CTRL)), "Ctrl+Home"),
        (Cmd::End, Some(k(KeyCode::End, CTRL)), "Ctrl+End"),
        // lo zoom e' della finestra: nessun tasto dell'App
        (Cmd::ZoomIn, None, "Ctrl++"),
        (Cmd::ZoomOut, None, "Ctrl+-"),
        (Cmd::ZoomReset, None, "Ctrl+0"),
        (Cmd::Settings, None, "Ctrl+,"),
        // il tempo: l'intervallo e il formato dell'orario hanno un tasto; il filtro per data (acceso / spento) e' solo del menu e della barra
        (Cmd::TimeRange, Some(k(KeyCode::Char('t'), ALT)), "Alt+T"),
        (Cmd::TimeMask, Some(k(KeyCode::Char('o'), ALT)), "Alt+O"),
        (Cmd::TimeFilter, None, ""),
        (Cmd::LineIndex, Some(k(KeyCode::Char('x'), ALT)), "Alt+X"),
    ];
    assert_eq!(attesi.len(), COMMANDS.len(), "un comando in piu' o in meno");
    for (cmd, tasto, scritta) in attesi {
        let i = cmd.info();
        assert_eq!((i.key, i.shortcut), (tasto, scritta), "{cmd:?}");
    }
}

#[test]
fn a_key_that_is_not_a_command_maps_to_nothing() {
    assert_eq!(commands::from_key(k(KeyCode::Char('q'), NIENTE)), None);
    assert_eq!(commands::from_key(k(KeyCode::PageDown, NIENTE)), None);
    assert_eq!(
        commands::from_key(k(KeyCode::F(3), NIENTE)),
        None,
        "F3 e' 'trova il prossimo', non un comando del menu"
    );
}

// ----- quando un comando e' attivo -----

#[test]
fn save_is_enabled_only_when_there_is_something_to_save() {
    let d = cartella();
    let mut ws = ws_con_file(&d, "uno\n");
    assert!(!commands::enabled(&ws, Cmd::Save));
    ws.handle_key(k(KeyCode::F(4), NIENTE));
    ws.handle_key(k(KeyCode::Char('x'), NIENTE));
    assert!(commands::enabled(&ws, Cmd::Save));
    ws.handle_key(k(KeyCode::F(2), NIENTE));
    assert!(ws.wait_idle(Duration::from_secs(20)));
    assert!(
        !commands::enabled(&ws, Cmd::Save),
        "salvato: niente da salvare"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn undo_and_redo_follow_the_history_and_need_edit_mode() {
    let d = cartella();
    let mut ws = ws_con_file(&d, "uno\n");
    assert!(!commands::enabled(&ws, Cmd::Undo) && !commands::enabled(&ws, Cmd::Redo));
    ws.handle_key(k(KeyCode::F(4), NIENTE));
    ws.handle_key(k(KeyCode::Char('x'), NIENTE));
    assert!(commands::enabled(&ws, Cmd::Undo));
    assert!(!commands::enabled(&ws, Cmd::Redo));
    ws.handle_key(k(KeyCode::Char('z'), CTRL));
    assert!(!commands::enabled(&ws, Cmd::Undo));
    assert!(commands::enabled(&ws, Cmd::Redo));
    // in lettura la storia non si usa
    ws.handle_key(k(KeyCode::F(4), NIENTE));
    assert!(!commands::enabled(&ws, Cmd::Redo));
    ws.handle_key(k(KeyCode::F(4), NIENTE));
    ws.handle_key(k(KeyCode::Char('y'), NIENTE));
    assert!(commands::enabled(&ws, Cmd::Undo));
    ws.handle_key(k(KeyCode::F(4), NIENTE));
    assert!(
        !commands::enabled(&ws, Cmd::Undo),
        "in lettura non si annulla"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn copy_needs_a_selection_and_cut_and_paste_need_edit_mode() {
    let d = cartella();
    let mut ws = ws_con_file(&d, "uno due\n");
    assert!(!commands::enabled(&ws, Cmd::Copy));
    assert!(!commands::enabled(&ws, Cmd::Cut));
    assert!(
        !commands::enabled(&ws, Cmd::Paste),
        "in lettura non si incolla"
    );
    ws.handle_key(k(KeyCode::Right, KeyModifiers::SHIFT));
    assert!(commands::enabled(&ws, Cmd::Copy), "con una selezione");
    assert!(
        !commands::enabled(&ws, Cmd::Cut),
        "ma in lettura non si taglia"
    );
    ws.handle_key(k(KeyCode::F(4), NIENTE));
    assert!(commands::enabled(&ws, Cmd::Cut));
    assert!(commands::enabled(&ws, Cmd::Paste));
    // senza selezione taglia e copia non hanno niente da fare, incolla si
    ws.handle_key(k(KeyCode::Esc, NIENTE));
    assert!(!commands::enabled(&ws, Cmd::Cut));
    assert!(!commands::enabled(&ws, Cmd::Copy));
    assert!(commands::enabled(&ws, Cmd::Paste));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_tab_commands_need_more_than_one_tab() {
    let d = cartella();
    let mut ws = ws_con_file(&d, "uno\n");
    assert!(!commands::enabled(&ws, Cmd::NextTab) && !commands::enabled(&ws, Cmd::PrevTab));
    ws.exec(Cmd::New);
    assert!(commands::enabled(&ws, Cmd::NextTab) && commands::enabled(&ws, Cmd::PrevTab));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_other_commands_are_always_enabled() {
    let d = cartella();
    let ws = ws_con_file(&d, "uno\n");
    for c in [
        Cmd::New,
        Cmd::Open,
        Cmd::SaveAs,
        Cmd::CloseTab,
        Cmd::Quit,
        Cmd::EditMode,
        Cmd::SelectAll,
        Cmd::Find,
        Cmd::Goto,
        Cmd::Replace,
        Cmd::Filter,
        Cmd::ColumnMode,
        Cmd::Special,
        Cmd::LineNumbers,
        Cmd::Follow,
        Cmd::LogColors,
        Cmd::NavBar,
        Cmd::Theme,
        Cmd::Language,
        Cmd::Encoding,
        Cmd::Help,
        Cmd::Home,
        Cmd::End,
        Cmd::ZoomIn,
        Cmd::ZoomOut,
        Cmd::ZoomReset,
    ] {
        assert!(commands::enabled(&ws, c), "{c:?}");
    }
    let _ = std::fs::remove_dir_all(d);
}

// ----- eseguire un comando -----

#[test]
fn exec_does_what_the_key_does() {
    let d = cartella();
    let mut ws = ws_con_file(&d, "uno\n");
    assert!(ws.exec(Cmd::New));
    assert_eq!(ws.len(), 2);
    assert!(ws.exec(Cmd::PrevTab));
    assert_eq!(ws.active_index(), 0);
    assert!(ws.exec(Cmd::NextTab));
    assert_eq!(ws.active_index(), 1);
    assert!(ws.exec(Cmd::NavBar));
    assert!(ws.nav_bar());
    assert!(ws.exec(Cmd::EditMode));
    assert!(
        !ws.active().is_edit(),
        "la scheda senza nome era in modifica: ora in lettura"
    );
    assert!(ws.exec(Cmd::EditMode));
    assert!(ws.active().is_edit());
    assert!(ws.exec(Cmd::ColumnMode));
    assert!(ws.active().column_mode());
    assert!(ws.exec(Cmd::ColumnMode));
    assert!(ws.exec(Cmd::Find));
    assert_eq!(
        ws.active().dialog_kind(),
        Some(mrp_tui::app::DialogKind::Find)
    );
    ws.handle_key(k(KeyCode::Esc, NIENTE));
    assert!(ws.exec(Cmd::Goto));
    assert_eq!(
        ws.active().dialog_kind(),
        Some(mrp_tui::app::DialogKind::Goto)
    );
    ws.handle_key(k(KeyCode::Esc, NIENTE));
    assert!(ws.exec(Cmd::Help));
    assert_eq!(
        ws.active().dialog_kind(),
        Some(mrp_tui::app::DialogKind::Help)
    );
    ws.handle_key(k(KeyCode::Esc, NIENTE));
    assert!(ws.exec(Cmd::CloseTab));
    assert_eq!(
        ws.len(),
        1,
        "la scheda senza nome era intatta: si chiude subito"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn exec_leaves_the_window_only_commands_to_the_window() {
    let d = cartella();
    let mut ws = ws_con_file(&d, "uno\n");
    for c in [Cmd::ZoomIn, Cmd::ZoomOut, Cmd::ZoomReset] {
        assert!(!ws.exec(c), "{c:?} non ha un tasto dell'App");
    }
    assert_eq!(ws.len(), 1);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn exec_does_not_run_a_disabled_command() {
    let d = cartella();
    let mut ws = ws_con_file(&d, "uno\n");
    // Salva senza modifiche e Annulla in lettura: niente da fare, e non si sporca la riga di stato con "niente da salvare"
    assert!(!ws.exec(Cmd::Save));
    assert!(ws.active().flash().is_none());
    assert!(!ws.exec(Cmd::Undo));
    assert!(!ws.exec(Cmd::NextTab));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn exec_does_nothing_while_a_dialog_is_open() {
    let d = cartella();
    let mut ws = ws_con_file(
        &d, "uno
",
    );
    ws.exec(Cmd::New);
    ws.exec(Cmd::PrevTab);
    assert_eq!(ws.active_index(), 0);
    assert!(ws.exec(Cmd::Help));
    assert!(!ws.exec(Cmd::NextTab), "il dialogo tiene i tasti");
    assert_eq!(ws.active_index(), 0);
    let _ = std::fs::remove_dir_all(d);
}
