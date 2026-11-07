//! Le codifiche nel TUI (E20): lettura, cursore, scrittura, appunti, ricerca, menu F9. Scritti da ARCH prima
//! dell'implementazione: gli attesi sono calcolati a mano sui byte di prova, mai dal codice sotto test.

use mrp_core::encoding::Encoding;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_enc_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    /// Un file nuovo a ogni chiamata (su Windows un file ancora mappato non si puo' troncare).
    fn file(&self, data: &[u8]) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = self.0.join(format!("f{n}.txt"));
        std::fs::write(&p, data).unwrap();
        p
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn shift(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::SHIFT)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_text(a: &mut App, s: &str) {
    for c in s.chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
}

fn cp1252() -> Encoding {
    Encoding::by_name("windows-1252").unwrap()
}

/// App su `data`, terminale 40 x 10 (7 righe di testo), indice pronto; in modifica se `edit`.
fn app(dir: &TestDir, data: &[u8], enc: Option<Encoding>, edit: bool) -> App {
    let p = dir.file(data);
    let cfg = AppConfig {
        encoding: enc,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    if edit {
        a.handle_key(modifica());
        assert!(a.is_edit());
    }
    a
}

fn screen(a: &mut App) -> Vec<String> {
    render_lines(a, 40, 9)
}

/// La parte sinistra della riga di stato (messaggio o posizione): a destra ci sono nome, dimensione e modo, almeno tre spazi dopo.
fn status(a: &mut App) -> String {
    status_intera(a)
        .split("   ")
        .next()
        .unwrap_or("")
        .trim_end()
        .to_string()
}

fn status_intera(a: &mut App) -> String {
    screen(a)[7].clone()
}

// ---------- apertura e riconoscimento ----------

#[test]
fn the_default_config_is_utf8_so_old_behaviour_is_unchanged() {
    assert_eq!(AppConfig::default().encoding, Some(Encoding::Utf8));
    let dir = TestDir::new();
    let mut a = app(&dir, b"caff\xe8\n", Some(Encoding::Utf8), false);
    assert_eq!(a.encoding(), Encoding::Utf8);
    assert!(screen(&mut a)[0].contains("caff\u{fffd}"));
}

#[test]
fn an_explicit_code_page_reads_the_bytes_as_that_code_page() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"caff\xe8 \x80\n", Some(cp1252()), false);
    assert_eq!(a.encoding(), cp1252());
    let s = screen(&mut a);
    assert!(s[0].contains("caff\u{e8} \u{20ac}"), "{:?}", s[0]);
}

#[test]
fn without_an_explicit_encoding_the_file_start_decides() {
    let dir = TestDir::new();
    let casi: Vec<(Vec<u8>, Encoding)> = vec![
        (b"plain ascii\n".to_vec(), Encoding::Utf8),
        ("caff\u{e8} \u{20ac}\n".as_bytes().to_vec(), Encoding::Utf8),
        (b"caff\xe8 ok\n".to_vec(), cp1252()),
        (b"a\x00b \xe8\n".to_vec(), Encoding::Utf8),
        (Vec::new(), Encoding::Utf8),
    ];
    for (data, atteso) in casi {
        let a = app(&dir, &data, None, false);
        assert_eq!(a.encoding(), atteso, "{:?}", String::from_utf8_lossy(&data));
    }
}

#[test]
fn the_status_line_names_the_encoding_only_when_it_is_not_utf8() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", Some(Encoding::Utf8), false);
    assert_eq!(status(&mut a), " Ln 1  Col 1  byte 0  0%");
    let mut b = app(&dir, b"abc\n", Some(cp1252()), false);
    // la codifica sta a destra, dopo la dimensione e prima del modo: su 40 colonne non c'e' posto, su 80 si
    b.resize(80, 9);
    let s = render_lines(&mut b, 80, 9)[7].clone();
    assert!(s.starts_with(" Ln 1  Col 1  byte 0  0%"), "{s:?}");
    assert!(s.trim_end().ends_with("4 B  windows-1252  VIEW"), "{s:?}");
}

// ---------- cursore ----------

