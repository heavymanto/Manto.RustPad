//! Le codifiche multibyte asiatiche nel TUI (E28): vista, cursore avanti e indietro (anche sui caratteri il cui secondo
//! byte e' uguale a un carattere ASCII), scrittura, cancellazione, ricerca, F9, appunti e modo colonna. Scritti da ARCH:
//! gli attesi sono calcolati a mano sui byte di prova (riletti con i codec di Python), mai dal codice sotto test.

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
        let p = std::env::temp_dir().join(format!("mrp_mbapp_test_{}_{}", std::process::id(), n));
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

fn enc(nome: &str) -> Encoding {
    Encoding::by_name(nome).unwrap()
}

/// `s` nella codifica `nome` (la scrittura e' verificata contro Python e .NET in mrp-core/tests/multibyte.rs).
fn b(nome: &str, s: &str) -> Vec<u8> {
    enc(nome).encode_str(s).unwrap()
}

/// App su `data`, terminale 40 x 10 (7 righe di testo), indice pronto; in modifica se `edit`. Ritorna anche il file.
fn app_file(dir: &TestDir, data: &[u8], e: Option<Encoding>, edit: bool) -> (App, PathBuf) {
    let p = dir.file(data);
    let cfg = AppConfig {
        encoding: e,
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

fn app(dir: &TestDir, data: &[u8], e: Option<Encoding>, edit: bool) -> App {
    app_file(dir, data, e, edit).0
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

#[test]
fn text_is_shown_and_the_cursor_steps_one_character() {
    let dir = TestDir::new();
    let sj = enc("shift-jis");
    let mut a = app(&dir, &b("shift-jis", "日本語\nabc\n"), Some(sj), false);
    let s = screen(&mut a);
    // le celle larghe sono seguite da una cella vuota nel disegno
    assert!(s[1].replace(' ', "").contains("日本語"), "{:?}", s[1]);
    assert!(s[2].contains("abc"), "{:?}", s[2]);
    assert!(
        status(&mut a).trim_end().ends_with("shift-jis"),
        "{:?}",
        status(&mut a)
    );
    for atteso in [2u64, 4, 6, 7] {
        a.handle_key(k(KeyCode::Right));
        assert_eq!(a.cursor(), atteso);
    }
    a.handle_key(k(KeyCode::Left));
    assert_eq!(a.cursor(), 6);
    a.handle_key(k(KeyCode::Left));
    assert_eq!(a.cursor(), 4);
}

#[test]
fn going_back_over_a_second_byte_that_looks_like_ascii() {
    let dir = TestDir::new();
    let sj = enc("shift-jis");
    // 表 = 95 5C, ソ = 83 5C, poi una vera barra rovesciata 5C e 'A': 0..2, 2..4, 4, 5
    let mut a = app(&dir, &b("shift-jis", "表ソ\\A\n"), Some(sj), false);
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.cursor(), 6);
    for atteso in [5u64, 4, 2, 0] {
        a.handle_key(k(KeyCode::Left));
        assert_eq!(a.cursor(), atteso);
    }
    for atteso in [2u64, 4, 5, 6] {
        a.handle_key(k(KeyCode::Right));
        assert_eq!(a.cursor(), atteso);
    }
}

#[test]
fn backspace_and_delete_remove_a_whole_character() {
    let dir = TestDir::new();
    let sj = enc("shift-jis");
    let (mut a, p) = app_file(&dir, &b("shift-jis", "表ソ\\A\n"), Some(sj), true);
    a.handle_key(k(KeyCode::Right));
    a.handle_key(k(KeyCode::Right)); // dopo ソ (4)
    a.handle_key(k(KeyCode::Backspace)); // toglie ソ (83 5C), non il solo 5C
    assert_eq!(a.document_text(), b("shift-jis", "表\\A\n"));
    assert_eq!(a.cursor(), 2);
    a.handle_key(k(KeyCode::Left)); // prima di 表
    a.handle_key(k(KeyCode::Delete)); // toglie 表 (95 5C)
    assert_eq!(a.document_text(), b("shift-jis", "\\A\n"));
    save(&mut a);
    assert_eq!(std::fs::read(&p).unwrap(), b("shift-jis", "\\A\n"));
}

#[test]
fn typing_writes_the_bytes_of_the_encoding_and_refuses_what_it_cannot_write() {
    let dir = TestDir::new();
    let sj = enc("shift-jis");
    let (mut a, p) = app_file(&dir, &b("shift-jis", "x\n"), Some(sj), true);
    type_text(&mut a, "あ\u{FF71}");
    assert_eq!(
        a.document_text(),
        [&[0x82, 0xA0, 0xB1][..], b"x\n"].concat()
    );
    // l'euro non e' in Shift_JIS: niente cambia, e il messaggio dice la codifica
    type_text(&mut a, "€");
    assert_eq!(
        a.document_text(),
        [&[0x82, 0xA0, 0xB1][..], b"x\n"].concat()
    );
    assert!(status(&mut a).contains("shift-jis"), "{:?}", status(&mut a));
    a.handle_key(k(KeyCode::Enter));
    a.handle_key(k(KeyCode::Tab));
    assert_eq!(
        a.document_text(),
        [&[0x82, 0xA0, 0xB1, b'\n', b'\t'][..], b"x\n"].concat()
    );
    save(&mut a);
    assert_eq!(
        std::fs::read(&p).unwrap(),
        [&[0x82, 0xA0, 0xB1, b'\n', b'\t'][..], b"x\n"].concat()
    );
}

#[test]
fn four_byte_characters_are_one_step() {
    let dir = TestDir::new();
    let gb = enc("gb18030");
    let (mut a, _p) = app_file(&dir, &b("gb18030", "a\u{1F600}1\n"), Some(gb), true);
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 1);
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 5);
    a.handle_key(k(KeyCode::Right)); // la cifra '1'
    assert_eq!(a.cursor(), 6);
    a.handle_key(k(KeyCode::Left));
    a.handle_key(k(KeyCode::Left));
    assert_eq!(a.cursor(), 1);
    a.handle_key(k(KeyCode::Right));
    a.handle_key(k(KeyCode::Backspace));
    assert_eq!(a.document_text(), b("gb18030", "a1\n"));
    // una faccina si digita
    type_text(&mut a, "\u{1F600}");
    assert_eq!(a.document_text(), b("gb18030", "a\u{1F600}1\n"));
}

