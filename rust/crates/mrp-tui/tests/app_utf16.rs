//! UTF-16 nel TUI (E24): riconoscimento, vista, cursore, scrittura, salvataggio, ricerca, F9, appunti e modo colonna.
//! Scritti da ARCH prima dell'implementazione: gli attesi sono calcolati a mano sui byte di prova (UTF-16 con la libreria
//! standard), mai dal codice sotto test.

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
        let p = std::env::temp_dir().join(format!("mrp_u16app_test_{}_{}", std::process::id(), n));
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
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn be(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

const LE: Encoding = Encoding::Utf16Le;
const BE: Encoding = Encoding::Utf16Be;

/// App su `data`, terminale 40 x 10 (7 righe di testo), indice pronto; in modifica se `edit`. Ritorna anche il file.
fn app_file(dir: &TestDir, data: &[u8], enc: Option<Encoding>, edit: bool) -> (App, PathBuf) {
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
    (a, p)
}

fn app(dir: &TestDir, data: &[u8], enc: Option<Encoding>, edit: bool) -> App {
    app_file(dir, data, enc, edit).0
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

/// La riga di stato a 80 colonne (a 40 non c'e' posto per la codifica, che sta a destra).
fn status_larga(a: &mut App) -> String {
    a.resize(80, 9);
    let r = render_lines(a, 80, 9)[7].trim_end().to_string();
    a.resize(40, 9);
    r
}

fn status_intera(a: &mut App) -> String {
    screen(a)[7].clone()
}

fn save(a: &mut App) {
    a.handle_key(ctrl('s'));
    assert!(a.wait_idle(Duration::from_secs(20)));
}

// ---------- apertura e riconoscimento ----------

#[test]
fn utf16_files_are_recognised_and_shown() {
    let dir = TestDir::new();
    // con BOM, LE e BE
    let mut a = app(
        &dir,
        &[&[0xFF, 0xFE][..], &le("ab\ncd\n")].concat(),
        None,
        false,
    );
    assert_eq!(a.encoding(), LE);
    let s = screen(&mut a);
    assert!(s[0].contains("ab"), "{:?}", s[0]);
    assert!(s[1].contains("cd"), "{:?}", s[1]);
    let mut b = app(
        &dir,
        &[&[0xFE, 0xFF][..], &be("ab\ncd\n")].concat(),
        None,
        false,
    );
    assert_eq!(b.encoding(), BE);
    assert!(screen(&mut b)[1].contains("cd"));
    // senza BOM: lo si indovina dagli zeri
    let testo = "prima riga di prova\nseconda riga\n";
    let mut c = app(&dir, &le(testo), None, false);
    assert_eq!(c.encoding(), LE);
    assert!(screen(&mut c)[1].contains("seconda riga"));
    let mut d = app(&dir, &be(testo), None, false);
    assert_eq!(d.encoding(), BE);
    assert!(screen(&mut d)[1].contains("seconda riga"));
    // il nome compare nella riga di stato
    assert!(
        status_larga(&mut c).contains("utf-16le"),
        "{:?}",
        status_larga(&mut c)
    );
    // un file UTF-8 resta UTF-8
    let e = app(&dir, "ab\u{e8}\n".as_bytes(), None, false);
    assert_eq!(e.encoding(), Encoding::Utf8);
}

#[test]
fn an_explicit_encoding_wins_and_the_index_is_built_for_it() {
    let dir = TestDir::new();
    // meno di 8 byte: il riconoscimento non lo indovinerebbe, la richiesta esplicita si
    let mut a = app(&dir, &le("ab\ncd\n"), Some(LE), false);
    assert_eq!(a.encoding(), LE);
    assert!(screen(&mut a)[1].contains("cd"));
    // l'indice righe c'e' (la riga di stato conosce la riga) e conta le righe giuste: "vai a" riga 2
    assert!(
        status(&mut a).starts_with(" Ln 1  Col 1  byte 0"),
        "{:?}",
        status(&mut a)
    );
    a.handle_key(k(KeyCode::Down));
    assert!(
        status(&mut a).starts_with(" Ln 2  Col 1  byte 6"),
        "{:?}",
        status(&mut a)
    );
    assert_eq!(a.cursor(), 6);
}

// ---------- cursore ----------

#[test]
fn the_cursor_moves_one_character_at_a_time() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("ab\ncd\n"), Some(LE), false);
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 2);
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 4); // sull'a-capo
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 6); // inizio della seconda riga
    a.handle_key(k(KeyCode::Left));
    assert_eq!(a.cursor(), 4);
    a.handle_key(k(KeyCode::Home));
    assert_eq!(a.cursor(), 0);
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.cursor(), 4);
    // in lettura il cursore non va oltre l'ultimo a-capo
    for _ in 0..10 {
        a.handle_key(k(KeyCode::Right));
    }
    assert_eq!(a.cursor(), 10);
}

