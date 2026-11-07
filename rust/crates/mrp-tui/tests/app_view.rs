//! Test dell'app in modalita' lettura: tastiera, schermo, indice righe in background. Scritti da ARCH prima
//! dell'implementazione: gli schermi attesi sono scritti a mano (contratto di layout in `docs/TEAM/DESIGN_TUI.md`).

use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_app_test_{}_{}", std::process::id(), n));
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

fn ch(c: char) -> KeyEvent {
    k(KeyCode::Char(c))
}

fn ctrl(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::CONTROL)
}

fn wheel(down: bool) -> MouseEvent {
    MouseEvent {
        kind: if down {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        },
        column: 5,
        row: 3,
        modifiers: KeyModifiers::NONE,
    }
}

/// App su `data` con un terminale `w` x `h`, indice righe pronto.
fn app(dir: &TestDir, data: &[u8], w: u16, h: u16) -> App {
    let p = dir.file("f.txt", data);
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(w, h);
    assert!(
        a.wait_idle(Duration::from_secs(20)),
        "l'indice righe non e' finito"
    );
    a
}

/// Le righe dello schermo senza gli spazi in fondo.
fn screen(a: &mut App, w: u16, h: u16) -> Vec<String> {
    render_lines(a, w, h)
        .into_iter()
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// 30 righe "line 01\n" .. "line 30\n": 240 byte.
fn thirty() -> Vec<u8> {
    (1..=30)
        .map(|i| format!("line {i:02}\n"))
        .collect::<String>()
        .into_bytes()
}

// ---------- apertura e schermo iniziale ----------

#[test]
fn open_fails_for_a_missing_file_and_for_a_directory() {
    let dir = TestDir::new();
    assert!(App::open(&dir.0.join("non_esiste"), AppConfig::default()).is_err());
    assert!(App::open(&dir.0, AppConfig::default()).is_err());
}

#[test]
fn the_first_screen() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 30, 7);
    // niente titolo: il testo comincia alla prima riga (nome, dimensione e modo stanno a destra della riga di stato, che a 30
    // colonne non li contiene); 3 colonne di numeri (len = 240) + '|'
    assert_eq!(
        screen(&mut a, 30, 7),
        vec![
            "  1│line 01".to_string(),
            "  2│line 02".to_string(),
            "  3│line 03".to_string(),
            "  4│line 04".to_string(),
            "  5│line 05".to_string(),
            " Ln 1  Col 1  byte 0  0%".to_string(),
            " Alt+V View  Alt+E Edit  Alt+T".to_string(),
        ]
    );
    assert_eq!((a.top(), a.cursor()), (0, 0));
    assert!(!a.is_edit());
    assert!(!a.should_quit());
}

#[test]
fn without_the_index_line_numbers_are_unknown() {
    let dir = TestDir::new();
    let p = dir.file("f.txt", &thirty());
    let cfg = AppConfig {
        index: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(30, 7);
    let s = screen(&mut a, 30, 7);
    // senza l'indice righe il margine dei numeri non c'e' proprio
    assert_eq!(s[0], "line 01");
    assert_eq!(s[5], " Ln ?  Col 1  byte 0  0%");
}

#[test]
fn an_empty_file() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"", 30, 5);
    assert_eq!(
        screen(&mut a, 30, 5),
        vec![
            String::new(),
            String::new(),
            String::new(),
            " Ln 1  Col 1  byte 0  0%".to_string(),
            " Alt+V View  Alt+E Edit  Alt+T".to_string(),
        ]
    );
}

// ---------- spostamenti ----------

#[test]
fn down_and_up_move_the_cursor_by_rows() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 30, 7);
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 16);
    assert_eq!(screen(&mut a, 30, 7)[5], " Ln 3  Col 1  byte 16  6%");
    a.handle_key(k(KeyCode::Up));
    assert_eq!(a.cursor(), 8);
    a.handle_key(k(KeyCode::Up));
    a.handle_key(k(KeyCode::Up));
    assert_eq!(a.cursor(), 0, "in cima non si esce");
}

#[test]
fn moving_below_the_last_visible_row_scrolls() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 30, 7); // 5 righe di testo
    for _ in 0..5 {
        a.handle_key(k(KeyCode::Down));
    }
    assert_eq!(a.cursor(), 40); // riga 6
    assert_eq!(
        a.top(),
        8,
        "la riga 6 e' l'ultima visibile: la prima e' la 2"
    );
    let s = screen(&mut a, 30, 7);
    assert_eq!(s[0], "  2│line 02");
    assert_eq!(s[4], "  6│line 06");
    assert_eq!(s[5], " Ln 6  Col 1  byte 40  16%");
    // e tornando su oltre la cima della finestra
    for _ in 0..5 {
        a.handle_key(k(KeyCode::Up));
    }
    assert_eq!(a.cursor(), 0);
    assert_eq!(a.top(), 0);
}

