//! Test della ricerca nel TUI (F3 / Ctrl+F, F3 / Shift+F3). Scritti da ARCH prima dell'implementazione.
//! Il file di prova ha 30 righe "line NN\n" da 8 byte: "line 07" sta all'offset 48, "line 10" a 72, "line 11" a 80.

use mrp_core::lang::Lang;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::theme::{Theme, ThemeId};
use mrp_tui::ui::{draw, render_lines};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_find_test_{}_{}", std::process::id(), n));
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

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn shift_f3() -> KeyEvent {
    KeyEvent::new(KeyCode::F(3), KeyModifiers::SHIFT)
}

fn type_text(a: &mut App, s: &str) {
    for c in s.chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
}

fn thirty() -> Vec<u8> {
    (1..=30)
        .map(|i| format!("line {i:02}\n"))
        .collect::<String>()
        .into_bytes()
}

fn open(dir: &TestDir, nome: &str, data: &[u8], index: bool) -> App {
    let p = dir.file(nome, data);
    let mut a = App::open(
        &p,
        AppConfig {
            index,
            ..AppConfig::default()
        },
    )
    .unwrap();
    a.resize(80, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a
}

fn app(dir: &TestDir) -> App {
    open(dir, "f.txt", &thirty(), true)
}

/// Cerca `testo` da prompt (F3 senza ricerca precedente) e aspetta la fine.
fn find(a: &mut App, testo: &str) {
    a.handle_key(k(KeyCode::F(3)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Find));
    type_text(a, testo);
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
}

fn screen(a: &mut App) -> Vec<String> {
    render_lines(a, 80, 24)
        .into_iter()
        .map(|l| l.trim_end().to_string())
        .collect()
}

fn status(a: &mut App) -> String {
    screen(a)[22].clone()
}

#[test]
fn f3_asks_for_the_text_and_goes_to_the_first_match_after_the_cursor() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    a.handle_key(k(KeyCode::F(3)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Find));
    let en = Lang::english();
    let righe = screen(&mut a);
    for key in ["prompt.find.title", "prompt.find.hint", "prompt.keys"] {
        let atteso = en.get(key).unwrap();
        assert!(
            righe.iter().any(|l| l.contains(atteso.trim_end())),
            "manca {key}"
        );
    }
    assert!(
        a.cursor_xy().is_some(),
        "nel prompt il cursore del terminale si vede"
    );
    type_text(&mut a, "line 07");
    assert!(righe.iter().all(|l| !l.contains("> line 07")));
    assert!(screen(&mut a).iter().any(|l| l.contains("> line 07")));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.cursor(), 48);
    assert_eq!(status(&mut a), " Ln 7  Col 1  byte 48  20%");
}

#[test]
fn the_search_starts_after_the_cursor_not_on_it() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    // "line 1" non c'e' all'offset 0 ("line 01"); la prima e' "line 10" a 72, poi "line 11" a 80
    find(&mut a, "line 1");
    assert_eq!(a.cursor(), 72);
    // F3 senza prompt: la prossima occorrenza dopo il cursore, non quella sotto il cursore
    a.handle_key(k(KeyCode::F(3)));
    assert_eq!(a.dialog_kind(), None);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.cursor(), 80);
}

#[test]
fn shift_f3_goes_to_the_previous_match() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    find(&mut a, "line 1");
    a.handle_key(k(KeyCode::F(3)));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.cursor(), 80);
    a.handle_key(shift_f3());
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.cursor(), 72);
}

#[test]
fn the_search_wraps_around_and_says_so() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    a.handle_key(ctrl_end());
    assert_eq!(a.cursor(), 239);
    find(&mut a, "line 05");
    assert_eq!(a.cursor(), 32, "ripartita dall'inizio");
    assert_eq!(status(&mut a), " search wrapped around");
    // in avanti dalla sola occorrenza: torna su se' stessa, sempre con l'avviso
    a.handle_key(k(KeyCode::F(3)));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.cursor(), 32);
    assert_eq!(status(&mut a), " search wrapped around");
    // all'indietro dalla sola occorrenza: idem
    a.handle_key(shift_f3());
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.cursor(), 32);
    assert_eq!(status(&mut a), " search wrapped around");
}

fn ctrl_end() -> KeyEvent {
    KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL)
}

#[test]
fn not_found_says_so_and_does_not_move() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    a.handle_key(k(KeyCode::Down));
    let prima = a.cursor();
    find(&mut a, "zzz");
    assert_eq!(a.cursor(), prima);
    assert_eq!(status(&mut a), " not found: zzz");
    // il messaggio sparisce al primo tasto
    a.handle_key(k(KeyCode::Right));
    assert_ne!(status(&mut a), " not found: zzz");
}

