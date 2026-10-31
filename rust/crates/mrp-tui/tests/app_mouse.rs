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

// ----- doppio e triplo clic (E38): la parola e la riga sotto il puntatore -----
// Gli istanti sono passati a mano (`handle_mouse_at`): i test non dipendono dall'orologio.
// "foo bar_1  +x\n": f0 o1 o2 ' '3 b4 a5 r6 _7 '1'8 ' '9 ' '10 '+'11 x12 a-capo 13 (len 14)

use std::time::Instant;

fn premi(a: &mut App, col: u16, row: u16, t: Instant, mods: KeyModifiers) {
    a.handle_mouse_at(
        mouse(MouseEventKind::Down(MouseButton::Left), col, row, mods),
        t,
    );
    a.handle_mouse_at(
        mouse(MouseEventKind::Up(MouseButton::Left), col, row, mods),
        t,
    );
    let _ = render_lines(a, 40, 10);
}

fn doppio(a: &mut App, col: u16, row: u16) {
    let t = Instant::now();
    premi(a, col, row, t, KeyModifiers::NONE);
    premi(
        a,
        col,
        row,
        t + Duration::from_millis(200),
        KeyModifiers::NONE,
    );
}

#[test]
fn a_double_click_selects_the_word_under_the_pointer() {
    let (mut a, p, g) = app(b"foo bar_1  +x\n", None, false);
    // in mezzo a "bar_1" (il trattino basso fa parte della parola)
    doppio(&mut a, g + 5, 1);
    assert_eq!(a.selection(), Some((4, 9)));
    // la prima lettera e l'ultima cifra della stessa parola
    doppio(&mut a, g + 4, 1);
    assert_eq!(a.selection(), Some((4, 9)));
    doppio(&mut a, g + 8, 1);
    assert_eq!(a.selection(), Some((4, 9)));
    doppio(&mut a, g + 1, 1);
    assert_eq!(a.selection(), Some((0, 3)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_double_click_on_spaces_or_punctuation_selects_that_run() {
    let (mut a, p, g) = app(b"foo bar_1  +x\n", None, false);
    doppio(&mut a, g + 10, 1);
    assert_eq!(a.selection(), Some((9, 11)), "i due spazi");
    doppio(&mut a, g + 11, 1);
    assert_eq!(a.selection(), Some((11, 12)), "il '+'");
    doppio(&mut a, g + 3, 1);
    assert_eq!(a.selection(), Some((3, 4)), "lo spazio singolo");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_double_click_past_the_end_of_the_row_takes_the_last_word() {
    let (mut a, p, g) = app(b"foo bar_1  +x\n", None, false);
    doppio(&mut a, g + 30, 1);
    assert_eq!(
        a.selection(),
        Some((12, 13)),
        "la 'x' in fondo, non l'a-capo"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_double_click_keeps_to_the_row_and_to_whole_characters() {
    // a0 ñ1-2 b3 ' '4 ü5-6 a-capo 7; "añb" sono 4 byte
    let (mut a, p, g) = app("añb ü\nxy\n".as_bytes(), None, false);
    doppio(&mut a, g + 1, 1);
    assert_eq!(a.selection(), Some((0, 4)));
    doppio(&mut a, g + 4, 1);
    assert_eq!(a.selection(), Some((5, 7)), "ü e' di due byte");
    // la parola non attraversa l'a-capo: "xy" comincia dopo
    doppio(&mut a, g, 2);
    assert_eq!(a.selection(), Some((8, 10)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_double_click_works_in_a_multibyte_encoding() {
    let sj = Encoding::by_name("shift-jis").unwrap();
    // "ab 日本\n": a0 b1 ' '2 日3-4 本5-6 a-capo 7; 日本 sono due lettere di una parola
    let dati = sj.encode_str("ab 日本\n").unwrap();
    let (mut a, p, g) = app(&dati, Some(sj), false);
    doppio(&mut a, g + 5, 1);
    assert_eq!(a.selection(), Some((3, 7)));
    doppio(&mut a, g, 1);
    assert_eq!(a.selection(), Some((0, 2)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_triple_click_selects_the_row_and_a_fourth_starts_over() {
    let (mut a, p, g) = app(b"foo bar_1  +x\nsecond\n", None, false);
    let t = Instant::now();
    premi(&mut a, g + 5, 1, t, KeyModifiers::NONE);
    assert_eq!(a.selection(), None);
    premi(
        &mut a,
        g + 5,
        1,
        t + Duration::from_millis(150),
        KeyModifiers::NONE,
    );
    assert_eq!(a.selection(), Some((4, 9)));
    premi(
        &mut a,
        g + 5,
        1,
        t + Duration::from_millis(300),
        KeyModifiers::NONE,
    );
    assert_eq!(a.selection(), Some((0, 13)), "la riga, senza l'a-capo");
    // il quarto clic ricomincia da uno: solo il cursore
    premi(
        &mut a,
        g + 5,
        1,
        t + Duration::from_millis(450),
        KeyModifiers::NONE,
    );
    assert_eq!(a.selection(), None);
    assert_eq!(a.cursor(), 5);
    // e il quinto e' il secondo della nuova serie
    premi(
        &mut a,
        g + 5,
        1,
        t + Duration::from_millis(600),
        KeyModifiers::NONE,
    );
    assert_eq!(a.selection(), Some((4, 9)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_slow_or_moved_second_click_is_not_a_double_click() {
    let (mut a, p, g) = app(b"foo bar_1  +x\n", None, false);
    let t = Instant::now();
    premi(&mut a, g + 5, 1, t, KeyModifiers::NONE);
    // troppo tardi
    premi(
        &mut a,
        g + 5,
        1,
        t + Duration::from_millis(900),
        KeyModifiers::NONE,
    );
    assert_eq!(a.selection(), None);
    // abbastanza presto ma in un'altra cella
    premi(
        &mut a,
        g + 6,
        1,
        t + Duration::from_millis(1000),
        KeyModifiers::NONE,
    );
    assert_eq!(a.selection(), None);
    assert_eq!(a.cursor(), 6);
    // e il clic lento non conta come primo di una serie lunga: il terzo qui e' un secondo
    premi(
        &mut a,
        g + 6,
        1,
        t + Duration::from_millis(1100),
        KeyModifiers::NONE,
    );
    assert_eq!(a.selection(), Some((4, 9)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn shift_click_and_dialogs_and_column_mode_never_select_words() {
    let (mut a, p, g) = app(b"foo bar_1  +x\n", None, false);
    let t = Instant::now();
    premi(&mut a, g + 1, 1, t, KeyModifiers::NONE);
    // Maiusc+clic nella stessa cella, subito dopo: estende, non e' un doppio clic
    premi(
        &mut a,
        g + 1,
        1,
        t + Duration::from_millis(100),
        KeyModifiers::SHIFT,
    );
    assert_eq!(a.selection(), None);
    // dialogo aperto
    a.handle_key(k(KeyCode::F(1)));
    premi(
        &mut a,
        g + 5,
        1,
        t + Duration::from_millis(200),
        KeyModifiers::NONE,
    );
    premi(
        &mut a,
        g + 5,
        1,
        t + Duration::from_millis(300),
        KeyModifiers::NONE,
    );
    assert_eq!(a.selection(), None);
    a.handle_key(k(KeyCode::Esc));
    // modalita' colonna
    a.handle_key(k(KeyCode::F(8)));
    premi(
        &mut a,
        g + 5,
        1,
        t + Duration::from_millis(5000),
        KeyModifiers::NONE,
    );
    premi(
        &mut a,
        g + 5,
        1,
        t + Duration::from_millis(5100),
        KeyModifiers::NONE,
    );
    assert_eq!(a.selection(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn typing_over_a_double_clicked_word_replaces_it() {
    let (mut a, p, g) = app(b"foo bar_1  +x\n", None, true);
    doppio(&mut a, g + 5, 1);
    a.handle_key(k(KeyCode::Char('Z')));
    assert_eq!(a.document_text(), b"foo Z  +x\n");
    // un solo Ctrl+Z rimette la parola
    a.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
    assert_eq!(a.document_text(), b"foo bar_1  +x\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_run_of_spaces_stops_at_the_row_edges_not_at_the_newlines() {
    // ab 0-1, spazi 2-3, a-capo 4, spazi 5-6, cd 7-8, a-capo 9
    let (mut a, p, g) = app(b"ab  \n  cd\n", None, false);
    doppio(&mut a, g + 3, 1);
    assert_eq!(
        a.selection(),
        Some((2, 4)),
        "gli spazi in fondo, non l'a-capo"
    );
    doppio(&mut a, g + 1, 2);
    assert_eq!(
        a.selection(),
        Some((5, 7)),
        "gli spazi in testa, non l'a-capo prima"
    );
    let _ = std::fs::remove_file(p);
}
