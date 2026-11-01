//! La selezione a colonna col mouse (fase 2, 1.3): Alt+clic e trascinamento disegnano il blocco rettangolare (e accendono la
//! modalita' colonna); in modalita' colonna basta trascinare. Il rilascio chiude il gesto; i tasti (Maiusc+frecce) riprendono
//! il blocco da dove sta. Scritti da ARCH: gli attesi sono calcolati a mano. Il testo ha 4 righe di 7 byte
//! ("abcdef\n", "ghijkl\n", "mnopqr\n", "stuvwx\n"), il terminale e' 40 x 10 (la riga 0 e' il titolo, il testo parte dalla 1)
//! e il gutter e' di 3 celle (le cifre della dimensione del file piu' uno).

use mrp_core::block::ColumnBlock;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const TESTO: &[u8] = b"abcdef\nghijkl\nmnopqr\nstuvwx\n";
const G: u16 = 3;

fn app() -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_blockmouse_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, TESTO).unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let _ = render_lines(&mut a, 40, 10);
    assert_eq!(a.cursor_xy().unwrap().0, G, "il gutter");
    (a, p)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn ev(kind: MouseEventKind, col: u16, row: u16, mods: KeyModifiers) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: mods,
    }
}

fn giu(a: &mut App, col: u16, riga: u16, mods: KeyModifiers) {
    a.handle_mouse(ev(MouseEventKind::Down(MouseButton::Left), col, riga, mods));
    let _ = render_lines(a, 40, 10);
}

/// Un trascinamento: i terminali e la finestra mandano i `Drag` senza Alt.
fn trascina(a: &mut App, col: u16, riga: u16) {
    a.handle_mouse(ev(
        MouseEventKind::Drag(MouseButton::Left),
        col,
        riga,
        KeyModifiers::NONE,
    ));
    let _ = render_lines(a, 40, 10);
}

fn su(a: &mut App, col: u16, riga: u16) {
    a.handle_mouse(ev(
        MouseEventKind::Up(MouseButton::Left),
        col,
        riga,
        KeyModifiers::NONE,
    ));
    let _ = render_lines(a, 40, 10);
}

fn blocco(first_row: u64, last_row: u64, col_a: u64, col_b: u64) -> Option<ColumnBlock> {
    Some(ColumnBlock {
        first_row,
        last_row,
        col_a,
        col_b,
    })
}

fn copia(a: &mut App) -> Option<Vec<u8>> {
    a.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    a.take_copied()
}

#[test]
fn alt_drag_draws_a_block_and_turns_the_column_mode_on() {
    let (mut a, p) = app();
    assert!(!a.column_mode());
    // dalla colonna 1 della riga 0 alla colonna 3 della riga 2
    giu(&mut a, G + 1, 1, KeyModifiers::ALT);
    assert!(a.column_mode());
    trascina(&mut a, G + 3, 3);
    su(&mut a, G + 3, 3);
    assert_eq!(a.column_block(), blocco(0, 14, 1, 3));
    // il cursore sta sulla riga e sulla colonna del puntatore: riga 2 (byte 14), colonna 3
    assert_eq!(a.cursor(), 17);
    assert_eq!(copia(&mut a), Some(b"bc\nhi\nno".to_vec()));
    let _ = std::fs::remove_file(p);
}