#[test]
fn search_does_not_stop_inside_a_character() {
    let dir = TestDir::new();
    let sj = enc("shift-jis");
    // 表 = 95 5C (0..2), spazio (2), 'x' (3), barra rovesciata vera (4)
    let mut a = app(&dir, &b("shift-jis", "表 x\\\n"), Some(sj), false);
    a.handle_key(k(KeyCode::F(3)));
    type_text(&mut a, "\\");
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((4, 5)));
    assert_eq!(a.cursor(), 4);
    // un carattere intero si trova
    let mut c = app(&dir, &b("shift-jis", "ab表c\n"), Some(sj), false);
    c.handle_key(k(KeyCode::F(3)));
    type_text(&mut c, "表");
    c.handle_key(k(KeyCode::Enter));
    assert!(c.wait_idle(Duration::from_secs(20)));
    assert_eq!(c.found(), Some((2, 4)));
}

#[test]
fn switching_the_encoding_puts_the_cursor_at_the_start_of_a_character() {
    let dir = TestDir::new();
    // in UTF-8 ogni byte e' un passo: a 1 e a 3 si e' nel mezzo di un carattere Shift_JIS da 2 byte
    let dati = b("shift-jis", "日本語\n");
    let mut a = app(&dir, &dati, Some(Encoding::Utf8), false);
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 1);
    a.set_encoding(enc("shift-jis"));
    assert_eq!(a.cursor(), 0);
    let mut c = app(&dir, &dati, Some(Encoding::Utf8), false);
    for _ in 0..3 {
        c.handle_key(k(KeyCode::Right));
    }
    assert_eq!(c.cursor(), 3);
    c.set_encoding(enc("shift-jis"));
    assert_eq!(c.cursor(), 2);
    // con un secondo byte uguale a ASCII: 表 e' 95 5C, il cursore a 1 torna a 0 anche se 5C e' una barra
    let mut d = app(&dir, &b("shift-jis", "表x\n"), Some(Encoding::Utf8), false);
    d.handle_key(k(KeyCode::Right));
    d.set_encoding(enc("shift-jis"));
    assert_eq!(d.cursor(), 0);
}

