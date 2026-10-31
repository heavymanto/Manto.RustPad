//! Il BOM nel TUI (E29): non si scrive davanti al BOM e Backspace/Delete non lo cancellano. Scritti da ARCH.

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
        let p = std::env::temp_dir().join(format!("mrp_bom_test_{}_{}", std::process::id(), n));
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

fn editor(dir: &TestDir, data: &[u8], enc: Option<Encoding>) -> App {
    let cfg = AppConfig {
        encoding: enc,
        ..AppConfig::default()
    };
    let mut a = App::open(&dir.file(data), cfg).unwrap();
    a.resize(40, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.handle_key(k(KeyCode::F(4)));
    assert!(a.is_edit());
    a
}

#[test]
fn typing_at_the_start_goes_after_the_bom() {
    let dir = TestDir::new();
    // UTF-8: EF BB BF
    let mut a = editor(&dir, &[0xEF, 0xBB, 0xBF, b'a', b'b', b'\n'], None);
    a.handle_key(k(KeyCode::Char('x')));
    assert_eq!(
        a.document_text(),
        [0xEF, 0xBB, 0xBF, b'x', b'a', b'b', b'\n']
    );
    // Enter e incolla alla stessa maniera
    let mut b = editor(&dir, &[0xEF, 0xBB, 0xBF, b'a', b'\n'], None);
    b.handle_key(k(KeyCode::Enter));
    assert_eq!(b.document_text(), [0xEF, 0xBB, 0xBF, b'\n', b'a', b'\n']);
    let mut c = editor(&dir, &[0xEF, 0xBB, 0xBF, b'a', b'\n'], None);
    c.paste_text("zz");
    assert_eq!(
        c.document_text(),
        [0xEF, 0xBB, 0xBF, b'z', b'z', b'a', b'\n']
    );
    // UTF-16 LE: FF FE
    let mut d = editor(&dir, &[0xFF, 0xFE, b'a', 0, b'\n', 0], None);
    assert_eq!(d.encoding(), Encoding::Utf16Le);
    d.handle_key(k(KeyCode::Char('x')));
    assert_eq!(d.document_text(), [0xFF, 0xFE, b'x', 0, b'a', 0, b'\n', 0]);
    // UTF-32 BE: 00 00 FE FF
    let mut e = editor(
        &dir,
        &[0, 0, 0xFE, 0xFF, 0, 0, 0, b'a', 0, 0, 0, b'\n'],
        None,
    );
    assert_eq!(e.encoding(), Encoding::Utf32Be);
    e.handle_key(k(KeyCode::Char('x')));
    assert_eq!(
        e.document_text(),
        [0, 0, 0xFE, 0xFF, 0, 0, 0, b'x', 0, 0, 0, b'a', 0, 0, 0, b'\n']
    );
}

#[test]
fn backspace_and_delete_leave_the_bom_alone() {
    let dir = TestDir::new();
    let dati = [0xEF, 0xBB, 0xBF, b'a', b'b', b'\n'];
    let mut a = editor(&dir, &dati, None);
    // Backspace con il cursore a 0 o dopo il BOM non toglie nulla
    a.handle_key(k(KeyCode::Backspace));
    assert_eq!(a.document_text(), dati);
    a.handle_key(k(KeyCode::Right)); // un carattere: il BOM (3 byte)
    assert_eq!(a.cursor(), 3);
    a.handle_key(k(KeyCode::Backspace));
    assert_eq!(a.document_text(), dati);
    // Delete a 0 toglie il primo carattere dopo il BOM, non il BOM
    let mut b = editor(&dir, &dati, None);
    b.handle_key(k(KeyCode::Delete));
    assert_eq!(b.document_text(), [0xEF, 0xBB, 0xBF, b'b', b'\n']);
    // UTF-16
    let d16 = [0xFF, 0xFE, b'a', 0, b'b', 0];
    let mut c = editor(&dir, &d16, None);
    c.handle_key(k(KeyCode::Backspace));
    assert_eq!(c.document_text(), d16);
    c.handle_key(k(KeyCode::Delete));
    assert_eq!(c.document_text(), [0xFF, 0xFE, b'b', 0]);
}

#[test]
fn files_without_a_bom_are_unchanged() {
    let dir = TestDir::new();
    let mut a = editor(&dir, b"ab\n", None);
    a.handle_key(k(KeyCode::Char('x')));
    assert_eq!(a.document_text(), b"xab\n");
    a.handle_key(k(KeyCode::Home));
    a.handle_key(k(KeyCode::Delete));
    assert_eq!(a.document_text(), b"ab\n");
    // una code page: 0xEF 0xBB 0xBF sono tre caratteri, non un BOM
    let w = Encoding::by_name("windows-1252").unwrap();
    let mut b = editor(&dir, &[0xEF, 0xBB, 0xBF, b'a'], Some(w));
    b.handle_key(k(KeyCode::Delete));
    assert_eq!(b.document_text(), [0xBB, 0xBF, b'a']);
}

#[test]
fn switching_the_encoding_updates_what_counts_as_a_bom() {
    let dir = TestDir::new();
    // file UTF-8 con BOM: letto come windows-1252 il BOM non c'e' piu'; tornando a UTF-8 si
    let mut a = editor(&dir, &[0xEF, 0xBB, 0xBF, b'a', b'\n'], None);
    let w = Encoding::by_name("windows-1252").unwrap();
    a.set_encoding(w);
    a.handle_key(k(KeyCode::Char('x')));
    assert_eq!(a.document_text(), [b'x', 0xEF, 0xBB, 0xBF, b'a', b'\n']);
    let mut b = editor(&dir, &[0xEF, 0xBB, 0xBF, b'a', b'\n'], Some(w));
    b.set_encoding(Encoding::Utf8);
    b.handle_key(k(KeyCode::Char('x')));
    assert_eq!(b.document_text(), [0xEF, 0xBB, 0xBF, b'x', b'a', b'\n']);
}