#[test]
fn the_cursor_moves_one_character_at_a_time_in_the_active_encoding() {
    let dir = TestDir::new();
    // C3 A9 = "é" in UTF-8, "Ã©" (due caratteri) in windows-1252
    let mut u = app(&dir, b"\xc3\xa9x\n", Some(Encoding::Utf8), false);
    u.handle_key(k(KeyCode::Right));
    assert_eq!(u.cursor(), 2);
    let mut w = app(&dir, b"\xc3\xa9x\n", Some(cp1252()), false);
    w.handle_key(k(KeyCode::Right));
    assert_eq!(w.cursor(), 1);
    w.handle_key(k(KeyCode::Right));
    assert_eq!(w.cursor(), 2);
    w.handle_key(k(KeyCode::Left));
    assert_eq!(w.cursor(), 1);
    // una colonna dopo una sola freccia: "Col 2" in windows-1252, "Col 2" dopo due byte in UTF-8
    assert!(status(&mut w).starts_with(" Ln 1  Col 2  byte 1"));
    assert!(status(&mut u).starts_with(" Ln 1  Col 2  byte 2"));
}

#[test]
fn changing_the_encoding_puts_the_cursor_on_a_character_boundary() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"\xc3\xa9x\n", Some(cp1252()), false);
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 1); // in mezzo a "é" per UTF-8
    a.set_encoding(Encoding::Utf8);
    assert_eq!(a.encoding(), Encoding::Utf8);
    assert_eq!(a.cursor(), 0);
    assert_eq!(status(&mut a), " encoding: utf-8");
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 2);
    // verso una code page ogni offset e' un confine
    a.set_encoding(cp1252());
    assert_eq!(a.cursor(), 2);
}

#[test]
fn changing_the_encoding_drops_the_selection() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", Some(cp1252()), false);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    assert_eq!(a.selection(), Some((0, 2)));
    a.set_encoding(Encoding::Utf8);
    assert_eq!(a.selection(), None);
}

// ---------- scrittura ----------

#[test]
fn typing_writes_the_bytes_of_the_active_encoding() {
    let dir = TestDir::new();
    let mut w = app(&dir, b"abc\n", Some(cp1252()), true);
    type_text(&mut w, "\u{e9}\u{20ac}");
    assert_eq!(w.document_text(), b"\xe9\x80abc\n");
    assert_eq!(w.cursor(), 2);
    let mut u = app(&dir, b"abc\n", Some(Encoding::Utf8), true);
    type_text(&mut u, "\u{e9}\u{3a9}");
    assert_eq!(u.document_text(), b"\xc3\xa9\xce\xa9abc\n");
}

#[test]
fn a_character_the_code_page_cannot_write_is_refused() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", Some(cp1252()), true);
    type_text(&mut a, "\u{3a9}");
    assert_eq!(a.document_text(), b"abc\n");
    assert!(!a.is_modified());
    assert_eq!(status(&mut a), " cannot write '\u{3a9}' in windows-1252");
    // il resto continua a funzionare
    type_text(&mut a, "x");
    assert_eq!(a.document_text(), b"xabc\n");
}

#[test]
fn pasted_text_is_encoded_all_or_nothing() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", Some(cp1252()), true);
    a.paste_text("ab\u{20ac}\u{3a9}cd");
    assert_eq!(
        a.document_text(),
        b"abc\n",
        "un carattere non scrivibile: niente incolla"
    );
    assert_eq!(status(&mut a), " cannot write '\u{3a9}' in windows-1252");
    a.paste_text("caff\u{e8} \u{20ac}");
    assert_eq!(a.document_text(), b"caff\xe8 \x80abc\n");
}

#[test]
fn tab_and_enter_are_ascii_in_every_encoding() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"ab\n", Some(cp1252()), true);
    a.handle_key(k(KeyCode::Tab));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.document_text(), b"\t\nab\n");
}

#[test]
fn column_typing_writes_one_byte_per_row_in_a_code_page() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"ab\ncd\n", Some(cp1252()), true);
    a.handle_key(k(KeyCode::F(8)));
    a.handle_key(shift(KeyCode::Down));
    type_text(&mut a, "\u{e9}");
    assert_eq!(a.document_text(), b"\xe9ab\n\xe9cd\n");
    assert_eq!(a.vcol(), 1);
    // un carattere non scrivibile non cambia niente
    type_text(&mut a, "\u{3a9}");
    assert_eq!(a.document_text(), b"\xe9ab\n\xe9cd\n");
    assert_eq!(a.vcol(), 1);
}

// ---------- appunti ----------

