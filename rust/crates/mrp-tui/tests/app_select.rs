//! Test della selezione, degli appunti e dell'incolla del TUI (E17). Scritti da ARCH prima dell'implementazione: gli
//! attesi sono calcolati a mano sul testo di prova, mai dal codice sotto test.

use mrp_tui::app::{App, AppConfig, COPY_LIMIT};
use mrp_tui::theme::ThemeId;
use mrp_tui::ui::{draw, render_lines};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Style;
use ratatui::Terminal;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_select_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, nome: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(nome);
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

fn ctrl_shift(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::CONTROL | KeyModifiers::SHIFT)
}

fn type_text(a: &mut App, s: &str) {
    for c in s.chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
}

/// App su `data`, terminale 40 x 10 (7 righe di testo), indice pronto; in modifica se `edit`.
fn app(dir: &TestDir, data: &[u8], edit: bool) -> App {
    let p = dir.file("f.txt", data);
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    if edit {
        a.handle_key(k(KeyCode::F(4)));
        assert!(a.is_edit());
    }
    a
}

fn text(a: &App) -> String {
    String::from_utf8(a.document_text()).unwrap()
}

fn status_line(a: &mut App) -> String {
    render_lines(a, 40, 9)[7].clone()
}

/// Muove il cursore a `pos` con Right (da 0): i test partono dal byte 0.
fn go(a: &mut App, pos: u64) {
    a.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
    for _ in 0..pos {
        a.handle_key(k(KeyCode::Right));
    }
    assert_eq!(a.cursor(), pos);
}

// ---------- selezione con la tastiera ----------

#[test]
fn shift_right_and_left_extend_and_shrink_the_selection() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", false);
    assert_eq!(a.selection(), None);
    for _ in 0..3 {
        a.handle_key(shift(KeyCode::Right));
    }
    assert_eq!((a.cursor(), a.selection()), (3, Some((0, 3))));
    a.handle_key(shift(KeyCode::Left));
    assert_eq!((a.cursor(), a.selection()), (2, Some((0, 2))));
    // tornando all'ancora la selezione e' vuota: non c'e'
    a.handle_key(shift(KeyCode::Left));
    a.handle_key(shift(KeyCode::Left));
    assert_eq!((a.cursor(), a.selection()), (0, None));
    // andare oltre l'ancora la rovescia: selection() e' sempre (inizio, fine)
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    assert_eq!(a.selection(), Some((0, 2)));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!((a.cursor(), a.selection()), (2, None));
    // dopo Esc la selezione riparte da dove sta il cursore: andando a sinistra si rovescia
    a.handle_key(shift(KeyCode::Left));
    a.handle_key(shift(KeyCode::Left));
    assert_eq!((a.cursor(), a.selection()), (0, Some((0, 2))));
}

#[test]
fn plain_arrows_collapse_the_selection_to_its_ends() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\nghi\n", false);
    go(&mut a, 1);
    for _ in 0..3 {
        a.handle_key(shift(KeyCode::Right));
    }
    assert_eq!(a.selection(), Some((1, 4)));
    // Left porta il cursore all'inizio della selezione, Right alla fine
    a.handle_key(k(KeyCode::Left));
    assert_eq!((a.cursor(), a.selection()), (1, None));
    for _ in 0..3 {
        a.handle_key(shift(KeyCode::Right));
    }
    a.handle_key(k(KeyCode::Right));
    assert_eq!((a.cursor(), a.selection()), (4, None));
    // Down/Up/Home/End azzerano la selezione e si muovono dal cursore
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.selection(), None);
    assert_eq!(a.cursor(), 10); // riga "ghi" (byte 7..10): colonna ricordata 5 -> fine riga
}

#[test]
fn shift_down_up_home_end_pgdn_extend_over_rows() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"ab\ncd\nef\ngh\n", false);
    go(&mut a, 1);
    a.handle_key(shift(KeyCode::Down));
    assert_eq!(a.selection(), Some((1, 4)));
    a.handle_key(shift(KeyCode::Down));
    assert_eq!(a.selection(), Some((1, 7)));
    a.handle_key(shift(KeyCode::Up));
    a.handle_key(shift(KeyCode::Up));
    assert_eq!(a.selection(), None);
    a.handle_key(shift(KeyCode::End));
    assert_eq!(a.selection(), Some((1, 2)));
    // l'ancora e' in 1: andare a inizio riga la rovescia
    a.handle_key(shift(KeyCode::Home));
    assert_eq!(a.selection(), Some((0, 1)));
    // Maiusc+Home dal mezzo di una riga seleziona fino all'inizio riga
    go(&mut a, 4);
    a.handle_key(shift(KeyCode::Home));
    assert_eq!(a.selection(), Some((3, 4)));
    // Esc toglie l'ancora ma il cursore resta dov'e' (a 3, anche se la selezione era rovesciata); Ctrl+Maiusc+End
    // seleziona fino alla fine del documento (in lettura il cursore si ferma prima dell'ultimo a-capo)
    a.handle_key(k(KeyCode::Esc));
    assert_eq!((a.cursor(), a.selection()), (3, None));
    a.handle_key(ctrl_shift(KeyCode::End));
    assert_eq!(a.selection(), Some((3, 11)));
    a.handle_key(ctrl_shift(KeyCode::Home));
    assert_eq!(a.selection(), Some((0, 3)));
}

