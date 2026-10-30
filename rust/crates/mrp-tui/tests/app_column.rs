//! Test della modalita' colonna del TUI (E18). Scritti da ARCH prima dell'implementazione: gli attesi sono calcolati a
//! mano sul testo di prova, mai dal codice sotto test. Il terminale e' 40 x 10 (7 righe di testo).

use mrp_core::block::ColumnBlock;
use mrp_tui::app::{App, AppConfig, MAX_VCOL};
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
        let p = std::env::temp_dir().join(format!("mrp_column_test_{}_{}", std::process::id(), n));
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

fn type_text(a: &mut App, s: &str) {
    for c in s.chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
}

/// App su `data`, in modifica se `edit`, con la modalita' colonna ACCESA (F8), cursore a 0.
fn app(dir: &TestDir, data: &[u8], edit: bool) -> App {
    let p = dir.file("f.txt", data);
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    if edit {
        a.handle_key(k(KeyCode::F(4)));
        assert!(a.is_edit());
    }
    a.handle_key(k(KeyCode::F(8)));
    assert!(a.column_mode());
    a
}

fn text(a: &App) -> String {
    String::from_utf8(a.document_text()).unwrap()
}

fn status_line(a: &mut App) -> String {
    render_lines(a, 40, 10)[8].clone()
}

/// Cursore alla colonna `col` della riga `row` (da 0), con Ctrl+Home, Down e Right senza maiuscole (in modo colonna i
/// movimenti semplici sono quelli di sempre).
fn go(a: &mut App, row: usize, col: usize) {
    a.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
    for _ in 0..row {
        a.handle_key(k(KeyCode::Down));
    }
    for _ in 0..col {
        a.handle_key(k(KeyCode::Right));
    }
}

fn block(a: &App) -> Option<(u64, u64, u64, u64)> {
    a.column_block()
        .map(|b: ColumnBlock| (b.first_row, b.last_row, b.col_a, b.col_b))
}

const TRE: &[u8] = b"abcdef\nghijkl\nmnopqr\n";

// ---------- accendere, spegnere, creare il blocco ----------

#[test]
fn f8_toggles_the_mode_and_the_title_says_so() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, false);
    assert!(a.column_mode());
    assert!(render_lines(&mut a, 40, 10)[0].contains("COL"));
    assert!(
        status_line(&mut a).contains("column mode on"),
        "{:?}",
        status_line(&mut a)
    );
    a.handle_key(k(KeyCode::F(8)));
    assert!(!a.column_mode());
    assert!(!render_lines(&mut a, 40, 10)[0].contains("COL"));
    assert!(
        status_line(&mut a).contains("column mode off"),
        "{:?}",
        status_line(&mut a)
    );
}

#[test]
fn shift_arrows_draw_a_rectangle_and_leave_the_stream_selection_empty() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, false);
    go(&mut a, 0, 1);
    assert_eq!(a.column_block(), None);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    assert_eq!(block(&a), Some((0, 0, 1, 3)));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    // righe che iniziano a 0, 7, 14; colonne [1, 3); il cursore e' in riga 2 colonna 3
    assert_eq!(block(&a), Some((0, 14, 1, 3)));
    assert_eq!(a.cursor(), 17);
    assert_eq!(a.selection(), None);
    // tornando su di una riga il blocco si accorcia
    a.handle_key(shift(KeyCode::Up));
    assert_eq!(block(&a), Some((0, 7, 1, 3)));
    // a sinistra oltre l'ancora si rovescia: colonne [0, 1)
    a.handle_key(shift(KeyCode::Left));
    a.handle_key(shift(KeyCode::Left));
    a.handle_key(shift(KeyCode::Left));
    assert_eq!(block(&a), Some((0, 7, 0, 1)));
}

#[test]
fn shift_home_and_end_set_the_cursor_column_to_the_row_start_and_end() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\nxy\n", false);
    go(&mut a, 0, 2);
    a.handle_key(shift(KeyCode::End));
    assert_eq!(block(&a), Some((0, 0, 2, 6)));
    a.handle_key(shift(KeyCode::Home));
    assert_eq!(block(&a), Some((0, 0, 0, 2)));
    // End su una riga corta: la colonna e' la larghezza di quella riga
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::End));
    assert_eq!(block(&a), Some((0, 7, 2, 2)));
}

