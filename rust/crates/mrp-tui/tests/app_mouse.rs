//! Il mouse nel TUI/GUI (E32): il clic con il tasto sinistro porta il cursore al carattere sotto il puntatore, Maiusc+clic e
//! il trascinamento selezionano, la rotella scorre. Scritti da ARCH: gli attesi sono calcolati a mano sui byte di prova.
//! La colonna dello schermo = larghezza del gutter + colonna del testo; il gutter si ricava dal cursore (colonna 0).

use mrp_core::encoding::Encoding;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn app(data: &[u8], enc: Option<Encoding>, edit: bool) -> (App, PathBuf, u16) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_mouse_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, data).unwrap();
    let cfg = AppConfig {
        encoding: enc,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(40, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    if edit {
        a.handle_key(k(KeyCode::F(4)));
    }
    // un disegno: serve ai clic (ricordano le righe disegnate); il gutter e' la colonna del cursore a 0
    let _ = render_lines(&mut a, 40, 10);
    let (gutter, _) = a.cursor_xy().expect("cursore");
    (a, p, gutter)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn mouse(kind: MouseEventKind, col: u16, row: u16, mods: KeyModifiers) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: mods,
    }
}

fn clic(a: &mut App, col: u16, row: u16) {
    a.handle_mouse(mouse(
        MouseEventKind::Down(MouseButton::Left),
        col,
        row,
        KeyModifiers::NONE,
    ));
    a.handle_mouse(mouse(
        MouseEventKind::Up(MouseButton::Left),
        col,
        row,
        KeyModifiers::NONE,
    ));
    let _ = render_lines(a, 40, 10);
}