#[test]
fn copy_gives_utf8_text_and_paste_encodes_it_back() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"caff\xe8\n", Some(cp1252()), false);
    for _ in 0..5 {
        a.handle_key(shift(KeyCode::Right));
    }
    a.handle_key(ctrl('c'));
    // gli appunti del terminale (OSC 52) sono UTF-8
    assert_eq!(a.take_copied(), Some("caff\u{e8}".as_bytes().to_vec()));
    a.handle_key(k(KeyCode::Esc));
    a.handle_key(modifica());
    a.handle_key(ctrl('v'));
    assert_eq!(a.document_text(), b"caff\xe8caff\xe8\n");
}

#[test]
fn utf8_documents_still_copy_and_paste_raw_bytes() {
    let dir = TestDir::new();
    // un byte non valido resta tale: la copia in UTF-8 e' esatta, come prima
    let mut a = app(&dir, b"a\xe9b\n", Some(Encoding::Utf8), true);
    for _ in 0..3 {
        a.handle_key(shift(KeyCode::Right));
    }
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"a\xe9b".to_vec()));
    a.handle_key(k(KeyCode::Esc));
    a.handle_key(ctrl('v'));
    assert_eq!(a.document_text(), b"a\xe9ba\xe9b\n");
}

#[test]
fn a_clipboard_that_the_code_page_cannot_hold_is_refused_on_paste() {
    let dir = TestDir::new();
    // copio da un documento UTF-8 un carattere che windows-1252 non ha, poi cambio codifica e incollo
    let mut a = app(&dir, "x\u{3a9}y\n".as_bytes(), Some(Encoding::Utf8), true);
    for _ in 0..3 {
        a.handle_key(shift(KeyCode::Right));
    }
    a.handle_key(ctrl('c'));
    a.handle_key(k(KeyCode::Esc));
    a.set_encoding(cp1252());
    let prima = a.document_text();
    a.handle_key(ctrl('v'));
    assert_eq!(a.document_text(), prima);
    assert_eq!(status(&mut a), " cannot write '\u{3a9}' in windows-1252");
}

// ---------- ricerca ----------

#[test]
fn search_encodes_the_needle_in_the_active_encoding() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"caff\xe8 ok\n", Some(cp1252()), false);
    a.handle_key(k(KeyCode::F(3)));
    type_text(&mut a, "\u{e8}");
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((4, 5)));
    assert_eq!(a.cursor(), 4);
    // un carattere che la code page non ha non puo' essere nel file
    let mut b = app(&dir, b"caff\xe8 ok\n", Some(cp1252()), false);
    b.handle_key(k(KeyCode::F(3)));
    type_text(&mut b, "\u{3a9}");
    b.handle_key(k(KeyCode::Enter));
    assert!(b.wait_idle(Duration::from_secs(20)));
    assert_eq!(b.found(), None);
    assert_eq!(b.cursor(), 0);
    assert_eq!(status(&mut b), " not found: \u{3a9}");
}

// ---------- menu F9 ----------

#[test]
fn f9_opens_the_encoding_menu_on_the_current_encoding() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", Some(Encoding::Utf8), false);
    a.handle_key(k(KeyCode::F(9)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Encoding));
    assert_eq!(a.menu_index(), Some(0));
    assert!(screen(&mut a).iter().any(|l| l.contains("ENCODING")));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.dialog_kind(), None);
    let mut b = app(&dir, b"abc\n", Some(cp1252()), false);
    b.handle_key(k(KeyCode::F(9)));
    let i = Encoding::all().iter().position(|e| *e == cp1252()).unwrap();
    assert_eq!(b.menu_index(), Some(i));
}

#[test]
fn the_menu_applies_with_enter_and_cancels_with_esc() {
    let dir = TestDir::new();
    let tutte = Encoding::all();
    let mut a = app(&dir, b"abc\n", Some(Encoding::Utf8), false);
    a.handle_key(k(KeyCode::F(9)));
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.menu_index(), Some(2));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.encoding(), tutte[2]);
    assert_eq!(status(&mut a), format!(" encoding: {}", tutte[2].name()));
    // Esc: niente cambia
    a.handle_key(k(KeyCode::F(9)));
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.encoding(), tutte[2]);
    // ferme agli estremi; Home e End
    a.handle_key(k(KeyCode::F(9)));
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.menu_index(), Some(tutte.len() - 1));
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.menu_index(), Some(tutte.len() - 1));
    a.handle_key(k(KeyCode::Home));
    assert_eq!(a.menu_index(), Some(0));
    a.handle_key(k(KeyCode::Up));
    assert_eq!(a.menu_index(), Some(0));
}

