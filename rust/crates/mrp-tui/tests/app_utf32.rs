//! UTF-32 nel TUI (E26): riconoscimento col BOM, vista, cursore, scrittura, salvataggio, ricerca, F9, appunti e modo
//! colonna. Scritti da ARCH prima dell'implementazione: gli attesi sono calcolati a mano sui byte di prova (UTF-32 con
//! `char as u32`), mai dal codice sotto test. Ogni carattere e' una unita' di 4 byte.

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
        let p = std::env::temp_dir().join(format!("mrp_u32app_test_{}_{}", std::process::id(), n));
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

fn le(s: &str) -> Vec<u8> {
    s.chars().flat_map(|c| (c as u32).to_le_bytes()).collect()
}

fn be(s: &str) -> Vec<u8> {
    s.chars().flat_map(|c| (c as u32).to_be_bytes()).collect()
}

const LE: Encoding = Encoding::Utf32Le;
const BE: Encoding = Encoding::Utf32Be;

/// App su `data`, terminale 40 x 10 (7 righe di testo), indice pronto; in modifica se `edit`. Ritorna anche il file.
fn app_file(dir: &TestDir, data: &[u8], enc: Option<Encoding>, edit: bool) -> (App, PathBuf) {
    let p = dir.file(data);
    let cfg = AppConfig {
        encoding: enc,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(40, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    if edit {
        a.handle_key(k(KeyCode::F(4)));
        assert!(a.is_edit());
    }
    (a, p)
}

fn app(dir: &TestDir, data: &[u8], enc: Option<Encoding>, edit: bool) -> App {
    app_file(dir, data, enc, edit).0
}

fn screen(a: &mut App) -> Vec<String> {
    render_lines(a, 40, 10)
}

fn status(a: &mut App) -> String {
    screen(a)[8].clone()
}

fn save(a: &mut App) {
    a.handle_key(ctrl('s'));
    assert!(a.wait_idle(Duration::from_secs(20)));
}

// ---------- apertura e riconoscimento ----------

#[test]
fn utf32_files_with_a_bom_are_recognised_and_shown() {
    let dir = TestDir::new();
    let mut a = app(
        &dir,
        &[&[0xFF, 0xFE, 0, 0][..], &le("ab\ncd\n")].concat(),
        None,
        false,
    );
    assert_eq!(a.encoding(), LE);
    let s = screen(&mut a);
    assert!(s[1].contains("ab"), "{:?}", s[1]);
    assert!(s[2].contains("cd"), "{:?}", s[2]);
    assert!(
        status(&mut a).trim_end().ends_with("utf-32le"),
        "{:?}",
        status(&mut a)
    );
    let mut b = app(
        &dir,
        &[&[0, 0, 0xFE, 0xFF][..], &be("ab\ncd\n")].concat(),
        None,
        false,
    );
    assert_eq!(b.encoding(), BE);
    assert!(screen(&mut b)[2].contains("cd"));
    // un file UTF-8 e uno UTF-16 restano quel che sono
    let c = app(&dir, "ab\u{e8}\n".as_bytes(), None, false);
    assert_eq!(c.encoding(), Encoding::Utf8);
    let d = app(&dir, &[&[0xFF, 0xFE][..], b"a\0b\0"].concat(), None, false);
    assert_eq!(d.encoding(), Encoding::Utf16Le);
}

#[test]
fn an_explicit_encoding_wins_and_the_index_is_built_for_it() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("ab\ncd\n"), Some(LE), false);
    assert_eq!(a.encoding(), LE);
    assert!(screen(&mut a)[2].contains("cd"));
    assert!(
        status(&mut a).starts_with(" Ln 1  Col 1  byte 0"),
        "{:?}",
        status(&mut a)
    );
    a.handle_key(k(KeyCode::Down));
    assert!(
        status(&mut a).starts_with(" Ln 2  Col 1  byte 12"),
        "{:?}",
        status(&mut a)
    );
    assert_eq!(a.cursor(), 12);
}

// ---------- cursore ----------

