//! La barra dei salti (E39b): Ctrl+B la mostra o la nasconde sotto la barra delle schede; ha i pulsanti
//! [Home] [-1000] [-100] [-10] [-1] [+1] [+10] [+100] [+1000] [End] (i numeri sono le pagine, dalle impostazioni) e si
//! usa col mouse. Scritti da ARCH: gli attesi sono calcolati a mano. Il file ha 30.000 righe "riga NNNNN\n" di 11 byte;
//! con un terminale alto 24 e la barra dei salti restano 23 righe: 20 di testo, quindi una pagina e' 19 righe.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const RIGA: u64 = 11;
const BARRA: &str = "[Home] [-1000] [-100] [-10] [-1] [+1] [+10] [+100] [+1000] [End]";

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_nav_test_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn testo(righe: usize) -> Vec<u8> {
    (0..righe)
        .flat_map(|i| format!("riga {i:05}\n").into_bytes())
        .collect()
}

fn ws_con(cfg: AppConfig, file: usize) -> (PathBuf, Workspace) {
    let d = cartella();
    let mut nomi = Vec::new();
    for i in 0..file {
        let p = d.join(format!("f{i}.txt"));
        std::fs::write(&p, testo(30_000)).unwrap();
        nomi.push(p);
    }
    let primo = App::open(&nomi[0], cfg.clone()).unwrap();
    let mut ws = Workspace::new(primo, cfg);
    for p in &nomi[1..] {
        ws.open(p).unwrap();
    }
    ws.select(0);
    ws.resize(80, 24);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    (d, ws)
}

fn ws() -> (PathBuf, Workspace) {
    ws_con(AppConfig::default(), 1)
}

fn ctrl_b() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL)
}

fn clic_in(col: u16, riga: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row: riga,
        modifiers: KeyModifiers::NONE,
    }
}

/// La colonna in cui comincia il pulsante `etichetta` nella riga `riga` dello schermo.
fn colonna_di(ws: &mut Workspace, riga: usize, etichetta: &str) -> u16 {
    let righe = ws.render_lines(80, 24);
    righe[riga]
        .find(etichetta)
        .unwrap_or_else(|| panic!("{etichetta} non c'e' in {:?}", righe[riga])) as u16
}

