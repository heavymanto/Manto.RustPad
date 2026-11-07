//! Lo stile di fine riga nel TUI (E29): riconoscimento all'apertura, il tasto Invio che scrive lo stesso stile del file
//! (un file CRLF non diventa misto), i file con il solo `\r`, la riga di stato e il cambio di codifica. Scritti da ARCH.

use mrp_core::encoding::Encoding;
use mrp_core::lineending::LineEnding;
use mrp_core::newline::Newline;
use mrp_tui::app::{App, AppConfig};
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
        let p = std::env::temp_dir().join(format!("mrp_eolapp_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
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

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

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

fn status_intera(a: &mut App) -> String {
    screen(a)[7].clone()
}

fn utf16le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[test]
fn the_style_is_recognised_at_open() {
    let dir = TestDir::new();
    let casi: &[(&[u8], LineEnding, Newline)] = &[
        (b"a\nb\n", LineEnding::Lf, Newline::Byte),
        (b"a\r\nb\r\n", LineEnding::CrLf, Newline::Byte),
        (b"a\rb\r", LineEnding::Cr, Newline::Cr),
        (b"senza fine riga", LineEnding::Lf, Newline::Byte),
    ];
    for &(dati, eol, _forma) in casi {
        let a = app(&dir, dati, None, false);
        assert_eq!(a.line_ending(), eol, "{dati:?}");
    }
    let a = app(&dir, &utf16le("a\r\nb\r\n"), Some(Encoding::Utf16Le), false);
    assert_eq!(a.line_ending(), LineEnding::CrLf);
}

#[test]
fn enter_writes_the_style_of_the_file() {
    let dir = TestDir::new();
    for (dati, atteso) in [
        (&b"ab\n"[..], &b"\nab\n"[..]),
        (&b"ab\r\ncd\r\n"[..], &b"\r\nab\r\ncd\r\n"[..]),
        (&b"ab\rcd\r"[..], &b"\rab\rcd\r"[..]),
    ] {
        let (mut a, p) = app_file(&dir, dati, None, true);
        a.handle_key(k(KeyCode::Enter));
        assert_eq!(a.document_text(), atteso, "{dati:?}");
        a.handle_key(ctrl('s'));
        assert!(a.wait_idle(Duration::from_secs(20)));
        assert_eq!(std::fs::read(&p).unwrap(), atteso);
    }
    // a 16 bit: CRLF e' due unita'
    let (mut b, _p) = app_file(&dir, &utf16le("ab\r\n"), Some(Encoding::Utf16Le), true);
    b.handle_key(k(KeyCode::Enter));
    assert_eq!(
        b.document_text(),
        [&[0x0D, 0, 0x0A, 0][..], &utf16le("ab\r\n")].concat()
    );
    // multibyte
    let sj = Encoding::by_name("shift-jis").unwrap();
    let (mut c, _q) = app_file(&dir, &sj.encode_str("日本\r\n").unwrap(), Some(sj), true);
    c.handle_key(k(KeyCode::Enter));
    assert_eq!(
        c.document_text(),
        [&b"\r\n"[..], &sj.encode_str("日本\r\n").unwrap()].concat()
    );
}

#[test]
fn a_file_with_only_cr_shows_its_lines_and_moves_by_line() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"uno\rdue\rtre\r", None, false);
    let s = screen(&mut a);
    assert!(s[0].contains("uno"), "{:?}", s[0]);
    assert!(s[1].contains("due"), "{:?}", s[1]);
    assert!(s[2].contains("tre"), "{:?}", s[2]);
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 4);
    assert!(
        status(&mut a).starts_with(" Ln 2  Col 1  byte 4"),
        "{:?}",
        status(&mut a)
    );
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.cursor(), 7);
    // scendendo si resta alla colonna 4 (dopo "due"): "tre" ha la stessa lunghezza, quindi la fine della riga 3
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 11);
    // in modifica: dopo un salvataggio l'indice si rifa con la forma giusta
    let (mut b, p) = app_file(&dir, b"uno\rdue\r", None, true);
    b.handle_key(k(KeyCode::Enter));
    b.handle_key(ctrl('s'));
    assert!(b.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(&p).unwrap(), b"\runo\rdue\r");
    b.handle_key(k(KeyCode::Down));
    assert!(status(&mut b).starts_with(" Ln 3"), "{:?}", status(&mut b));
}

#[test]
fn the_status_line_names_the_style_only_when_it_is_not_lf() {
    let dir = TestDir::new();
    // lo stile sta a destra, dopo la dimensione e prima del modo: serve una riga larga
    let larga = |a: &mut App| {
        a.resize(80, 9);
        render_lines(a, 80, 9)[7].trim_end().to_string()
    };
    let mut lf = app(&dir, b"a\nb\n", None, false);
    let st = larga(&mut lf);
    assert!(!st.contains("LF") && !st.contains("CR"), "{st:?}");
    let mut crlf = app(&dir, b"a\r\nb\r\n", None, false);
    let st = larga(&mut crlf);
    assert!(st.ends_with("CRLF  VIEW"), "{st:?}");
    let mut cr = app(&dir, b"a\rb\r", None, false);
    let st = larga(&mut cr);
    assert!(st.ends_with(" CR  VIEW"), "{st:?}");
    assert!(!st.contains("CRLF"));
    // con una codifica non UTF-8: prima il nome, poi lo stile
    let w = Encoding::by_name("windows-1252").unwrap();
    let mut b = app(&dir, b"a\r\nb\r\n", Some(w), false);
    // su 40 colonne il nome e lo stile non stanno insieme alla posizione: si prova su 80
    b.resize(80, 9);
    let st = render_lines(&mut b, 80, 9)[7].clone();
    assert!(
        st.trim_end().ends_with("windows-1252  CRLF  VIEW"),
        "{st:?}"
    );
}

#[test]
fn switching_to_a_wide_encoding_turns_a_lone_cr_into_lf() {
    let dir = TestDir::new();
    // un file con il solo \r letto come UTF-8: stile CR; in UTF-16 il CR da solo non c'e'
    let mut c = app(&dir, b"a\rb\rc\r", Some(Encoding::Utf8), false);
    assert_eq!(c.line_ending(), LineEnding::Cr);
    c.set_encoding(Encoding::Utf16Le);
    assert_eq!(c.line_ending(), LineEnding::Lf);
    assert!(c.wait_idle(Duration::from_secs(20)));
    // e tornando a UTF-8 resta LF: lo stile non si riconosce di nuovo
    c.set_encoding(Encoding::Utf8);
    assert_eq!(c.line_ending(), LineEnding::Lf);
}

#[test]
fn a_crlf_file_switches_between_8_and_16_bit_keeping_its_style() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"a\r\nb\r\n", Some(Encoding::Utf8), false);
    assert_eq!(a.line_ending(), LineEnding::CrLf);
    a.set_encoding(Encoding::Utf16Le);
    assert_eq!(a.line_ending(), LineEnding::CrLf);
    a.set_encoding(Encoding::Utf8);
    assert_eq!(a.line_ending(), LineEnding::CrLf);
}

/// Alt+E: passa alla modifica (F4 e' Sostituisci).
fn modifica() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
}