#[test]
fn the_cursor_moves_one_character_at_a_time() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("ab\ncd\n"), Some(LE), false);
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 4);
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 8); // sull'a-capo
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 12); // inizio della seconda riga
    a.handle_key(k(KeyCode::Left));
    assert_eq!(a.cursor(), 8);
    a.handle_key(k(KeyCode::Home));
    assert_eq!(a.cursor(), 0);
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.cursor(), 8);
    // in lettura il cursore non va oltre l'ultimo a-capo
    for _ in 0..10 {
        a.handle_key(k(KeyCode::Right));
    }
    assert_eq!(a.cursor(), 20);
}

#[test]
fn a_character_outside_the_bmp_is_one_step_and_two_columns() {
    let dir = TestDir::new();
    for (enc, dati) in [(LE, le("a\u{1f600}b\n")), (BE, be("a\u{1f600}b\n"))] {
        let mut a = app(&dir, &dati, Some(enc), false);
        a.handle_key(k(KeyCode::Right));
        assert_eq!(a.cursor(), 4);
        a.handle_key(k(KeyCode::Right));
        assert_eq!(a.cursor(), 8, "{}", enc.name());
        assert!(
            status(&mut a).starts_with(" Ln 1  Col 4  byte 8"),
            "{:?}",
            status(&mut a)
        );
        a.handle_key(k(KeyCode::Left));
        assert_eq!(a.cursor(), 4);
        a.handle_key(k(KeyCode::End));
        assert_eq!(a.cursor(), 12);
    }
}

#[test]
fn an_invalid_unit_is_one_replacement_character_and_one_step() {
    let dir = TestDir::new();
    // 'a', l'unita' 0x00110000 (sopra U+10FFFF), 'b', a-capo
    let dati = [le("a"), vec![0x00, 0x00, 0x11, 0x00], le("b\n")].concat();
    let mut a = app(&dir, &dati, Some(LE), false);
    let s = screen(&mut a);
    assert!(s[1].contains("a\u{fffd}b"), "{:?}", s[1]);
    a.handle_key(k(KeyCode::Right));
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 8);
    // un surrogato isolato (D800) vale lo stesso
    let dati = [le("a"), vec![0x00, 0xD8, 0x00, 0x00], le("b\n")].concat();
    let mut b = app(&dir, &dati, Some(LE), false);
    assert!(screen(&mut b)[1].contains("a\u{fffd}b"));
}

#[test]
fn characters_with_the_byte_0a_do_not_split_lines() {
    let dir = TestDir::new();
    // riga 0: 'A' U+0A0A, a-capo; riga 1: U+A0000 U+100000 'x' (i due caratteri hanno `0A 00 00 00` al byte 2), a-capo
    let mut a = app(
        &dir,
        &le("A\u{0A0A}\n\u{A0000}\u{100000}x\n"),
        Some(LE),
        false,
    );
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 12);
    assert!(
        status(&mut a).starts_with(" Ln 2  Col 1  byte 12"),
        "{:?}",
        status(&mut a)
    );
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.cursor(), 24);
    a.handle_key(k(KeyCode::Up));
    assert_eq!(a.cursor(), 8);
    let s = screen(&mut a);
    assert!(s[1].contains("A\u{0A0A}"), "{:?}", s[1]);
    assert!(s[2].contains('x'), "{:?}", s[2]);
}

// ---------- scrittura e salvataggio ----------

#[test]
fn typing_writes_four_byte_units_and_save_keeps_them() {
    let dir = TestDir::new();
    let (mut a, p) = app_file(&dir, &le("ab\n"), Some(LE), true);
    type_text(&mut a, "\u{e9}\u{20ac}\u{1f600}");
    assert_eq!(
        a.document_text(),
        [le("\u{e9}\u{20ac}\u{1f600}"), le("ab\n")].concat()
    );
    assert_eq!(a.cursor(), 12);
    // Enter e Tab sono l'a-capo e il tabulatore a 32 bit
    a.handle_key(k(KeyCode::Enter));
    a.handle_key(k(KeyCode::Tab));
    assert_eq!(
        a.document_text(),
        [le("\u{e9}\u{20ac}\u{1f600}\n\t"), le("ab\n")].concat()
    );
    // Backspace toglie un carattere intero: 4 byte
    a.handle_key(k(KeyCode::Backspace)); // il tabulatore
    a.handle_key(k(KeyCode::Backspace)); // l'a-capo
    a.handle_key(k(KeyCode::Backspace)); // la faccina
    assert_eq!(
        a.document_text(),
        [le("\u{e9}\u{20ac}"), le("ab\n")].concat()
    );
    assert_eq!(a.cursor(), 8);
    // Delete toglie il carattere sotto il cursore
    a.handle_key(k(KeyCode::Home));
    a.handle_key(k(KeyCode::Delete));
    assert_eq!(a.document_text(), [le("\u{20ac}"), le("ab\n")].concat());
    save(&mut a);
    assert_eq!(
        std::fs::read(&p).unwrap(),
        [le("\u{20ac}"), le("ab\n")].concat()
    );
    assert!(!a.is_modified());
}