#[test]
fn left_and_right_cross_the_line_ends() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 30, 7);
    for _ in 0..7 {
        a.handle_key(k(KeyCode::Right));
    }
    assert_eq!(a.cursor(), 7, "a fine riga, prima dell'a-capo");
    assert_eq!(screen(&mut a, 30, 7)[5], " Ln 1  Col 8  byte 7  2%");
    a.handle_key(k(KeyCode::Right));
    assert_eq!(a.cursor(), 8, "oltre l'a-capo: inizio della riga dopo");
    a.handle_key(k(KeyCode::Left));
    assert_eq!(a.cursor(), 7);
    a.handle_key(k(KeyCode::Left));
    assert_eq!(a.cursor(), 6);
    a.handle_key(k(KeyCode::Home));
    assert_eq!(a.cursor(), 0);
    a.handle_key(k(KeyCode::Left));
    assert_eq!(a.cursor(), 0, "all'inizio del file non si esce");
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.cursor(), 7);
}

#[test]
fn the_column_is_remembered_across_short_rows() {
    let dir = TestDir::new();
    let mut a = app(&dir, b"abcdef\nab\nabcdef\n", 30, 7);
    for _ in 0..5 {
        a.handle_key(k(KeyCode::Right));
    }
    assert_eq!(a.cursor(), 5);
    a.handle_key(k(KeyCode::Down));
    assert_eq!(
        a.cursor(),
        9,
        "la riga 'ab' e' corta: fine riga (offset 7+2)"
    );
    a.handle_key(k(KeyCode::Down));
    assert_eq!(
        a.cursor(),
        15,
        "la colonna 5 torna sulla riga lunga (10 + 5)"
    );
    a.handle_key(k(KeyCode::Up));
    a.handle_key(k(KeyCode::Up));
    assert_eq!(a.cursor(), 5);
    // un movimento orizzontale azzera la colonna ricordata
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Left));
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 11, "Left dalla fine della riga corta (colonna 1), poi Down: colonna 1 della terza riga (10 + 1)");
}

#[test]
fn page_down_and_page_up_move_by_the_visible_rows_minus_one() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 30, 7); // 5 righe di testo, pagina = 4
    a.handle_key(k(KeyCode::PageDown));
    assert_eq!(
        (a.top(), a.cursor()),
        (32, 32),
        "la riga 5 in cima, il cursore sulla stessa riga dello schermo"
    );
    let s = screen(&mut a, 30, 7);
    assert_eq!(s[0], "  5│line 05");
    assert_eq!(s[4], "  9│line 09");
    assert_eq!(s[5], " Ln 5  Col 1  byte 32  13%");
    a.handle_key(k(KeyCode::PageUp));
    assert_eq!((a.top(), a.cursor()), (0, 0));
    a.handle_key(k(KeyCode::PageUp));
    assert_eq!((a.top(), a.cursor()), (0, 0), "in cima non si esce");
}

#[test]
fn ctrl_end_and_ctrl_home() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 30, 7);
    a.handle_key(ctrl(KeyCode::End));
    assert_eq!(
        a.cursor(),
        239,
        "in lettura il cursore non va oltre l'ultimo a-capo"
    );
    assert_eq!(
        a.top(),
        200,
        "ultima riga in fondo allo schermo: la prima visibile e' la 26"
    );
    let s = screen(&mut a, 30, 7);
    assert_eq!(s[0], " 26│line 26");
    assert_eq!(s[4], " 30│line 30");
    assert_eq!(s[5], " Ln 30  Col 8  byte 239  99%");
    a.handle_key(ctrl(KeyCode::Home));
    assert_eq!((a.top(), a.cursor()), (0, 0));
}

#[test]
fn long_rows_scroll_horizontally() {
    let dir = TestDir::new();
    let riga = "0123456789".repeat(6); // 60 caratteri
    let mut a = app(&dir, format!("{riga}\nx\n").as_bytes(), 30, 7); // len 63 -> 2 cifre + '|'; 27 celle di testo
    let s = screen(&mut a, 30, 7);
    assert_eq!(
        s[0],
        format!(" 1│{}»", &riga[..26]),
        "riga piu' lunga della finestra: ultimo segno »"
    );
    a.handle_key(k(KeyCode::End));
    assert_eq!(a.cursor(), 60);
    let s = screen(&mut a, 30, 7);
    // colonna 60 nella finestra: hscroll = 60 - 27 + 1 = 34; si vedono le celle 34..60 (26 caratteri)
    assert_eq!(a.hscroll(), 34);
    assert_eq!(s[0], format!(" 1│«{}", &riga[35..60]));
    assert_eq!(s[5], " Ln 1  Col 61  byte 60  95%");
    a.handle_key(k(KeyCode::Home));
    assert_eq!(a.hscroll(), 0);
}