#[test]
fn dragging_up_and_to_the_left_gives_the_same_kind_of_block() {
    let (mut a, p) = app();
    // dall'ancora (riga 2, colonna 4) a (riga 0, colonna 1)
    giu(&mut a, G + 4, 3, KeyModifiers::ALT);
    trascina(&mut a, G + 1, 1);
    su(&mut a, G + 1, 1);
    assert_eq!(a.column_block(), blocco(0, 14, 1, 4));
    // il cursore e' sulla riga del puntatore, la prima: byte 1
    assert_eq!(a.cursor(), 1);
    assert_eq!(copia(&mut a), Some(b"bcd\nhij\nnop".to_vec()));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_block_follows_the_pointer_row_by_row() {
    let (mut a, p) = app();
    giu(&mut a, G + 1, 2, KeyModifiers::ALT); // riga 1 (offset 7), colonna 1
    trascina(&mut a, G + 2, 3); // una riga sotto
    assert_eq!(a.column_block(), blocco(7, 14, 1, 2));
    trascina(&mut a, G + 2, 4); // due righe sotto
    assert_eq!(a.column_block(), blocco(7, 21, 1, 2));
    trascina(&mut a, G + 2, 2); // di nuovo sull'ancora: una riga sola
    assert_eq!(a.column_block(), blocco(7, 7, 1, 2));
    trascina(&mut a, G + 2, 1); // sopra l'ancora
    assert_eq!(a.column_block(), blocco(0, 7, 1, 2));
    su(&mut a, G + 2, 1);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_plain_drag_in_the_stream_mode_is_unchanged() {
    let (mut a, p) = app();
    giu(&mut a, G + 1, 1, KeyModifiers::NONE);
    trascina(&mut a, G + 3, 2);
    su(&mut a, G + 3, 2);
    assert!(!a.column_mode());
    assert_eq!(a.column_block(), None);
    // la selezione di flusso va dal byte 1 al byte 7 + 3
    assert_eq!(a.selection(), Some((1, 10)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn in_the_column_mode_a_plain_drag_is_enough() {
    let (mut a, p) = app();
    a.handle_key(k(KeyCode::F(8)));
    assert!(a.column_mode());
    giu(&mut a, G + 1, 1, KeyModifiers::NONE);
    trascina(&mut a, G + 3, 2);
    su(&mut a, G + 3, 2);
    assert_eq!(a.column_block(), blocco(0, 7, 1, 3));
    assert_eq!(copia(&mut a), Some(b"bc\nhi".to_vec()));
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_alt_click_without_a_drag_only_moves_the_cursor_and_turns_the_mode_on() {
    let (mut a, p) = app();
    giu(&mut a, G + 2, 2, KeyModifiers::ALT);
    su(&mut a, G + 2, 2);
    assert!(a.column_mode());
    assert_eq!(a.column_block(), None);
    assert_eq!(a.cursor(), 9, "riga 1 (offset 7), colonna 2");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_pointer_past_the_end_of_a_row_extends_the_virtual_column() {
    let (mut a, p) = app();
    giu(&mut a, G + 1, 1, KeyModifiers::ALT);
    trascina(&mut a, G + 12, 2);
    su(&mut a, G + 12, 2);
    assert_eq!(a.column_block(), blocco(0, 7, 1, 12));
    // le righe sono corte: il blocco copre quello che c'e'
    assert_eq!(copia(&mut a), Some(b"bcdef\nhijkl".to_vec()));
    let _ = std::fs::remove_file(p);
}

#[test]
fn dragging_below_the_text_stops_on_the_last_row() {
    let (mut a, p) = app();
    giu(&mut a, G + 1, 1, KeyModifiers::ALT);
    // la riga 9 e' la barra dei tasti: oltre l'ultima riga di testo (la 4 dello schermo) si resta sull'ultima
    trascina(&mut a, G + 2, 9);
    assert_eq!(a.column_block(), blocco(0, 21, 1, 2));
    su(&mut a, G + 2, 9);
    let _ = std::fs::remove_file(p);
}

#[test]
fn after_the_release_a_stray_drag_does_nothing() {
    let (mut a, p) = app();
    giu(&mut a, G + 1, 1, KeyModifiers::ALT);
    trascina(&mut a, G + 3, 2);
    su(&mut a, G + 3, 2);
    let prima = a.column_block();
    trascina(&mut a, G + 5, 4);
    assert_eq!(a.column_block(), prima);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_keys_carry_on_from_the_block_the_mouse_drew() {
    let (mut a, p) = app();
    giu(&mut a, G + 1, 1, KeyModifiers::ALT);
    trascina(&mut a, G + 3, 3);
    su(&mut a, G + 3, 3);
    a.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    assert_eq!(a.column_block(), blocco(0, 14, 1, 4));
    a.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT));
    assert_eq!(a.column_block(), blocco(0, 21, 1, 4));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_new_alt_click_starts_a_new_block() {
    let (mut a, p) = app();
    giu(&mut a, G + 1, 1, KeyModifiers::ALT);
    trascina(&mut a, G + 3, 3);
    su(&mut a, G + 3, 3);
    giu(&mut a, G + 2, 2, KeyModifiers::ALT);
    trascina(&mut a, G + 4, 2);
    su(&mut a, G + 4, 2);
    assert_eq!(a.column_block(), blocco(7, 7, 2, 4));
    assert_eq!(copia(&mut a), Some(b"ij".to_vec()));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_dialog_keeps_the_mouse_to_itself() {
    let (mut a, p) = app();
    a.handle_key(k(KeyCode::F(1)));
    giu(&mut a, G + 1, 1, KeyModifiers::ALT);
    assert!(!a.column_mode());
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_alt_click_below_the_last_row_of_text_does_nothing() {
    let (mut a, p) = app();
    // la riga 6 dello schermo e' vuota: il testo finisce alla 4
    giu(&mut a, G + 2, 6, KeyModifiers::ALT);
    assert!(!a.column_mode());
    assert_eq!(a.column_block(), None);
    assert_eq!(a.cursor(), 0);
    let _ = std::fs::remove_file(p);
}