#[test]
fn the_long_menu_scrolls_so_the_choice_is_always_visible() {
    let dir = TestDir::new();
    let tutte = Encoding::all();
    let mut a = app(&dir, b"abc\n", Some(Encoding::Utf8), false);
    a.handle_key(k(KeyCode::F(9)));
    for i in 0..tutte.len() - 1 {
        a.handle_key(k(KeyCode::Down));
        let nome = tutte[i + 1].name();
        assert!(
            screen(&mut a).iter().any(|l| l.contains(nome)),
            "la voce scelta ({nome}) non si vede nello schermo 40 x 10"
        );
    }
}

#[test]
fn f9_does_not_open_while_another_dialog_is_open() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", Some(Encoding::Utf8), false);
    a.handle_key(k(KeyCode::F(6)));
    a.handle_key(k(KeyCode::F(9)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Theme));
}

#[test]
fn the_help_window_lists_f9() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", Some(Encoding::Utf8), false);
    a.resize(80, 23);
    a.handle_key(k(KeyCode::F(1)));
    let righe = render_lines(&mut a, 80, 23);
    assert!(
        righe
            .iter()
            .any(|l| l.contains("F9") && l.contains("text encoding")),
        "{righe:#?}"
    );
}

#[test]
fn backspace_and_delete_remove_one_character_of_the_active_encoding() {
    let dir = TestDir::new();
    // C3 A9: due caratteri in windows-1252, uno solo ("é") in UTF-8
    let mut w = app(&dir, b"\xc3\xa9x\n", Some(cp1252()), true);
    w.handle_key(k(KeyCode::Right));
    w.handle_key(k(KeyCode::Right));
    assert_eq!(w.cursor(), 2);
    w.handle_key(k(KeyCode::Backspace));
    assert_eq!(w.document_text(), b"\xc3x\n");
    assert_eq!(w.cursor(), 1);
    w.handle_key(k(KeyCode::Delete));
    assert_eq!(w.document_text(), b"\xc3\n");
    let mut u = app(&dir, b"\xc3\xa9x\n", Some(Encoding::Utf8), true);
    u.handle_key(k(KeyCode::Right));
    u.handle_key(k(KeyCode::Backspace));
    assert_eq!(u.document_text(), b"x\n");
    let mut d = app(&dir, b"\xc3\xa9x\n", Some(Encoding::Utf8), true);
    d.handle_key(k(KeyCode::Delete));
    assert_eq!(d.document_text(), b"x\n");
}

#[test]
fn copy_message_counts_document_bytes() {
    // il messaggio "copied N bytes" conta i byte del DOCUMENTO (1 per carattere in una code page)
    let dir = TestDir::new();
    let mut a = app(&dir, b"caff\xe8\n", Some(cp1252()), false);
    for _ in 0..5 {
        a.handle_key(shift(KeyCode::Right));
    }
    a.handle_key(ctrl('c'));
    assert_eq!(status(&mut a), " copied 5 bytes");
}

#[test]
fn delete_at_the_start_removes_one_byte_in_a_code_page() {
    let dir = TestDir::new();
    let mut w = app(&dir, b"\xc3\xa9x\n", Some(cp1252()), true);
    w.handle_key(k(KeyCode::Delete));
    assert_eq!(w.document_text(), b"\xa9x\n");
}

#[test]
fn changing_the_encoding_drops_the_column_block_and_the_found_mark() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\ndef\n", Some(cp1252()), false);
    a.handle_key(k(KeyCode::F(8)));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Right));
    assert!(a.column_block().is_some());
    a.set_encoding(Encoding::Utf8);
    assert_eq!(a.column_block(), None);
    assert!(a.column_mode(), "la modalita' colonna resta accesa");
    // l'evidenziazione dell'ultima ricerca sparisce
    let mut b = app(&dir, b"abc\ndef\n", Some(cp1252()), false);
    b.handle_key(k(KeyCode::F(3)));
    type_text(&mut b, "def");
    b.handle_key(k(KeyCode::Enter));
    assert!(b.wait_idle(Duration::from_secs(20)));
    assert_eq!(b.found(), Some((4, 7)));
    b.set_encoding(Encoding::Utf8);
    assert_eq!(b.found(), None);
}

#[test]
fn column_copy_gives_utf8_text_in_a_code_page() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"a\xe8c\nd\xe8f\n", Some(cp1252()), false);
    a.handle_key(k(KeyCode::F(8)));
    a.handle_key(k(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some("\u{e8}\n\u{e8}".as_bytes().to_vec()));
}

/// Alt+E: passa alla modifica (F4 e' Sostituisci).
fn modifica() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
}