#[test]
fn escape_and_an_empty_text_close_the_prompt_without_searching() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    a.handle_key(k(KeyCode::F(3)));
    type_text(&mut a, "line 07");
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.cursor(), 0);
    assert_eq!(status(&mut a), " Ln 1  Col 1  byte 0  0%");
    a.handle_key(k(KeyCode::F(3)));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(status(&mut a), " Ln 1  Col 1  byte 0  0%");
    // senza una ricerca fatta, F3 riapre ancora il prompt
    a.handle_key(k(KeyCode::F(3)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Find));
}

#[test]
fn ctrl_f_reopens_the_prompt_with_the_last_text() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    find(&mut a, "line 1");
    assert_eq!(a.cursor(), 72);
    a.handle_key(ctrl('f'));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Find));
    assert!(screen(&mut a).iter().any(|l| l.contains("> line 1")));
    // il testo si puo' proseguire: "line 1" + "2" = "line 12"
    type_text(&mut a, "2");
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.cursor(), 88, "line 12");
}

#[test]
fn the_match_is_drawn_with_the_found_style() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    find(&mut a, "line 10");
    assert_eq!(a.found(), Some((72, 79)));
    // disegnata: le 7 celle del testo trovato (riga 10 dello schermo, dopo il gutter di 4 colonne) hanno lo stile `found`
    let t = Theme::for_id(ThemeId::DEFAULT);
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| draw(f, &mut a)).unwrap();
    let buf = term.backend().buffer().clone();
    for x in 4..11u16 {
        assert_eq!(buf[(x, 10)].bg, t.found.bg.unwrap(), "x={x}");
    }
    assert_ne!(buf[(11, 10)].bg, t.found.bg.unwrap());
    assert_ne!(buf[(4, 9)].bg, t.found.bg.unwrap());
}

#[test]
fn a_key_after_a_match_clears_the_highlight() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    find(&mut a, "line 1");
    assert_eq!(a.found(), Some((72, 78)));
    a.handle_key(k(KeyCode::F(3)));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(
        a.found(),
        Some((80, 86)),
        "F3 tiene l'evidenziazione sulla nuova occorrenza"
    );
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.found(), None);
}

#[test]
fn a_match_out_of_view_is_centered_like_goto() {
    let dir = TestDir::new();
    let mut a = app(&dir);
    a.resize(80, 10); // 7 righe di testo
    find(&mut a, "line 20");
    // la riga 20 inizia a 152 e non era visibile: va "al centro", top = inizio della riga 20 - 7/2 = 17, cioe' 16 * 8 = 128
    assert_eq!(a.cursor(), 152);
    assert_eq!(a.top(), 128);
}

#[test]
fn it_works_without_the_line_index_and_on_a_modified_document() {
    let dir = TestDir::new();
    let mut a = open(&dir, "g.txt", &thirty(), false);
    find(&mut a, "line 07");
    assert_eq!(a.cursor(), 48);
    assert_eq!(status(&mut a), " Ln ?  Col 1  byte 48  20%");

    // in modifica: due caratteri all'inizio spostano tutto di 2 byte e la ricerca li trova
    let mut b = app(&dir);
    b.handle_key(k(KeyCode::F(4)));
    type_text(&mut b, "ZZ");
    assert_eq!(b.cursor(), 2);
    find(&mut b, "line 02");
    assert_eq!(b.cursor(), 10);
    b.handle_key(ctrl('f'));
    for _ in 0..7 {
        b.handle_key(k(KeyCode::Backspace));
    }
    type_text(&mut b, "ZZ");
    b.handle_key(k(KeyCode::Enter));
    assert!(b.wait_idle(Duration::from_secs(20)));
    assert_eq!(b.cursor(), 0, "ZZ sta all'inizio: ripartita da capo");
    assert_eq!(status(&mut b), " search wrapped around");
}

#[test]
fn the_text_is_searched_as_exact_utf8_bytes() {
    let dir = TestDir::new();
    let mut a = open(
        &dir,
        "u.txt",
        "caff\u{e8}\nna\u{ef}ve\nCAFF\u{c8}\n".as_bytes(),
        true,
    );
    find(&mut a, "\u{e8}");
    assert_eq!(a.cursor(), 4);
    // le maiuscole contano: "CAFF" sta alla terza riga, dopo 7 + 7 byte
    a.handle_key(ctrl('f'));
    a.handle_key(k(KeyCode::Backspace));
    type_text(&mut a, "CAFF");
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.cursor(), 14);
}