#[test]
fn after_a_save_the_index_is_rebuilt_for_utf32() {
    let dir = TestDir::new();
    let (mut a, p) = app_file(&dir, &le("ab\n"), Some(LE), true);
    a.handle_key(k(KeyCode::Enter));
    save(&mut a);
    assert_eq!(std::fs::read(&p).unwrap(), le("\nab\n"));
    // in modifica Down dalla seconda riga scende sulla riga vuota finale (il documento finisce con un a-capo)
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 16);
    assert!(
        status(&mut a).starts_with(" Ln 3  Col 1  byte 16"),
        "{:?}",
        status(&mut a)
    );
}

#[test]
fn every_character_can_be_written_in_utf32() {
    let dir = TestDir::new();
    let mut a = app(&dir, &be("x\n"), Some(BE), true);
    type_text(&mut a, "\u{3a9}\u{1f600}");
    assert_eq!(
        a.document_text(),
        [be("\u{3a9}\u{1f600}"), be("x\n")].concat()
    );
}

// ---------- ricerca ----------

#[test]
fn search_finds_aligned_matches_only() {
    let dir = TestDir::new();
    // LE: U+4100 U+0100 sono `00 41 00 00 | 00 01 00 00`: dentro c'e' `41 00 00 00` ('A') ma sfasato di 1;
    // la vera 'A' e' al byte 8
    let mut a = app(&dir, &le("\u{4100}\u{0100}A x\n"), Some(LE), false);
    a.handle_key(k(KeyCode::F(3)));
    type_text(&mut a, "A");
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((8, 12)));
    assert_eq!(a.cursor(), 8);
    // un carattere fuori dal piano base: 4 byte, una unita'
    let mut b = app(&dir, &le("ab\u{1f600}c\n"), Some(LE), false);
    b.handle_key(k(KeyCode::F(3)));
    type_text(&mut b, "\u{1f600}");
    b.handle_key(k(KeyCode::Enter));
    assert!(b.wait_idle(Duration::from_secs(20)));
    assert_eq!(b.found(), Some((8, 12)));
    // BE: U+10000 U+4100 sono `00 01 00 00 | 00 00 41 00`: `00 00 00 41` compare al byte 3, sfasato di 3;
    // la vera 'A' e' al byte 8
    let mut c = app(&dir, &be("\u{10000}\u{4100}A\n"), Some(BE), false);
    c.handle_key(k(KeyCode::F(3)));
    type_text(&mut c, "A");
    c.handle_key(k(KeyCode::Enter));
    assert!(c.wait_idle(Duration::from_secs(20)));
    assert_eq!(c.found(), Some((8, 12)));
}

// ---------- F9 e cambio di codifica ----------