#[test]
fn a_surrogate_pair_is_one_step_and_two_columns() {
    let dir = TestDir::new();
    for (enc, dati) in [(LE, le("a\u{1f600}b\n")), (BE, be("a\u{1f600}b\n"))] {
        let mut a = app(&dir, &dati, Some(enc), false);
        a.handle_key(k(KeyCode::Right));
        assert_eq!(a.cursor(), 2);
        a.handle_key(k(KeyCode::Right));
        assert_eq!(a.cursor(), 6, "{}", enc.name());
        assert!(
            status(&mut a).starts_with(" Ln 1  Col 4  byte 6"),
            "{:?}",
            status(&mut a)
        );
        a.handle_key(k(KeyCode::Left));
        assert_eq!(a.cursor(), 2);
        a.handle_key(k(KeyCode::End));
        assert_eq!(a.cursor(), 8);
    }
}

#[test]
fn characters_with_the_byte_0a_do_not_split_lines() {
    let dir = TestDir::new();
    // riga 0: 'A' U+010A, a-capo; riga 1: U+0A15 U+0100 'x' (contiene `0A 00` sfasato), a-capo
    let mut a = app(&dir, &le("A\u{010A}\n\u{0A15}\u{0100}x\n"), Some(LE), false);
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 6);
    assert!(
        status(&mut a).starts_with(" Ln 2  Col 1  byte 6"),
        "{:?}",
        status(&mut a)
    );
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.cursor(), 12);
    a.handle_key(k(KeyCode::Up));
    assert_eq!(a.cursor(), 4);
    let s = screen(&mut a);
    assert!(s[0].contains("A\u{010A}"), "{:?}", s[0]);
    assert!(s[1].contains("\u{0A15}\u{0100}x"), "{:?}", s[1]);
}

// ---------- scrittura e salvataggio ----------

#[test]
fn typing_writes_utf16_units_and_pairs_and_save_keeps_them() {
    let dir = TestDir::new();
    let (mut a, p) = app_file(&dir, &le("ab\n"), Some(LE), true);
    type_text(&mut a, "\u{e9}\u{20ac}\u{1f600}");
    assert_eq!(
        a.document_text(),
        [le("\u{e9}\u{20ac}\u{1f600}"), le("ab\n")].concat()
    );
    assert_eq!(a.cursor(), 8);
    // Enter e Tab sono l'a-capo e il tabulatore a 16 bit
    a.handle_key(k(KeyCode::Enter));
    a.handle_key(k(KeyCode::Tab));
    assert_eq!(
        a.document_text(),
        [le("\u{e9}\u{20ac}\u{1f600}\n\t"), le("ab\n")].concat()
    );
    // Backspace toglie un carattere intero, due unita' per la coppia
    a.handle_key(k(KeyCode::Backspace)); // il tabulatore
    a.handle_key(k(KeyCode::Backspace)); // l'a-capo
    a.handle_key(k(KeyCode::Backspace)); // la faccina
    assert_eq!(
        a.document_text(),
        [le("\u{e9}\u{20ac}"), le("ab\n")].concat()
    );
    assert_eq!(a.cursor(), 4);
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
fn after_a_save_the_index_is_rebuilt_for_utf16() {
    let dir = TestDir::new();
    let (mut a, p) = app_file(&dir, &le("ab\n"), Some(LE), true);
    a.handle_key(k(KeyCode::Enter));
    save(&mut a);
    assert_eq!(std::fs::read(&p).unwrap(), le("\nab\n"));
    // se l'indice non fosse stato rifatto con la forma giusta, la riga non si conoscerebbe
    // (in modifica Down dalla seconda riga scende sulla riga vuota finale: il documento finisce con un a-capo)
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 8);
    assert!(
        status(&mut a).starts_with(" Ln 3  Col 1  byte 8"),
        "{:?}",
        status(&mut a)
    );
}

#[test]
fn a_character_a_utf16_document_can_always_hold_is_written_even_if_replacement() {
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
fn search_finds_aligned_matches_only_and_covers_the_whole_pair() {
    let dir = TestDir::new();
    // LE: U+4100 U+4200 sono `00 41 00 42`: dentro c'e' `41 00` ('A') ma sfasato; la vera 'A' e' a 4
    let mut a = app(&dir, &le("\u{4100}\u{4200}A x\u{1f600}\n"), Some(LE), false);
    a.handle_key(k(KeyCode::F(3)));
    type_text(&mut a, "A");
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((4, 6)));
    assert_eq!(a.cursor(), 4);
    // la faccina: 4 byte
    let mut b = app(&dir, &le("ab\u{1f600}c\n"), Some(LE), false);
    b.handle_key(k(KeyCode::F(3)));
    type_text(&mut b, "\u{1f600}");
    b.handle_key(k(KeyCode::Enter));
    assert!(b.wait_idle(Duration::from_secs(20)));
    assert_eq!(b.found(), Some((4, 8)));
    // BE: stessa prova della trappola
    let mut c = app(&dir, &be("\u{4100}\u{4142}A\n"), Some(BE), false);
    c.handle_key(k(KeyCode::F(3)));
    type_text(&mut c, "A");
    c.handle_key(k(KeyCode::Enter));
    assert!(c.wait_idle(Duration::from_secs(20)));
    assert_eq!(c.found(), Some((4, 6)));
}