#[test]
fn the_cursor_column_is_virtual_beyond_the_end_of_a_short_row() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\nxy\nghijkl\n", true);
    go(&mut a, 0, 5);
    a.handle_key(shift(KeyCode::Down));
    // la riga "xy" e' larga 2, ma la colonna del blocco resta 5
    assert_eq!(block(&a), Some((0, 7, 5, 5)));
    assert_eq!(a.vcol(), 5);
    type_text(&mut a, "Z");
    // solo la riga che arriva a colonna 5 cambia
    assert_eq!(text(&a), "abcdeZf\nxy\nghijkl\n");
    assert_eq!(a.vcol(), 6);
    assert_eq!(block(&a), Some((0, 8, 6, 6)));
}

#[test]
fn the_virtual_column_has_a_ceiling() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"ab\ncd\n", false);
    a.handle_key(shift(KeyCode::Down));
    for _ in 0..(MAX_VCOL + 50) {
        a.handle_key(shift(KeyCode::Right));
    }
    assert_eq!(a.vcol(), MAX_VCOL);
    assert_eq!(block(&a), Some((0, 3, 0, MAX_VCOL)));
}

#[test]
fn plain_movement_and_esc_drop_the_block_but_keep_the_mode() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, false);
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    assert!(a.column_block().is_some());
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.column_block(), None);
    assert!(a.column_mode());
    a.handle_key(shift(KeyCode::Right));
    assert!(a.column_block().is_some());
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.column_block(), None);
    // F8 spegne la modalita' e il blocco
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(k(KeyCode::F(8)));
    assert_eq!((a.column_mode(), a.column_block()), (false, None));
}

// ---------- copiare, tagliare ----------

#[test]
fn copy_gives_the_rows_of_the_block_joined_by_newlines() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, false);
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"bc\nhi\nno".to_vec()));
    assert!(
        status_line(&mut a).contains("copied 8"),
        "{:?}",
        status_line(&mut a)
    );
    assert_eq!(text(&a), "abcdef\nghijkl\nmnopqr\n");
    // senza blocco: niente di selezionato
    a.handle_key(k(KeyCode::Esc));
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), None);
    assert!(
        status_line(&mut a).contains("nothing selected"),
        "{:?}",
        status_line(&mut a)
    );
}

#[test]
fn cut_removes_the_block_in_one_undo_step() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, true);
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(ctrl('x'));
    assert_eq!(a.take_copied(), Some(b"bc\nhi\nno".to_vec()));
    assert_eq!(text(&a), "adef\ngjkl\nmpqr\n");
    // il blocco diventa di larghezza zero alla colonna 1, sulle stesse tre righe
    assert_eq!(block(&a), Some((0, 10, 1, 1)));
    a.handle_key(ctrl('z'));
    assert_eq!(text(&a), "abcdef\nghijkl\nmnopqr\n");
    assert_eq!(a.column_block(), None);
    assert!(!a.is_modified());
}

// ---------- scrivere ----------

#[test]
fn typing_replaces_the_block_on_every_row_and_goes_on_in_column() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, true);
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    type_text(&mut a, "X");
    assert_eq!(text(&a), "aXdef\ngXjkl\nmXpqr\n");
    assert_eq!(block(&a), Some((0, 12, 2, 2)));
    type_text(&mut a, "Y");
    assert_eq!(text(&a), "aXYdef\ngXYjkl\nmXYpqr\n");
    assert_eq!(a.vcol(), 3);
    // tutto in un solo passo di storia
    a.handle_key(ctrl('z'));
    assert_eq!(text(&a), "abcdef\nghijkl\nmnopqr\n");
}

#[test]
fn typing_skips_the_rows_that_do_not_reach_the_column() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcd\nx\nefgh\n", true);
    go(&mut a, 0, 3);
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    assert_eq!(block(&a), Some((0, 7, 3, 3)));
    type_text(&mut a, "#");
    assert_eq!(text(&a), "abc#d\nx\nefg#h\n");
    // Backspace con blocco largo zero toglie i caratteri che iniziano alla colonna prima
    a.handle_key(k(KeyCode::Backspace));
    assert_eq!(text(&a), "abcd\nx\nefgh\n");
    assert_eq!(a.vcol(), 3);
}