fn pulisci(d: PathBuf) {
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn ctrl_b_shows_the_bar_under_the_tab_bar_and_hides_it_again() {
    let (d, mut ws) = ws();
    assert!(!ws.nav_bar());
    let righe = ws.render_lines(80, 24);
    assert!(
        righe[0].contains("f0.txt"),
        "senza barra: il titolo e' in cima"
    );
    ws.handle_key(ctrl_b());
    assert!(ws.nav_bar());
    let righe = ws.render_lines(80, 24);
    assert_eq!(righe[0], BARRA);
    assert!(righe[1].contains("f0.txt"), "il titolo scende di una riga");
    ws.handle_key(ctrl_b());
    assert!(!ws.nav_bar());
    let righe = ws.render_lines(80, 24);
    assert!(righe[0].contains("f0.txt"));
    pulisci(d);
}

#[test]
fn with_two_tabs_the_jump_bar_goes_below_the_tab_bar() {
    let (d, mut ws) = ws_con(AppConfig::default(), 2);
    ws.handle_key(ctrl_b());
    let righe = ws.render_lines(80, 24);
    assert!(
        righe[0].contains("f0.txt") && righe[0].contains("f1.txt"),
        "{:?}",
        righe[0]
    );
    assert_eq!(righe[1], BARRA);
    assert!(righe[2].contains("f0.txt"));
    // la barra delle schede funziona ancora
    let c = colonna_di(&mut ws, 0, "f1.txt");
    ws.handle_mouse(clic_in(c, 0));
    assert_eq!(ws.active().file_name(), "f1.txt");
    // e un clic sulla barra dei salti agisce sulla scheda attiva: con due barre il testo ha 19 righe, la pagina 18 (+10 pagine = 180 righe)
    let c = colonna_di(&mut ws, 1, "[+10]");
    ws.handle_mouse(clic_in(c + 1, 1));
    assert_eq!(ws.active().top(), 180 * RIGA);
    pulisci(d);
}

#[test]
fn the_buttons_jump_by_pages_of_nineteen_rows() {
    let (d, mut ws) = ws();
    ws.handle_key(ctrl_b());
    let _ = ws.render_lines(80, 24);
    let su = |ws: &mut Workspace, etichetta: &str| {
        let c = colonna_di(ws, 0, etichetta);
        ws.handle_mouse(clic_in(c + 1, 0));
    };
    su(&mut ws, "[+1]");
    assert_eq!(ws.active().top(), 19 * RIGA);
    su(&mut ws, "[+10]");
    assert_eq!(ws.active().top(), (19 + 190) * RIGA);
    su(&mut ws, "[+100]");
    assert_eq!(ws.active().top(), (19 + 190 + 1900) * RIGA);
    su(&mut ws, "[+1000]");
    assert_eq!(ws.active().top(), (19 + 190 + 1900 + 19000) * RIGA);
    su(&mut ws, "[-1]");
    assert_eq!(ws.active().top(), (190 + 1900 + 19000) * RIGA);
    su(&mut ws, "[-10]");
    assert_eq!(ws.active().top(), (1900 + 19000) * RIGA);
    su(&mut ws, "[-100]");
    assert_eq!(ws.active().top(), 19000 * RIGA);
    su(&mut ws, "[-1000]");
    assert_eq!(ws.active().top(), 0);
    pulisci(d);
}

#[test]
fn home_and_end_go_to_the_ends_of_the_file() {
    let (d, mut ws) = ws();
    ws.handle_key(ctrl_b());
    let _ = ws.render_lines(80, 24);
    let c = colonna_di(&mut ws, 0, "[End]");
    ws.handle_mouse(clic_in(c + 2, 0));
    // l'ultima riga (29999) e' l'ultima del testo: 20 righe visibili, la prima e' la 29980
    assert_eq!(ws.active().top(), 29_980 * RIGA);
    let c = colonna_di(&mut ws, 0, "[Home]");
    ws.handle_mouse(clic_in(c + 2, 0));
    assert_eq!((ws.active().top(), ws.active().cursor()), (0, 0));
    pulisci(d);
}

#[test]
fn the_labels_follow_the_configured_multipliers() {
    let cfg = AppConfig {
        jump_pages: [2, 3, 4],
        ..AppConfig::default()
    };
    let (d, mut ws) = ws_con(cfg, 1);
    ws.handle_key(ctrl_b());
    let righe = ws.render_lines(80, 24);
    assert_eq!(
        righe[0],
        "[Home] [-4] [-3] [-2] [-1] [+1] [+2] [+3] [+4] [End]"
    );
    let c = colonna_di(&mut ws, 0, "[+3]");
    ws.handle_mouse(clic_in(c + 1, 0));
    assert_eq!(ws.active().top(), 57 * RIGA);
    pulisci(d);
}

#[test]
fn a_click_between_buttons_or_past_the_bar_does_nothing() {
    let (d, mut ws) = ws();
    ws.handle_key(ctrl_b());
    let _ = ws.render_lines(80, 24);
    let c = colonna_di(&mut ws, 0, "[+1]");
    // lo spazio fra [+1] e [+10]
    ws.handle_mouse(clic_in(c + 4, 0));
    assert_eq!(ws.active().top(), 0);
    // oltre l'ultimo pulsante
    ws.handle_mouse(clic_in(70, 0));
    assert_eq!(ws.active().top(), 0);
    pulisci(d);
}

#[test]
fn a_click_on_the_text_counts_rows_below_the_bar() {
    let (d, mut ws) = ws();
    ws.handle_key(ctrl_b());
    // riga 0 la barra dei salti, riga 1 il titolo, riga 2 la prima riga di testo
    let righe = ws.render_lines(80, 24);
    // il gutter ha un carattere largo 3 byte: si contano le colonne, non i byte
    let gutter = righe[2].split("riga 00000").next().unwrap().chars().count() as u16;
    ws.handle_mouse(clic_in(gutter + 3, 4));
    assert_eq!(ws.active().cursor(), 2 * RIGA + 3, "la terza riga di testo");
    pulisci(d);
}

#[test]
fn a_click_on_the_text_counts_rows_below_both_bars() {
    let (d, mut ws) = ws_con(AppConfig::default(), 2);
    ws.handle_key(ctrl_b());
    // riga 0 le schede, riga 1 i salti, riga 2 il titolo, riga 3 la prima riga di testo
    let righe = ws.render_lines(80, 24);
    let gutter = righe[3].split("riga 00000").next().unwrap().chars().count() as u16;
    ws.handle_mouse(clic_in(gutter + 2, 5));
    assert_eq!(ws.active().cursor(), 2 * RIGA + 2, "la terza riga di testo");
    pulisci(d);
}

#[test]
fn a_narrow_terminal_keeps_only_the_buttons_that_fit_whole() {
    let (d, mut ws) = ws();
    ws.handle_key(ctrl_b());
    ws.resize(30, 24);
    let righe = ws.render_lines(30, 24);
    // [Home] [-1000] [-100] [-10] [-1] = 6+1+7+1+6+1+5+1+4 = 32 > 30: il quinto non entra
    assert_eq!(righe[0], "[Home] [-1000] [-100] [-10]");
    // un clic dove sarebbe il pulsante tagliato non fa niente e non panica
    ws.handle_mouse(clic_in(29, 0));
    assert_eq!(ws.active().top(), 0);
    ws.resize(5, 24);
    let _ = ws.render_lines(5, 24);
    pulisci(d);
}

#[test]
fn a_dialog_keeps_ctrl_b_for_itself() {
    let (d, mut ws) = ws();
    ws.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
    ws.handle_key(ctrl_b());
    assert!(!ws.nav_bar());
    pulisci(d);
}

#[test]
fn the_bar_choice_is_saved_in_the_settings_and_read_back() {
    let prof = std::env::temp_dir().join(format!("mrp_nav_prof_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&prof);
    let settings = prof.join("settings.txt");
    let cfg = AppConfig {
        settings_path: Some(settings.clone()),
        ..AppConfig::default()
    };
    let (d, mut ws) = ws_con(cfg, 1);
    ws.handle_key(ctrl_b());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("nav_bar=1"), "{scritto}");
    ws.handle_key(ctrl_b());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(!scritto.contains("nav_bar"), "{scritto}");
    // all'avvio la scelta salvata si rispetta
    let cfg = AppConfig {
        nav_bar: true,
        ..AppConfig::default()
    };
    let (d2, mut ws2) = ws_con(cfg, 1);
    assert!(ws2.nav_bar());
    assert_eq!(ws2.render_lines(80, 24)[0], BARRA);
    let _ = std::fs::remove_dir_all(prof);
    pulisci(d);
    pulisci(d2);
}

#[test]
fn a_new_tab_inherits_the_bar_and_a_theme_change_keeps_it_saved() {
    let prof = std::env::temp_dir().join(format!("mrp_nav_prof2_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&prof);
    let settings = prof.join("settings.txt");
    let cfg = AppConfig {
        settings_path: Some(settings.clone()),
        ..AppConfig::default()
    };
    let (d, mut ws) = ws_con(cfg, 1);
    ws.handle_key(ctrl_b());
    let altro = d.join("altro.txt");
    std::fs::write(&altro, testo(100)).unwrap();
    ws.open(&altro).unwrap();
    let righe = ws.render_lines(80, 24);
    assert_eq!(
        righe[1], BARRA,
        "con due schede la barra dei salti e' la seconda riga"
    );
    // cambiare tema riscrive le impostazioni senza perdere la barra
    ws.handle_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
    ws.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    ws.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("nav_bar=1"), "{scritto}");
    let _ = std::fs::remove_dir_all(prof);
    pulisci(d);
}

#[test]
fn a_button_that_fits_exactly_is_drawn() {
    let (d, mut ws) = ws();
    ws.handle_key(ctrl_b());
    // [Home] [-1000] [-100] [-10] = 27 colonne esatte
    ws.resize(27, 24);
    let righe = ws.render_lines(27, 24);
    assert_eq!(righe[0], "[Home] [-1000] [-100] [-10]");
    let c = righe[0].find("[-10]").unwrap() as u16;
    ws.handle_mouse(clic_in(c + 4, 0));
    assert_eq!(ws.active().top(), 0, "prima dell'inizio: resta a 0");
    ws.resize(80, 24);
    let _ = ws.render_lines(80, 24);
    pulisci(d);
}