#[test]
fn the_menu_lists_the_multibyte_encodings_last() {
    let dir = TestDir::new();
    let tutte = Encoding::all();
    let k_sjis = tutte.iter().position(|e| *e == enc("shift-jis")).unwrap();
    assert!(k_sjis > 50, "dopo le code page: {k_sjis}");
    let mut a = app(&dir, b"abc\n", Some(Encoding::Utf8), false);
    a.handle_key(k(KeyCode::F(9)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Encoding));
    for _ in 0..k_sjis {
        a.handle_key(k(KeyCode::Down));
    }
    assert_eq!(a.menu_index(), Some(k_sjis));
    assert!(screen(&mut a).iter().any(|l| l.contains("shift-jis")));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.encoding(), enc("shift-jis"));
    assert_eq!(status(&mut a), " encoding: shift-jis");
}

#[test]
fn copy_gives_utf8_text_and_paste_writes_the_encoding() {
    let dir = TestDir::new();
    let ek = enc("euc-kr");
    let mut a = app(&dir, &b("euc-kr", "한국어 ok\n"), Some(ek), false);
    for _ in 0..6 {
        a.handle_key(shift(KeyCode::Right));
    }
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some("한국어 ok".as_bytes().to_vec()));
    a.handle_key(k(KeyCode::Esc));
    a.handle_key(k(KeyCode::F(4)));
    a.handle_key(ctrl('v'));
    assert_eq!(a.document_text(), b("euc-kr", "한국어 ok한국어 ok\n"));
    // incollare un testo che la codifica non ha: niente cambia
    let prima = a.document_text();
    a.paste_text("\u{1F600}");
    assert_eq!(a.document_text(), prima);
}

#[test]
fn column_typing_uses_the_multibyte_encoding() {
    let dir = TestDir::new();
    let ej = enc("euc-jp");
    let mut a = app(&dir, &b("euc-jp", "日本\n語\n"), Some(ej), true);
    a.handle_key(k(KeyCode::F(8)));
    a.handle_key(shift(KeyCode::Down));
    type_text(&mut a, "あ");
    assert_eq!(a.document_text(), b("euc-jp", "あ日本\nあ語\n"));
}

#[test]
fn going_back_looks_at_the_whole_line_start_not_just_four_bytes() {
    let dir = TestDir::new();
    let sj = enc("shift-jis");
    // 82 82 | 83 83 | 41: gli ultimi 4 byte (82 83 83 41) si leggerebbero (82 83)(83 41) e l'ultimo carattere
    // sembrerebbe lungo 2; guardando l'inizio della riga e' la 'A' da sola
    let dati = [0x82, 0x82, 0x83, 0x83, 0x41, b'\n'];
    let mut a = app(&dir, &dati, Some(sj), false);
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.cursor(), 5);
    for atteso in [4u64, 2, 0] {
        a.handle_key(k(KeyCode::Left));
        assert_eq!(a.cursor(), atteso);
    }
    let mut b = app(&dir, &dati, Some(sj), true);
    b.handle_key(k(KeyCode::End));
    b.handle_key(k(KeyCode::Backspace));
    assert_eq!(b.document_text(), [0x82, 0x82, 0x83, 0x83, b'\n']);
}

#[test]
fn search_and_the_encoding_switch_also_look_back_far_enough() {
    let dir = TestDir::new();
    let sj = enc("shift-jis");
    // 82 82 | 83 83 | 83 41 (ア): la 'A' e' il secondo byte di ア
    let dati = [0x82, 0x82, 0x83, 0x83, 0x83, 0x41, b'\n'];
    let mut a = app(&dir, &dati, Some(sj), false);
    a.handle_key(k(KeyCode::F(3)));
    type_text(&mut a, "A");
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), None);
    // dal cursore a 5 (UTF-8: ogni byte e' un passo) si passa a Shift_JIS: 5 e' il secondo byte di ア, si torna a 4
    let mut c = app(&dir, &dati, Some(Encoding::Utf8), false);
    for _ in 0..5 {
        c.handle_key(k(KeyCode::Right));
    }
    assert_eq!(c.cursor(), 5);
    c.set_encoding(sj);
    assert_eq!(c.cursor(), 4);
}