#[test]
fn delete_with_an_empty_block_removes_the_characters_that_start_at_the_column() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcd\nx\nefgh\n", true);
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(k(KeyCode::Delete));
    // "x" (colonna 0, larga 1) non ha caratteri che iniziano a 1
    assert_eq!(text(&a), "acd\nx\negh\n");
    assert_eq!(a.vcol(), 1);
    assert_eq!(block(&a), Some((0, 6, 1, 1)));
}

#[test]
fn backspace_and_delete_with_a_wide_block_remove_only_the_block() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, true);
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(k(KeyCode::Backspace));
    assert_eq!(text(&a), "adef\ngjkl\nmnopqr\n");
    assert_eq!(block(&a), Some((0, 5, 1, 1)));
    let dir2 = TestDir::new();
    let mut b = app(&dir2, TRE, true);
    go(&mut b, 0, 1);
    b.handle_key(shift(KeyCode::Right));
    b.handle_key(shift(KeyCode::Down));
    b.handle_key(k(KeyCode::Delete));
    assert_eq!(text(&b), "acdef\ngijkl\nmnopqr\n");
}

#[test]
fn tab_inserts_a_tab_on_every_row() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"ab\ncd\n", true);
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(k(KeyCode::Tab));
    assert_eq!(text(&a), "a\tb\nc\td\n");
    // il tabulatore va a colonna 4: il blocco si sposta della larghezza della tabulazione sulla riga del cursore
    assert_eq!(a.vcol(), 4);
}

#[test]
fn enter_drops_the_block_and_breaks_the_line_at_the_cursor() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcd\nefgh\n", true);
    go(&mut a, 0, 2);
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(k(KeyCode::Enter));
    // il cursore era in riga 1 colonna 2 (offset 7): li' si spezza la riga
    assert_eq!(text(&a), "abcd\nef\ngh\n");
    assert_eq!(a.column_block(), None);
    assert!(a.column_mode());
}

#[test]
fn view_mode_can_select_and_copy_but_not_write() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, false);
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    type_text(&mut a, "Z");
    assert_eq!(text(&a), "abcdef\nghijkl\nmnopqr\n");
    assert!(
        status_line(&mut a).contains("read-only"),
        "{:?}",
        status_line(&mut a)
    );
    assert!(a.column_block().is_some());
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"b\nh".to_vec()));
    a.handle_key(ctrl('x'));
    assert_eq!(text(&a), "abcdef\nghijkl\nmnopqr\n");
}

// ---------- incollare ----------

#[test]
fn paste_of_a_column_clipboard_goes_row_by_row_from_the_cursor() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"12\n34\n56\n", true);
    go(&mut a, 0, 0);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"1\n3".to_vec()));
    // niente blocco: si incolla dalla riga del cursore, alla colonna del cursore
    a.handle_key(k(KeyCode::Esc));
    go(&mut a, 0, 1);
    a.handle_key(ctrl('v'));
    assert_eq!(text(&a), "112\n334\n56\n");
    a.handle_key(ctrl('z'));
    assert_eq!(text(&a), "12\n34\n56\n");
}

#[test]
fn paste_of_one_line_goes_into_every_row_of_the_block() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"12\n34\n56\n", true);
    // copia "2" (un carattere, una sola riga)
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"2".to_vec()));
    go(&mut a, 0, 1);
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(ctrl('v'));
    assert_eq!(text(&a), "122\n324\n526\n");
}

#[test]
fn paste_with_more_lines_than_rows_is_refused_and_changes_nothing() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"12\n34\n56\n", true);
    go(&mut a, 0, 0);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(ctrl('c'));
    assert_eq!(a.take_copied(), Some(b"1\n3\n5".to_vec()));
    a.handle_key(k(KeyCode::Esc));
    go(&mut a, 2, 0);
    a.handle_key(ctrl('v'));
    assert_eq!(text(&a), "12\n34\n56\n");
    assert!(
        status_line(&mut a).contains("not enough rows"),
        "{:?}",
        status_line(&mut a)
    );
    assert!(!a.is_modified());
}

