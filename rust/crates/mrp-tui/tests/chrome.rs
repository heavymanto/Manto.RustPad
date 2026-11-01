//! La modalita' senza cornice (B2a): l'`App` disegna solo il testo (niente titolo, riga di stato e tasti: li disegna la
//! finestra), espone lo stato (`status`) e la posizione per la scrollbar (`scroll_info`, `scroll_to_fraction`). Scritti da
//! ARCH: gli attesi sono calcolati a mano. Il file ha 100 righe di 10 byte ("l00000000\n"...): 1000 byte, gutter di 5 celle.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn dati() -> Vec<u8> {
    (0..100)
        .flat_map(|i| format!("l{i:08}\n").into_bytes())
        .collect()
}

fn file() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_chrome_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, dati()).unwrap();
    p
}

fn cfg(chrome: bool) -> AppConfig {
    AppConfig {
        chrome,
        ..AppConfig::default()
    }
}

fn app(chrome: bool, w: u16, h: u16) -> (App, PathBuf) {
    let p = file();
    let mut a = App::open(&p, cfg(chrome)).unwrap();
    a.resize(w, h);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let _ = render_lines(&mut a, w, h);
    (a, p)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn clic(col: u16, riga: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row: riga,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn the_default_is_the_terminal_look() {
    assert!(AppConfig::default().chrome);
    let (a, p) = app(true, 40, 6);
    assert!(a.chrome());
    let _ = std::fs::remove_file(p);
}

#[test]
fn without_chrome_every_row_of_the_screen_is_text() {
    let (mut a, p) = app(false, 40, 6);
    let righe = render_lines(&mut a, 40, 6);
    assert_eq!(righe.len(), 6);
    assert_eq!(righe[0], "   1│l00000000");
    assert_eq!(righe[5], "   6│l00000005");
    let tutto = righe.join("\n");
    assert!(
        !tutto.contains("VIEW") && !tutto.contains("F1") && !tutto.contains("Ln "),
        "{tutto}"
    );
    // col disegno col terminale: titolo, 3 righe di testo, stato, tasti
    let (mut b, q) = app(true, 40, 6);
    let con = render_lines(&mut b, 40, 6);
    assert!(
        con[0].contains("mrp_chrome_test"),
        "il titolo e' il nome del file: {:?}",
        con[0]
    );
    assert_eq!(con[1], "   1│l00000000");
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn without_chrome_the_cursor_and_the_clicks_start_at_row_zero() {
    let (mut a, p) = app(false, 40, 6);
    assert_eq!(a.cursor_xy(), Some((5, 0)));
    a.handle_mouse(clic(5 + 3, 0));
    assert_eq!(a.cursor(), 3, "la prima riga e' la riga 0 dello schermo");
    a.handle_mouse(clic(5, 2));
    assert_eq!(a.cursor(), 20);
    let _ = render_lines(&mut a, 40, 6);
    assert_eq!(a.cursor_xy(), Some((5, 2)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn without_chrome_a_page_is_the_whole_height_minus_one_row() {
    let (mut a, p) = app(false, 40, 6);
    a.handle_key(k(KeyCode::PageDown));
    // 6 righe di testo: una pagina e' 5 righe, cioe' 50 byte
    assert_eq!((a.top(), a.cursor()), (50, 50));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_config_reaches_the_new_tabs_of_a_workspace() {
    let p = file();
    let c = cfg(false);
    let mut ws = Workspace::new(App::open(&p, c.clone()).unwrap(), c);
    ws.resize(40, 8);
    assert!(!ws.active().chrome());
    let q = file();
    ws.open(&q).unwrap();
    assert_eq!(ws.len(), 2);
    assert!(!ws.active().chrome());
    ws.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert_eq!(ws.len(), 3);
    assert!(!ws.active().chrome());
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

// ----- lo stato -----

#[test]
fn the_status_says_what_the_footer_would() {
    let (mut a, p) = app(false, 40, 6);
    let s = a.status();
    assert_eq!(s.size, 1000);
    assert_eq!((s.line, s.column, s.offset, s.percent), (Some(1), 1, 0, 0));
    assert!(!s.edit && !s.modified && !s.column_mode && !s.untitled);
    assert_eq!(s.encoding, a.encoding().name());
    assert_eq!(s.line_ending, "LF");
    assert!(s.flash.is_none() && s.busy.is_none());
    assert_eq!(s.file_name, a.file_name());
    // due righe giu' e tre caratteri a destra: riga 3, colonna 4, byte 23, 2%
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Down));
    for _ in 0..3 {
        a.handle_key(k(KeyCode::Right));
    }
    let s = a.status();
    assert_eq!((s.line, s.column, s.offset, s.percent), (Some(3), 4, 23, 2));
    // la modalita' di modifica e la modifica
    a.handle_key(k(KeyCode::F(4)));
    a.handle_key(k(KeyCode::Char('x')));
    let s = a.status();
    assert!(s.edit && s.modified);
    assert_eq!(s.size, 1001);
    a.handle_key(k(KeyCode::F(8)));
    assert!(a.status().column_mode);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_status_carries_the_flash_and_the_untitled_name() {
    let (mut a, p) = app(false, 40, 6);
    a.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    let s = a.status();
    assert!(s.flash.is_some_and(|f| !f.is_empty()), "niente da salvare");
    let d = std::env::temp_dir().join(format!("mrp_chrome_untitled_{}", std::process::id()));
    let n = App::new_untitled(&d, 1, cfg(false)).unwrap();
    let mut n = n;
    let s = n.status();
    assert!(s.untitled && s.edit && !s.modified);
    assert_eq!(s.file_name, "Untitled 1");
    assert_eq!(s.size, 0);
    drop(n);
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_status_names_the_line_ending() {
    let casi: [(&[u8], &str); 3] = [
        (b"a\r\nb\r\n", "CRLF"),
        (b"a\nb\n", "LF"),
        (b"a\rb\r", "CR"),
    ];
    for (dati, atteso) in casi {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_chrome_eol_{}_{}.txt", std::process::id(), n));
        std::fs::write(&p, dati).unwrap();
        let mut a = App::open(&p, cfg(false)).unwrap();
        a.resize(40, 6);
        assert!(a.wait_idle(Duration::from_secs(20)));
        assert_eq!(a.status().line_ending, atteso, "{dati:?}");
        let _ = std::fs::remove_file(p);
    }
}

// ----- la scrollbar -----

fn vicini(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn scroll_info_is_where_the_window_is_in_the_file() {
    let (mut a, p) = app(false, 40, 5);
    let s = a.scroll_info();
    // 5 righe visibili di 10 byte su 1000: dall'inizio, un ventesimo
    assert!(vicini(s.start, 0.0), "{s:?}");
    assert!(vicini(s.size, 0.05), "{s:?}");
    a.scroll_to_fraction(0.3);
    let _ = render_lines(&mut a, 40, 5);
    let s = a.scroll_info();
    assert!(vicini(s.start, 0.3), "{s:?}");
    assert!(vicini(s.size, 0.05), "{s:?}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn scroll_to_fraction_puts_the_top_on_the_row_that_holds_that_byte_and_clamps() {
    let (mut a, p) = app(false, 40, 5);
    a.scroll_to_fraction(0.5);
    assert_eq!(a.top(), 500);
    a.scroll_to_fraction(0.505);
    assert_eq!(
        a.top(),
        500,
        "il byte 505 sta nella riga che comincia a 500"
    );
    a.scroll_to_fraction(0.0);
    assert_eq!(a.top(), 0);
    // in fondo: l'ultima pagina (l'ultima riga e' la 99, che comincia a 990: 5 righe visibili, la prima a 950)
    a.scroll_to_fraction(1.0);
    assert_eq!(a.top(), 950);
    a.scroll_to_fraction(7.0);
    assert_eq!(a.top(), 950);
    a.scroll_to_fraction(-3.0);
    assert_eq!(a.top(), 0);
    a.scroll_to_fraction(f64::NAN);
    assert_eq!(a.top(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn dragging_the_scrollbar_keeps_the_cursor_inside_the_window() {
    let (mut a, p) = app(false, 40, 5);
    a.scroll_to_fraction(0.5);
    let c = a.cursor();
    assert!((500..=540).contains(&c), "{c}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_file_that_fits_in_the_window_fills_the_bar() {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_chrome_small_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, b"uno\ndue\n").unwrap();
    let mut a = App::open(&p, cfg(false)).unwrap();
    a.resize(40, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let _ = render_lines(&mut a, 40, 10);
    let s = a.scroll_info();
    assert!(vicini(s.start, 0.0) && vicini(s.size, 1.0), "{s:?}");
    a.scroll_to_fraction(0.9);
    assert_eq!(a.top(), 0, "niente da scorrere");
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_empty_document_has_a_full_bar_and_scrolls_nowhere() {
    let d = std::env::temp_dir().join(format!("mrp_chrome_empty_{}", std::process::id()));
    let mut a = App::new_untitled(&d, 1, cfg(false)).unwrap();
    a.resize(40, 5);
    let _ = render_lines(&mut a, 40, 5);
    let s = a.scroll_info();
    assert!(vicini(s.start, 0.0) && vicini(s.size, 1.0), "{s:?}");
    a.scroll_to_fraction(0.5);
    assert_eq!(a.top(), 0);
    drop(a);
    let _ = std::fs::remove_dir_all(d);
}