#[test]
fn the_mouse_wheel_scrolls_three_rows_and_the_cursor_stays_in_view() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 30, 7);
    a.handle_mouse(wheel(true));
    assert_eq!(a.top(), 24, "tre righe");
    assert_eq!(
        a.cursor(),
        24,
        "il cursore era fuori dalla finestra: va sulla prima riga visibile"
    );
    a.handle_mouse(wheel(true));
    assert_eq!(a.top(), 48);
    a.handle_mouse(wheel(false));
    assert_eq!(a.top(), 24);
    for _ in 0..10 {
        a.handle_mouse(wheel(false));
    }
    assert_eq!(
        (a.top(), a.cursor()),
        (0, 32),
        "in cima; il cursore e' sull'ultima riga visibile"
    );
    for _ in 0..30 {
        a.handle_mouse(wheel(true));
    }
    assert_eq!(
        a.top(),
        200,
        "non si scorre oltre l'ultima pagina: l'ultima riga in fondo"
    );
}

#[test]
fn resizing_keeps_the_cursor_visible() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 30, 7);
    for _ in 0..4 {
        a.handle_key(k(KeyCode::Down));
    }
    assert_eq!((a.top(), a.cursor()), (0, 32));
    a.resize(30, 5); // 3 righe di testo
    let s = screen(&mut a, 30, 5);
    assert_eq!(s[0], "  3│line 03");
    assert_eq!(s[2], "  5│line 05");
    assert_eq!(a.top(), 16);
}

// ---------- modalita' e uscita ----------

#[test]
fn alt_e_and_alt_v_switch_between_view_and_edit() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 80, 7);
    a.handle_key(modifica());
    assert!(a.is_edit());
    let s = screen(&mut a, 80, 7);
    assert!(s[5].ends_with("240 B  EDIT"), "{:?}", s[5]);
    assert!(
        s[5].starts_with(" edit mode: the file is not touched until you save"),
        "{:?}",
        s[5]
    );
    assert!(
        s[6].contains("Alt+V View") && s[6].contains("Alt+E Edit"),
        "{:?}",
        s[6]
    );
    a.handle_key(k(KeyCode::Down)); // il messaggio sparisce al tasto successivo
    let s = screen(&mut a, 80, 7);
    assert!(s[5].starts_with(" Ln 2  Col 1  byte 8  3%"), "{:?}", s[5]);
    a.handle_key(vista());
    assert!(!a.is_edit());
    let s = screen(&mut a, 80, 7);
    assert!(s[5].ends_with("240 B  VIEW"), "{:?}", s[5]);
    assert!(s[5].starts_with(" view mode"), "{:?}", s[5]);
    assert!(s[6].contains("Alt+E Edit"), "{:?}", s[6]);
}

#[test]
fn typing_in_view_mode_is_refused_with_a_hint() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 80, 7);
    for e in [
        ch('x'),
        k(KeyCode::Enter),
        k(KeyCode::Backspace),
        k(KeyCode::Delete),
        k(KeyCode::Tab),
    ] {
        a.handle_key(e);
        let st = screen(&mut a, 80, 7)[5].clone();
        assert!(st.starts_with(" read-only: press Alt+E to edit"), "{st:?}");
        a.handle_key(k(KeyCode::Right)); // cancella il messaggio
    }
    assert_eq!(a.document_len(), 240);
    assert!(!a.is_modified());
}

#[test]
fn f10_and_ctrl_q_quit_when_nothing_is_modified() {
    let dir = TestDir::new();
    let mut a = app(&dir, &thirty(), 30, 7);
    assert!(!a.should_quit());
    a.handle_key(k(KeyCode::F(10)));
    assert!(a.should_quit());
    let mut b = app(&dir, &thirty(), 30, 7);
    b.handle_key(ctrl(KeyCode::Char('q')));
    assert!(b.should_quit());
}

#[test]
fn a_cut_row_shows_no_line_number_on_the_continuation() {
    // una riga da 3 MiB: la camminata la taglia; la continuazione non ha numero di riga
    let dir = TestDir::new();
    let mut data = vec![b'a'; 3 * 1024 * 1024];
    data.extend_from_slice(b"\nzz\n");
    let mut a = app(&dir, &data, 40, 7);
    let s = screen(&mut a, 40, 7);
    assert!(s[0].starts_with(&format!("{:>7}│", 1)), "{:?}", s[0]); // len ~ 3 MiB: 7 cifre
    assert!(
        s[1].starts_with(&format!("{:>7}│", "")),
        "la seconda riga visuale continua la prima: {:?}",
        s[1]
    );
    assert!(s[0].ends_with('»'));
}

/// Alt+E: passa alla modifica (F4 e' Sostituisci).
fn modifica() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
}

/// Alt+V: torna in sola lettura.
fn vista() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT)
}
