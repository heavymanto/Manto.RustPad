//! Un file con fine riga CRLF (V-01): il cursore di fine riga sta PRIMA del `\r`, Destra/Sinistra/Backspace/Canc trattano la
//! coppia `\r\n` come un solo carattere, la selezione di una riga non porta il `\r` dentro il testo scritto. Prima della
//! correzione scrivere a fine riga metteva il carattere fra `\r` e `\n` e il file diventava misto. Gli attesi sono calcolati a
//! mano sui byte di prova. Scritti da ARCH prima della correzione.

use mrp_core::encoding::Encoding;
use mrp_tui::app::{App, AppConfig};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_crlf_test_{}_{}", std::process::id(), n));
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

fn shift(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::SHIFT)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn app(dir: &TestDir, data: &[u8], enc: Option<Encoding>) -> App {
    let p = dir.file(data);
    let cfg = AppConfig {
        encoding: enc,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT));
    assert!(a.is_edit());
    a
}

fn keys(a: &mut App, tasti: &[KeyEvent]) {
    for &t in tasti {
        a.handle_key(t);
    }
}

fn x() -> KeyEvent {
    k(KeyCode::Char('X'))
}

fn u16le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn u16be(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

type Forma = (&'static str, Option<Encoding>, fn(&str) -> Vec<u8>, u64);

/// (nome, codifica, come si scrive un testo, byte di un carattere)
fn forme() -> Vec<Forma> {
    vec![
        ("utf8", None, |s| s.as_bytes().to_vec(), 1),
        ("utf16le", Some(Encoding::Utf16Le), u16le, 2),
        ("utf16be", Some(Encoding::Utf16Be), u16be, 2),
    ]
}

#[test]
fn typing_at_the_end_of_a_row_goes_before_the_cr() {
    let dir = TestDir::new();
    for (nome, enc, cod, _) in forme() {
        let mut a = app(&dir, &cod("ab\r\ncd\r\n"), enc);
        keys(&mut a, &[k(KeyCode::End), x()]);
        assert_eq!(a.document_text(), cod("abX\r\ncd\r\n"), "{nome} End+X");
        let mut b = app(&dir, &cod("ab\r\ncd\r\n"), enc);
        keys(&mut b, &[k(KeyCode::Down), k(KeyCode::End), x()]);
        assert_eq!(b.document_text(), cod("ab\r\ncdX\r\n"), "{nome} Down+End+X");
    }
}

#[test]
fn enter_at_the_end_of_a_row_keeps_the_pairs_whole() {
    let dir = TestDir::new();
    for (nome, enc, cod, _) in forme() {
        let mut a = app(&dir, &cod("ab\r\ncd\r\n"), enc);
        keys(&mut a, &[k(KeyCode::End), k(KeyCode::Enter), x()]);
        assert_eq!(a.document_text(), cod("ab\r\nX\r\ncd\r\n"), "{nome}");
    }
}

#[test]
fn delete_at_the_end_of_a_row_removes_the_whole_pair() {
    let dir = TestDir::new();
    for (nome, enc, cod, _) in forme() {
        let mut a = app(&dir, &cod("ab\r\ncd\r\n"), enc);
        keys(&mut a, &[k(KeyCode::End), k(KeyCode::Delete)]);
        assert_eq!(a.document_text(), cod("abcd\r\n"), "{nome}");
    }
}

#[test]
fn backspace_at_the_start_of_a_row_removes_the_whole_pair() {
    let dir = TestDir::new();
    for (nome, enc, cod, _) in forme() {
        let mut a = app(&dir, &cod("ab\r\ncd\r\n"), enc);
        keys(
            &mut a,
            &[k(KeyCode::Down), k(KeyCode::Home), k(KeyCode::Backspace)],
        );
        assert_eq!(a.document_text(), cod("abcd\r\n"), "{nome}");
    }
}

#[test]
fn right_and_left_step_over_the_pair_as_one_character() {
    let dir = TestDir::new();
    for (nome, enc, cod, u) in forme() {
        let mut a = app(&dir, &cod("ab\r\ncd\r\n"), enc);
        keys(&mut a, &[k(KeyCode::Right), k(KeyCode::Right)]);
        assert_eq!(a.cursor(), 2 * u, "{nome} dopo due Destra");
        a.handle_key(k(KeyCode::Right));
        assert_eq!(a.cursor(), 4 * u, "{nome} il terzo Destra salta il CRLF");
        a.handle_key(k(KeyCode::Left));
        assert_eq!(a.cursor(), 2 * u, "{nome} Sinistra torna prima del CR");
    }
}

#[test]
fn down_from_the_end_of_a_row_lands_at_the_end_of_the_next() {
    let dir = TestDir::new();
    for (nome, enc, cod, u) in forme() {
        let mut a = app(&dir, &cod("ab\r\ncd\r\n"), enc);
        keys(&mut a, &[k(KeyCode::End), k(KeyCode::Down)]);
        assert_eq!(a.cursor(), 6 * u, "{nome}");
        a.handle_key(x());
        assert_eq!(a.document_text(), cod("ab\r\ncdX\r\n"), "{nome}");
    }
}

#[test]
fn shift_end_selects_the_text_without_the_cr() {
    let dir = TestDir::new();
    for (nome, enc, cod, u) in forme() {
        let mut a = app(&dir, &cod("ab\r\ncd\r\n"), enc);
        a.handle_key(shift(KeyCode::End));
        assert_eq!(a.selection(), Some((0, 2 * u)), "{nome}");
        a.handle_key(x());
        assert_eq!(
            a.document_text(),
            cod("X\r\ncd\r\n"),
            "{nome} sostituisce la selezione"
        );
    }
}

#[test]
fn cutting_a_selected_row_leaves_the_file_pure_crlf() {
    let dir = TestDir::new();
    for (nome, enc, cod, _) in forme() {
        let mut a = app(&dir, &cod("ab\r\ncd\r\n"), enc);
        keys(&mut a, &[shift(KeyCode::End), ctrl('x')]);
        assert_eq!(a.document_text(), cod("\r\ncd\r\n"), "{nome}");
    }
}

#[test]
fn the_last_row_without_newline_and_lf_files_are_unchanged() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"ab\r\ncd", None);
    keys(&mut a, &[k(KeyCode::Down), k(KeyCode::End), x()]);
    assert_eq!(a.document_text(), b"ab\r\ncdX");
    let mut b = app(&dir, b"ab\ncd\n", None);
    keys(
        &mut b,
        &[k(KeyCode::End), x(), k(KeyCode::Right), k(KeyCode::Right)],
    );
    assert_eq!(b.document_text(), b"abX\ncd\n");
    assert_eq!(b.cursor(), 5);
}

#[test]
fn the_saved_file_stays_pure_crlf() {
    let dir = TestDir::new();
    let p = dir.file(b"ab\r\ncd\r\n");
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT));
    keys(
        &mut a,
        &[
            k(KeyCode::End),
            x(),
            k(KeyCode::Down),
            k(KeyCode::End),
            k(KeyCode::Enter),
            x(),
            ctrl('s'),
        ],
    );
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(&p).unwrap(), b"abX\r\ncd\r\nX\r\n");
}