#[test]
fn terminal_paste_in_column_mode_behaves_like_ctrl_v() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"12\n34\n56\n", true);
    go(&mut a, 0, 1);
    a.paste_text("a\nb");
    assert_eq!(text(&a), "1a2\n3b4\n56\n");
}

// ---------- disegno ----------

fn styles(a: &mut App) -> Vec<Style> {
    let mut term = Terminal::new(TestBackend::new(40, 10)).unwrap();
    term.draw(|f| draw(f, a)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = Vec::new();
    for y in 1..=7u16 {
        for x in 0..40u16 {
            out.push(buf[(x, y)].style());
        }
    }
    out
}

#[test]
fn the_block_is_drawn_cell_by_cell_and_short_rows_have_fewer_cells() {
    for id in ThemeId::ALL {
        let dir = TestDir::new();
        let p = dir.file("f.txt", b"abcdef\nx\nmnopqr\n");
        let cfg = AppConfig {
            theme: id,
            ..AppConfig::default()
        };
        let mut a = App::open(&p, cfg).unwrap();
        a.resize(40, 10);
        assert!(a.wait_idle(Duration::from_secs(20)));
        a.handle_key(k(KeyCode::F(8)));
        a.handle_key(k(KeyCode::Right));
        let senza = styles(&mut a);
        a.handle_key(shift(KeyCode::Right));
        a.handle_key(shift(KeyCode::Right));
        a.handle_key(shift(KeyCode::Down));
        a.handle_key(shift(KeyCode::Down));
        // colonne [1, 3): "bc" e "no" -> 4 celle; la riga "x" non ha celle a quelle colonne
        let con = styles(&mut a);
        let diverse = senza.iter().zip(&con).filter(|(x, y)| x != y).count();
        assert_eq!(diverse, 4, "tema {id:?}");
    }
}

// ---------- casi di bordo aggiunti dopo le mutazioni ----------

#[test]
fn turning_the_mode_off_and_on_does_not_bring_the_old_block_back() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, false);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Down));
    assert!(a.column_block().is_some());
    a.handle_key(k(KeyCode::F(8)));
    a.handle_key(k(KeyCode::F(8)));
    assert!(a.column_mode());
    assert_eq!(a.column_block(), None);
}

#[test]
fn the_block_grows_upwards_when_the_cursor_is_on_its_first_row() {
    let dir = TestDir::new();
    let mut a = app(&dir, TRE, false);
    go(&mut a, 2, 1);
    a.handle_key(shift(KeyCode::Right));
    a.handle_key(shift(KeyCode::Up));
    assert_eq!(block(&a), Some((7, 14, 1, 2)));
    a.handle_key(shift(KeyCode::Up));
    assert_eq!(block(&a), Some((0, 14, 1, 2)));
    // e giu' di nuovo: il cursore e' in cima, il blocco si accorcia da sopra
    a.handle_key(shift(KeyCode::Down));
    assert_eq!(block(&a), Some((7, 14, 1, 2)));
    a.handle_key(shift(KeyCode::Down));
    assert_eq!(block(&a), Some((14, 14, 1, 2)));
}

#[test]
fn backspace_at_column_zero_does_nothing() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"ab\ncd\n", true);
    a.handle_key(shift(KeyCode::Down));
    a.handle_key(k(KeyCode::Backspace));
    assert_eq!(text(&a), "ab\ncd\n");
    assert!(!a.is_modified());
}

#[test]
fn a_trailing_newline_in_the_pasted_text_adds_no_row() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"12\n34\n", true);
    a.paste_text("x\ny\n");
    assert_eq!(text(&a), "x12\ny34\n");
}

#[test]
fn a_stream_selection_made_before_the_mode_does_not_survive_it() {
    let dir = TestDir::new();
    let p = dir.file("f.txt", TRE);
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.handle_key(shift(KeyCode::Right));
    assert!(a.selection().is_some());
    a.handle_key(k(KeyCode::F(8)));
    assert_eq!(a.selection(), None);
    a.handle_key(k(KeyCode::F(8)));
    assert_eq!(a.selection(), None);
}