#[test]
fn switching_to_utf32_on_a_clean_file_reindexes_and_aligns_the_cursor() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("ab\ncd\n"), Some(Encoding::Utf8), false);
    for _ in 0..5 {
        a.handle_key(k(KeyCode::Right)); // cursore a 5 (in UTF-8 ogni byte e' un passo)
    }
    assert_eq!(a.cursor(), 5);
    a.set_encoding(LE);
    assert_eq!(a.encoding(), LE);
    assert_eq!(a.cursor(), 4, "il cursore va a un confine di unita'");
    assert!(a.wait_idle(Duration::from_secs(20)));
    // dopo la reindicizzazione le righe sono quelle UTF-32; il cursore e' a 4 (colonna 2): scendendo resta alla
    // colonna 2 della riga 2, la 'd' a 16
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 16);
    assert!(
        status(&mut a).starts_with(" Ln 2  Col 2  byte 16"),
        "{:?}",
        status(&mut a)
    );
    // e si torna indietro: in UTF-8 gli a-capo sono i byte 8 e 20
    a.set_encoding(Encoding::Utf8);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.encoding(), Encoding::Utf8);
    a.handle_key(k(KeyCode::Home));
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 21);
    assert!(
        status(&mut a).starts_with(" Ln 3  Col 1  byte 21"),
        "{:?}",
        status(&mut a)
    );
}

#[test]
fn switching_between_16_and_32_bit_changes_the_newline_form_too() {
    let dir = TestDir::new();
    // UTF-32 LE letto come UTF-16 LE: `0A 00` allineato a 2 c'e' (il byte 8..10 e 10..12 sono `0A 00`, `00 00`)
    let mut a = app(&dir, &le("ab\ncd\n"), Some(LE), false);
    a.set_encoding(Encoding::Utf16Le);
    assert_eq!(a.encoding(), Encoding::Utf16Le);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.handle_key(k(KeyCode::Down));
    assert_eq!(
        a.cursor(),
        10,
        "il primo a-capo UTF-16 e' al byte 8: la riga dopo comincia a 10"
    );
    // da LE a BE: nessun a-capo valido, una sola riga lunga
    let mut b = app(&dir, &le("ab\ncd\n"), Some(LE), false);
    b.set_encoding(BE);
    assert_eq!(b.encoding(), BE);
    assert!(b.wait_idle(Duration::from_secs(20)));
    b.handle_key(k(KeyCode::Down));
    assert_eq!(b.cursor(), 0);
}

#[test]
fn with_unsaved_changes_the_switch_is_refused_until_saved() {
    let dir = TestDir::new();
    let (mut a, _p) = app_file(&dir, "abc\n".as_bytes(), Some(Encoding::Utf8), true);
    type_text(&mut a, "x");
    a.set_encoding(LE);
    assert_eq!(a.encoding(), Encoding::Utf8, "la codifica non cambia");
    assert_eq!(status(&mut a), " save before switching to/from UTF-16/32");
    assert_eq!(a.document_text(), b"xabc\n");
    // una code page si puo' sempre scegliere: stessa forma di a-capo
    let w = Encoding::by_name("windows-1252").unwrap();
    a.set_encoding(w);
    assert_eq!(a.encoding(), w);
    // salvato, si puo'
    a.set_encoding(Encoding::Utf8);
    save(&mut a);
    a.set_encoding(LE);
    assert_eq!(a.encoding(), LE);
    assert!(a.wait_idle(Duration::from_secs(20)));
    // fra 16 e 32 bit la forma cambia: con modifiche non salvate e' rifiutato anche li'
    let (mut b, _q) = app_file(&dir, &[&[0xFF, 0xFE][..], b"a\0\n\0"].concat(), None, true);
    assert_eq!(b.encoding(), Encoding::Utf16Le);
    type_text(&mut b, "x");
    b.set_encoding(LE);
    assert_eq!(b.encoding(), Encoding::Utf16Le);
    assert_eq!(status(&mut b), " save before switching to/from UTF-16/32");
    // UTF-32 LE <-> UTF-32 BE: forme diverse, rifiutato con modifiche
    let (mut c, _r) = app_file(&dir, &le("a\n"), Some(LE), true);
    type_text(&mut c, "x");
    c.set_encoding(BE);
    assert_eq!(c.encoding(), LE);
}

#[test]
fn the_menu_lists_utf32_right_after_utf16() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", Some(Encoding::Utf8), false);
    a.handle_key(k(KeyCode::F(9)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Encoding));
    for _ in 0..3 {
        a.handle_key(k(KeyCode::Down));
    }
    assert_eq!(a.menu_index(), Some(3));
    assert!(screen(&mut a).iter().any(|l| l.contains("utf-32le")));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.encoding(), LE);
    assert_eq!(status(&mut a), " encoding: utf-32le");
}