#[test]
fn ctrl_a_selects_everything_and_esc_clears_it() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"ab\ncd\n", true);
    a.handle_key(ctrl('a'));
    // in modifica il cursore puo' stare a fine documento
    assert_eq!(a.selection(), Some((0, 6)));
    assert_eq!(a.cursor(), 6);
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.selection(), None);
    let dir2 = TestDir::new();
    let mut v = app(&dir2, b"ab\ncd\n", false);
    v.handle_key(ctrl('a'));
    // in lettura il cursore non supera len - 1: l'ultimo a-capo non e' selezionabile
    assert_eq!(v.selection(), Some((0, 5)));
}

#[test]
fn a_successful_goto_clears_the_selection() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", true);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    assert!(a.selection().is_some());
    a.handle_key(ctrl('g'));
    a.handle_key(k(KeyCode::Esc));
    // il dialogo lascia la selezione com'e' se non si sposta nulla; un goto riuscito la azzera
    a.handle_key(ctrl('g'));
    type_text(&mut a, "@3");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.selection(), None);
    assert_eq!(a.cursor(), 3);
}

// ---------- copia, taglia, incolla ----------

#[test]
fn copy_puts_the_selected_bytes_in_the_clipboard_and_works_in_view_mode() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", false);
    go(&mut a, 1);
    for _ in 0..3 {
        a.handle_key(shift(KeyCode::Right));
    }
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"bcd".to_vec()));
    // una volta sola
    assert_eq!(a.take_copied(), None);
    assert!(
        status_line(&mut a).contains("copied 3"),
        "{:?}",
        status_line(&mut a)
    );
    // la selezione resta, il testo non cambia
    assert_eq!(a.selection(), Some((1, 4)));
    assert_eq!(text(&a), "abcdef\n");
    assert!(!a.is_modified());
}

#[test]
fn copy_without_a_selection_says_so() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", false);
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), None);
    assert!(
        status_line(&mut a).contains("nothing selected"),
        "{:?}",
        status_line(&mut a)
    );
}

#[test]
fn copy_spans_rows_with_their_newlines() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"ab\ncd\nef\n", false);
    go(&mut a, 1);
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    assert_eq!(a.selection(), Some((1, 7)));
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"b\ncd\ne".to_vec()));
}

#[test]
fn cut_removes_the_selection_in_one_undo_step() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", true);
    go(&mut a, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(ctrl('x'));
    assert_eq!(a.take_copied(), Some(b"bc".to_vec()));
    assert_eq!(text(&a), "adef\n");
    assert_eq!((a.cursor(), a.selection()), (1, None));
    assert!(a.is_modified());
    a.handle_key(ctrl('z'));
    assert_eq!(text(&a), "abcdef\n");
    assert!(!a.is_modified());
}

#[test]
fn cut_in_view_mode_is_refused() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", false);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(ctrl('x'));
    assert_eq!(text(&a), "abcdef\n");
    assert_eq!(a.take_copied(), None);
    assert!(
        status_line(&mut a).contains("read-only"),
        "{:?}",
        status_line(&mut a)
    );
}

#[test]
fn paste_inserts_the_clipboard_at_the_cursor_and_replaces_a_selection() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", true);
    // vuoti: messaggio
    a.handle_key(ctrl('v'));
    assert!(
        status_line(&mut a).contains("clipboard is empty"),
        "{:?}",
        status_line(&mut a)
    );
    go(&mut a, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(ctrl('c'));
    // incolla in coda alla riga (nessuna selezione): "abcdefbc"
    a.handle_key(k(KeyCode::End));
    a.handle_key(ctrl('v'));
    assert_eq!(text(&a), "abcdefbc\n");
    assert_eq!(a.cursor(), 8);
    // incolla sopra una selezione: la sostituisce
    go(&mut a, 0);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(ctrl('v'));
    assert_eq!(text(&a), "bcdefbc\n");
    assert_eq!((a.cursor(), a.selection()), (2, None));
    // un solo annulla per l'ultima incolla (sostituzione compresa)
    a.handle_key(ctrl('z'));
    assert_eq!(text(&a), "abcdefbc\n");
}

#[test]
fn paste_in_view_mode_is_refused() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", false);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(ctrl('c'));
    a.handle_key(ctrl('v'));
    a.paste_text("zzz");
    assert_eq!(text(&a), "abc\n");
    assert!(!a.is_modified());
}

#[test]
fn paste_text_from_the_terminal_inserts_and_replaces_the_selection() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", true);
    go(&mut a, 2);
    a.paste_text("XY\nZ");
    assert_eq!(text(&a), "abXY\nZcdef\n");
    assert_eq!(a.cursor(), 6);
    // sostituisce una selezione, in un passo solo
    go(&mut a, 0);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.paste_text("--");
    assert_eq!(text(&a), "--XY\nZcdef\n");
    a.handle_key(ctrl('z'));
    assert_eq!(text(&a), "abXY\nZcdef\n");
    // testo vuoto: niente
    let prima = text(&a);
    a.paste_text("");
    assert_eq!(text(&a), prima);
}

