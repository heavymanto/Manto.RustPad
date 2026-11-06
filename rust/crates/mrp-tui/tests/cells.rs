//! La griglia di celle dello schermo (E31): `render_grid` deve dire le stesse cose di `render_lines` e dare i colori e
//! il cursore. Scritti da ARCH; gli attesi sono i colori della tavolozza e il disegno di `render_lines`.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::cells::{colore, render_grid, BG_PREDEFINITO, FG_PREDEFINITO};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(data: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_cells_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, data).unwrap();
    p
}

fn app(data: &[u8]) -> (App, PathBuf) {
    let p = file(data);
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

#[test]
fn the_grid_says_what_render_lines_says() {
    let (mut a, p) = app("prima riga\nseconda \u{65e5}\u{672c}\nterza\n".as_bytes());
    for (w, h) in [(40u16, 10u16), (80, 24), (20, 6)] {
        a.resize(w, h);
        let g = render_grid(&mut a, w, h);
        assert_eq!((g.w, g.h), (w, h));
        assert_eq!(g.celle.len(), w as usize * h as usize);
        assert_eq!(g.righe(), render_lines(&mut a, w, h), "{w}x{h}");
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_cursor_is_in_the_grid() {
    let (mut a, p) = app(b"abc\ndef\n");
    let g = render_grid(&mut a, 40, 10);
    // il gutter e' largo gutter_w celle: il cursore e' sulla prima cella del testo della prima riga di testo (riga 0)
    let (x, y) = g.cursore.expect("il cursore c'e'");
    assert_eq!(y, 0);
    assert_eq!(g.cella(x, y).unwrap().simbolo, "a");
    a.handle_key(k(KeyCode::Right));
    let g = render_grid(&mut a, 40, 10);
    let (x, y) = g.cursore.unwrap();
    assert_eq!(g.cella(x, y).unwrap().simbolo, "b");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_wide_character_takes_two_cells() {
    let (mut a, p) = app("\u{65e5}x\n".as_bytes());
    let g = render_grid(&mut a, 40, 10);
    let (x, y) = g.cursore.unwrap();
    assert_eq!(g.cella(x, y).unwrap().simbolo, "\u{65e5}");
    assert_eq!(g.cella(x + 1, y).unwrap().simbolo, "");
    assert_eq!(g.cella(x + 2, y).unwrap().simbolo, "x");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_found_text_has_another_background() {
    let (mut a, p) = app(b"uno due tre\n");
    a.handle_key(k(KeyCode::F(3)));
    for c in "due".chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    let g = render_grid(&mut a, 40, 10);
    // la riga 0 e' la prima di testo: trova la cella della 'u' di "uno" (dopo il gutter) e la 'd' di "due" 4 celle dopo
    let inizio = (0..37u16)
        .find(|&x| {
            ["u", "n", "o"]
                .iter()
                .enumerate()
                .all(|(i, s)| g.cella(x + i as u16, 0).unwrap().simbolo == *s)
        })
        .expect("uno");
    let uno = g.cella(inizio, 0).unwrap();
    let due = g.cella(inizio + 4, 0).unwrap();
    assert_eq!(due.simbolo, "d");
    assert_ne!(due.bg, uno.bg, "la parte trovata ha uno sfondo diverso");
    let _ = std::fs::remove_file(p);
}

#[test]
fn ansi_and_indexed_colours() {
    assert_eq!(colore(Color::Red, FG_PREDEFINITO), [197, 15, 31]);
    assert_eq!(colore(Color::White, FG_PREDEFINITO), [242, 242, 242]);
    assert_eq!(colore(Color::DarkGray, FG_PREDEFINITO), [118, 118, 118]);
    assert_eq!(colore(Color::Reset, FG_PREDEFINITO), FG_PREDEFINITO);
    assert_eq!(colore(Color::Reset, BG_PREDEFINITO), BG_PREDEFINITO);
    assert_eq!(colore(Color::Rgb(1, 2, 3), FG_PREDEFINITO), [1, 2, 3]);
    // indicizzati: 0-15 come ANSI, 16 e 231 gli estremi del cubo, 232-255 i grigi
    assert_eq!(colore(Color::Indexed(1), FG_PREDEFINITO), [197, 15, 31]);
    assert_eq!(colore(Color::Indexed(16), FG_PREDEFINITO), [0, 0, 0]);
    assert_eq!(colore(Color::Indexed(231), FG_PREDEFINITO), [255, 255, 255]);
    assert_eq!(colore(Color::Indexed(196), FG_PREDEFINITO), [255, 0, 0]);
    assert_eq!(colore(Color::Indexed(232), FG_PREDEFINITO), [8, 8, 8]);
    assert_eq!(colore(Color::Indexed(255), FG_PREDEFINITO), [238, 238, 238]);
}

#[test]
fn an_empty_size_gives_an_empty_grid() {
    let (mut a, p) = app(b"x\n");
    assert!(render_grid(&mut a, 0, 10).celle.is_empty());
    assert!(render_grid(&mut a, 10, 0).celle.is_empty());
    let _ = std::fs::remove_file(p);
}