// ---------- appunti ----------

#[test]
fn copy_gives_utf8_text_and_paste_writes_utf32() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("caff\u{e8} \u{1f600}\n"), Some(LE), false);
    for _ in 0..7 {
        a.handle_key(shift(KeyCode::Right));
    }
    a.handle_key(ctrl('c'));
    assert_eq!(
        a.take_copied(),
        Some("caff\u{e8} \u{1f600}".as_bytes().to_vec())
    );
    a.handle_key(k(KeyCode::Esc));
    a.handle_key(k(KeyCode::F(4)));
    a.handle_key(ctrl('v'));
    assert_eq!(
        a.document_text(),
        le("caff\u{e8} \u{1f600}caff\u{e8} \u{1f600}\n")
    );
    // incolla dal terminale, con un a-capo
    a.paste_text("x\ny");
    assert!(a.document_text().windows(12).any(|w| w == le("x\ny")));
}

// ---------- modo colonna ----------

#[test]
fn column_typing_and_paste_use_utf32() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("ab\ncd\n"), Some(LE), true);
    a.handle_key(k(KeyCode::F(8)));
    a.handle_key(shift(KeyCode::Down));
    type_text(&mut a, "\u{e9}");
    assert_eq!(a.document_text(), le("\u{e9}ab\n\u{e9}cd\n"));
    // incolla di due righe alla colonna 1 (blocco di larghezza zero sulle due righe)
    let mut b = app(&dir, &le("12\n34\n56\n"), Some(LE), true);
    b.handle_key(k(KeyCode::F(8)));
    b.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
    b.handle_key(k(KeyCode::Right));
    b.paste_text("a\nb");
    assert_eq!(b.document_text(), le("1a2\n3b4\n56\n"));
}

#[test]
fn column_typing_leaves_a_short_row_alone_in_utf32() {
    let dir = TestDir::new();
    // la seconda riga e' piu' corta del blocco: digitando in colonna resta intatta (il riempimento con spazi a 32 bit
    // e' coperto in view_utf32.rs, a livello di nucleo)
    let mut a = app(&dir, &le("abcd\nx\nabcd\n"), Some(LE), true);
    a.handle_key(k(KeyCode::F(8)));
    for _ in 0..3 {
        a.handle_key(k(KeyCode::Right));
    }
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    type_text(&mut a, "Z");
    assert_eq!(a.document_text(), le("abcZd\nx\nabcZd\n"));
}

#[test]
fn column_copy_joins_rows_with_a_newline_and_gives_utf8() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("ab\ncd\n"), Some(LE), false);
    a.handle_key(k(KeyCode::F(8)));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"a\nc".to_vec()));
}

// ---------- code finali ----------

#[test]
fn a_tail_shorter_than_a_unit_is_one_character_and_never_a_newline() {
    let dir = TestDir::new();
    // 'A' e poi `0A 00 00`: tre byte che non completano una unita'
    let mut a = app(&dir, &[0x41, 0, 0, 0, 0x0A, 0, 0], Some(LE), false);
    for _ in 0..5 {
        a.handle_key(k(KeyCode::Right));
    }
    assert_eq!(
        a.cursor(),
        7,
        "in lettura il cursore arriva alla fine: non c'e' un a-capo finale"
    );
}

#[test]
fn switching_to_utf32_rounds_the_cursor_down_to_a_multiple_of_four() {
    let dir = TestDir::new();
    // in UTF-8 il cursore a 6 e' fra due unita' a 32 bit (4..8): torna a 4 (non basta arrotondare a un numero pari)
    let mut a = app(&dir, &le("abcd\n"), Some(Encoding::Utf8), false);
    for _ in 0..6 {
        a.handle_key(k(KeyCode::Right));
    }
    assert_eq!(a.cursor(), 6);
    a.set_encoding(LE);
    assert_eq!(a.cursor(), 4);
    // a 7 e a 3 uguale
    let mut b = app(&dir, &be("abcd\n"), Some(Encoding::Utf8), false);
    for _ in 0..7 {
        b.handle_key(k(KeyCode::Right));
    }
    b.set_encoding(BE);
    assert_eq!(b.cursor(), 4);
}