#[test]
fn a_copy_over_the_limit_is_refused() {
    // un file appena piu' grande del limite: il Ctrl+A + Ctrl+C non deve costruire una copia in memoria
    let dir = TestDir::new();
    let blocco = vec![b'x'; 1 << 20];
    let p = dir.0.join("grande.txt");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&p).unwrap();
        for _ in 0..(COPY_LIMIT >> 20) + 1 {
            f.write_all(&blocco).unwrap();
        }
    }
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 9);
    a.handle_key(ctrl('a'));
    assert!(a.selection().is_some());
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), None);
    assert!(
        status_line(&mut a).contains("too big"),
        "{:?}",
        status_line(&mut a)
    );
}

// ---------- scrivere sopra una selezione ----------

#[test]
fn typing_replaces_the_selection_in_one_undo_step() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", true);
    go(&mut a, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    type_text(&mut a, "XY");
    assert_eq!(text(&a), "aXYdef\n");
    assert_eq!((a.cursor(), a.selection()), (3, None));
    a.handle_key(ctrl('z'));
    assert_eq!(text(&a), "abcdef\n");
}

#[test]
fn enter_tab_backspace_and_delete_act_on_the_selection_only() {
    let dir = TestDir::new();
    // Backspace e Delete tolgono la selezione e basta (non un carattere in piu')
    let mut a = app(&dir, b"abcdef\n", true);
    go(&mut a, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(k(KeyCode::Backspace));
    assert_eq!((text(&a), a.cursor()), ("adef\n".to_string(), 1));
    go(&mut a, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(k(KeyCode::Delete));
    assert_eq!((text(&a), a.cursor()), ("aef\n".to_string(), 1));
    // Enter e Tab sostituiscono la selezione
    go(&mut a, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(text(&a), "a\nf\n");
    go(&mut a, 2);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(k(KeyCode::Tab));
    assert_eq!(text(&a), "a\n\t\n");
}

#[test]
fn typing_over_a_selection_in_view_mode_is_refused_and_keeps_it() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\n", false);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(k(KeyCode::Char('Z')));
    assert_eq!(text(&a), "abcdef\n");
    assert_eq!(a.selection(), Some((0, 1)));
}

#[test]
fn a_multibyte_selection_is_cut_whole() {
    let dir = TestDir::new();
    let mut a = app(&dir, "a\u{e9}\u{65e5}b\n".as_bytes(), true);
    a.handle_key(k(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    // 'e'' (2 byte) e il carattere largo (3 byte): selezione 1..6
    assert_eq!(a.selection(), Some((1, 6)));
    a.handle_key(ctrl('x'));
    assert_eq!(a.take_copied(), Some("\u{e9}\u{65e5}".as_bytes().to_vec()));
    assert_eq!(text(&a), "ab\n");
}

// ---------- disegno ----------

/// Gli stili delle celle delle righe di testo (le righe 1..=7 di un terminale 40 x 10), per colonna e riga.
fn styles(a: &mut App) -> Vec<Style> {
    let mut term = Terminal::new(TestBackend::new(40, 9)).unwrap();
    term.draw(|f| draw(f, a)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = Vec::new();
    for y in 0..=6u16 {
        for x in 0..40u16 {
            out.push(buf[(x, y)].style());
        }
    }
    out
}

#[test]
fn the_selection_is_drawn_with_its_own_style_on_the_selected_cells_only() {
    for id in ThemeId::ALL {
        let dir = TestDir::new();
        let p = dir.file("f.txt", b"abcdef\nghijkl\n");
        let cfg = AppConfig {
            theme: id,
            ..AppConfig::default()
        };
        let mut a = App::open(&p, cfg).unwrap();
        a.resize(40, 9);
        assert!(a.wait_idle(Duration::from_secs(20)));
        let senza = styles(&mut a);
        // "bcdef" della prima riga e "gh" della seconda: 5 + 2 celle (gli a-capo non hanno celle)
        a.handle_key(k(KeyCode::Right));
        // 8 passi: "bcdef", l'a-capo, "gh"
        for _ in 0..8 {
            a.handle_key(shift(KeyCode::Right));
        }
        assert_eq!(a.selection(), Some((1, 9)));
        let con = styles(&mut a);
        let diverse = senza.iter().zip(&con).filter(|(x, y)| x != y).count();
        assert_eq!(diverse, 5 + 2, "tema {id:?}");
        // il disegno non cambia il testo
        assert!(render_lines(&mut a, 40, 9)[0].contains("abcdef"));
    }
}

#[test]
fn two_pastes_in_a_row_are_two_undo_steps() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abc\n", true);
    a.paste_text("1");
    a.paste_text("2");
    assert_eq!(text(&a), "12abc\n");
    a.handle_key(ctrl('z'));
    assert_eq!(text(&a), "1abc\n");
    a.handle_key(ctrl('z'));
    assert_eq!(text(&a), "abc\n");
}