// ---------- F9 e cambio di codifica ----------

#[test]
fn switching_to_utf16_on_a_clean_file_reindexes_and_aligns_the_cursor() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("ab\ncd\n"), Some(Encoding::Utf8), false);
    a.handle_key(k(KeyCode::Right)); // cursore a 1 (in UTF-8)
    assert_eq!(a.cursor(), 1);
    a.set_encoding(LE);
    assert_eq!(a.encoding(), LE);
    assert_eq!(a.cursor(), 0, "il cursore va a un confine di unita'");
    assert!(a.wait_idle(Duration::from_secs(20)));
    // dopo la reindicizzazione le righe sono quelle UTF-16
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 6);
    assert!(
        status(&mut a).starts_with(" Ln 2  Col 1  byte 6"),
        "{:?}",
        status(&mut a)
    );
    // e si torna indietro
    a.set_encoding(Encoding::Utf8);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.encoding(), Encoding::Utf8);
    // in UTF-8 `ab\0\ncd...` ha gli a-capo ai byte 4 e 10: dal cursore a 6 Home va a 5 e Down all'ultima riga (11)
    a.handle_key(k(KeyCode::Home));
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 11);
    assert!(
        status(&mut a).starts_with(" Ln 3  Col 1  byte 11"),
        "{:?}",
        status(&mut a)
    );
}

#[test]
fn le_to_be_also_changes_the_newline_form() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("ab\ncd\n"), Some(LE), false);
    a.set_encoding(BE);
    assert_eq!(a.encoding(), BE);
    assert!(a.wait_idle(Duration::from_secs(20)));
    // gli stessi byte letti come BE: nessun a-capo valido (`0A 00` non e' `00 0A`), una sola riga lunga
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 0);
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
}

#[test]
fn the_menu_lists_utf16_right_after_utf8() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", Some(Encoding::Utf8), false);
    a.handle_key(k(KeyCode::F(9)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Encoding));
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.menu_index(), Some(1));
    assert!(screen(&mut a).iter().any(|l| l.contains("utf-16le")));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.encoding(), LE);
    assert_eq!(status(&mut a), " encoding: utf-16le");
}

// ---------- appunti ----------

#[test]
fn copy_gives_utf8_text_and_paste_writes_utf16() {
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
    a.handle_key(modifica());
    a.handle_key(ctrl('v'));
    assert_eq!(
        a.document_text(),
        le("caff\u{e8} \u{1f600}caff\u{e8} \u{1f600}\n")
    );
    // incolla dal terminale, con un a-capo
    a.paste_text("x\ny");
    assert!(a.document_text().windows(6).any(|w| w == le("x\ny")));
}

// ---------- modo colonna ----------

#[test]
fn column_typing_and_paste_use_utf16() {
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
fn column_copy_joins_rows_with_a_newline_and_gives_utf8() {
    let dir = TestDir::new();
    let mut a = app(&dir, &le("ab\ncd\n"), Some(LE), false);
    a.handle_key(k(KeyCode::F(8)));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"a\nc".to_vec()));
}

#[test]
fn an_odd_length_file_does_not_end_with_a_newline_even_if_the_last_two_bytes_look_like_one() {
    let dir = TestDir::new();
    // LE: U+0A41 (`41 0A`) e il byte spaiato 00: gli ultimi due byte sono `0A 00`, ma sfasati
    let mut a = app(&dir, &[0x41, 0x0A, 0x00], Some(LE), false);
    for _ in 0..5 {
        a.handle_key(k(KeyCode::Right));
    }
    assert_eq!(
        a.cursor(),
        3,
        "in lettura il cursore arriva alla fine: non c'e' un a-capo finale"
    );
}

#[test]
fn switching_to_utf16_in_the_middle_of_a_surrogate_pair_goes_back_to_its_start() {
    let dir = TestDir::new();
    // "a" (0..2), la faccina (2..6: alto a 2, basso a 4), "b", a-capo; in UTF-8 il cursore avanza di un byte
    let mut a = app(&dir, &le("a\u{1f600}b\n"), Some(Encoding::Utf8), false);
    for _ in 0..4 {
        a.handle_key(k(KeyCode::Right));
    }
    assert_eq!(a.cursor(), 4);
    a.set_encoding(LE);
    assert_eq!(a.cursor(), 2);
    // su un confine di unita' resta dov'e' (6 e' l'inizio di "b")
    let mut b = app(&dir, &le("a\u{1f600}b\n"), Some(Encoding::Utf8), false);
    for _ in 0..6 {
        b.handle_key(k(KeyCode::Right));
    }
    assert_eq!(b.cursor(), 6);
    b.set_encoding(LE);
    assert_eq!(b.cursor(), 6);
}

/// Alt+E: passa alla modifica (F4 e' Sostituisci).
fn modifica() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
}