#[test]
fn a_click_puts_the_cursor_on_the_character() {
    // "abc\ndef\n": 8 byte, gutter di 2 celle; 'a' a 0, 'f' a 6
    let (mut a, p, g) = app(b"abc\ndef\n", None, false);
    assert_eq!(g, 2);
    clic(&mut a, g + 1, 1);
    assert_eq!(a.cursor(), 1);
    clic(&mut a, g + 2, 2);
    assert_eq!(a.cursor(), 6);
    clic(&mut a, g, 2);
    assert_eq!(a.cursor(), 4);
    // oltre la fine della riga: la fine della riga (sul suo a-capo)
    clic(&mut a, g + 20, 1);
    assert_eq!(a.cursor(), 3);
    // nel gutter: l'inizio della riga
    clic(&mut a, 0, 2);
    assert_eq!(a.cursor(), 4);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_outside_the_text_does_nothing() {
    let (mut a, p, g) = app(b"abc\ndef\n", None, false);
    clic(&mut a, g + 1, 1);
    assert_eq!(a.cursor(), 1);
    // il titolo (riga 0), sotto l'ultima riga di testo e la riga di stato
    clic(&mut a, g + 2, 0);
    assert_eq!(a.cursor(), 1);
    clic(&mut a, g + 2, 6);
    assert_eq!(a.cursor(), 1);
    clic(&mut a, g + 2, 8);
    assert_eq!(a.cursor(), 1);
    let _ = std::fs::remove_file(p);
}

#[test]
fn wide_characters_are_clicked_by_their_cells() {
    // 日 occupa le colonne 0-1, x la 2; i byte: 日 0..3, x 3, a-capo 4
    let (mut a, p, g) = app("日x\n".as_bytes(), None, false);
    clic(&mut a, g, 1);
    assert_eq!(a.cursor(), 0);
    clic(&mut a, g + 1, 1);
    assert_eq!(
        a.cursor(),
        0,
        "la seconda cella di un carattere largo e' il carattere"
    );
    clic(&mut a, g + 2, 1);
    assert_eq!(a.cursor(), 3);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_in_a_multibyte_encoding_lands_on_a_character_start() {
    let sj = Encoding::by_name("shift-jis").unwrap();
    // "日本x\n": 日 0..2, 本 2..4, x 4; colonne 0-1, 2-3, 4
    let dati = sj.encode_str("日本x\n").unwrap();
    let (mut a, p, g) = app(&dati, Some(sj), false);
    clic(&mut a, g + 3, 1);
    assert_eq!(a.cursor(), 2);
    clic(&mut a, g + 4, 1);
    assert_eq!(a.cursor(), 4);
    let _ = std::fs::remove_file(p);
}

#[test]
fn dragging_selects_and_shift_click_extends() {
    let (mut a, p, g) = app(b"abc\ndef\n", None, false);
    a.handle_mouse(mouse(
        MouseEventKind::Down(MouseButton::Left),
        g,
        1,
        KeyModifiers::NONE,
    ));
    a.handle_mouse(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        g + 2,
        2,
        KeyModifiers::NONE,
    ));
    assert_eq!(a.selection(), Some((0, 6)));
    // finito il trascinamento un nuovo clic toglie la selezione
    a.handle_mouse(mouse(
        MouseEventKind::Up(MouseButton::Left),
        g + 2,
        2,
        KeyModifiers::NONE,
    ));
    let _ = render_lines(&mut a, 40, 10);
    clic(&mut a, g + 1, 1);
    assert_eq!(a.selection(), None);
    assert_eq!(a.cursor(), 1);
    // Maiusc+clic: dal cursore al punto
    a.handle_mouse(mouse(
        MouseEventKind::Down(MouseButton::Left),
        g + 1,
        2,
        KeyModifiers::SHIFT,
    ));
    assert_eq!(a.selection(), Some((1, 5)));
    // trascinare oltre l'ultima riga arriva all'ultima
    clic(&mut a, g, 1);
    a.handle_mouse(mouse(
        MouseEventKind::Down(MouseButton::Left),
        g,
        1,
        KeyModifiers::NONE,
    ));
    a.handle_mouse(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        g + 1,
        9,
        KeyModifiers::NONE,
    ));
    assert_eq!(a.selection(), Some((0, 5)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_ignores_dialogs_and_column_mode() {
    let (mut a, p, g) = app(b"abc\ndef\n", None, false);
    a.handle_key(k(KeyCode::F(1)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Help));
    clic(&mut a, g + 2, 2);
    assert_eq!(a.cursor(), 0);
    a.handle_key(k(KeyCode::Esc));
    a.handle_key(k(KeyCode::F(8)));
    assert!(a.column_mode());
    clic(&mut a, g + 2, 2);
    assert_eq!(a.cursor(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn typing_after_a_click_writes_there() {
    let (mut a, p, g) = app(b"abc\ndef\n", None, true);
    clic(&mut a, g + 2, 2);
    a.handle_key(k(KeyCode::Char('X')));
    assert_eq!(a.document_text(), b"abc\ndeXf\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_after_scrolling_uses_the_rows_on_screen() {
    let testo: String = (0..100).map(|i| format!("riga {i:02}\n")).collect();
    let (mut a, p, g) = app(testo.as_bytes(), None, false);
    for _ in 0..4 {
        a.handle_mouse(mouse(MouseEventKind::ScrollDown, 0, 0, KeyModifiers::NONE));
    }
    let _ = render_lines(&mut a, 40, 10);
    let top = a.top();
    assert!(top > 0);
    clic(&mut a, g, 1);
    assert_eq!(a.cursor(), top, "la prima riga a schermo");
    clic(&mut a, g + 3, 3);
    assert_eq!(
        a.cursor(),
        top + 2 * 8 + 3,
        "la terza riga, quarto carattere"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_wheel_still_scrolls() {
    let testo: String = (0..100).map(|i| format!("riga {i:02}\n")).collect();
    let (mut a, p, _g) = app(testo.as_bytes(), None, false);
    let prima = a.top();
    a.handle_mouse(mouse(MouseEventKind::ScrollDown, 5, 5, KeyModifiers::NONE));
    assert!(a.top() > prima);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_on_a_horizontally_scrolled_line_counts_the_hidden_columns() {
    // una riga di 100 cifre che si ripetono: 0123456789...; dopo End la vista e' scorsa a destra
    let riga: String = (0..100)
        .map(|i| char::from(b'0' + (i % 10) as u8))
        .collect();
    let (mut a, p, g) = app(format!("{riga}\n").as_bytes(), None, false);
    a.handle_key(k(KeyCode::End));
    let _ = render_lines(&mut a, 40, 10);
    let h = a.hscroll();
    assert!(h > 0, "la vista e' scorsa");
    clic(&mut a, g + 3, 1);
    assert_eq!(a.cursor(), (h + 3) as u64);
    let _ = std::fs::remove_file(p);
}
