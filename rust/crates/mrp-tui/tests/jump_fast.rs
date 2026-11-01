//! I salti di pagina enormi con l'indice righe (fase 2, 1.1): con l'indice pronto un salto di molte migliaia di pagine non
//! cammina le righe una per una ma va a `offset_of_line`; il risultato e' quello di sempre (il cursore e l'inizio della vista
//! si muovono dello stesso numero di righe, ai bordi ci si ferma). Scritti da ARCH: il file ha 400.000 righe "riga NNNNNN\n"
//! di 12 byte, il terminale e' 40 x 60 (57 righe di testo, una pagina e' 56 righe); l'ultima pagina comincia alla riga
//! 399.943 (399.999 - 56).

use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const RIGA: u64 = 12;
const RIGHE: usize = 400_000;

fn app(indice: bool) -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_jumpfast_{}_{}.txt", std::process::id(), n));
    let dati: Vec<u8> = (0..RIGHE)
        .flat_map(|i| format!("riga {i:06}\n").into_bytes())
        .collect();
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        index: indice,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(40, 60);
    assert!(a.wait_idle(Duration::from_secs(60)));
    let _ = render_lines(&mut a, 40, 60);
    (a, p)
}

fn prima_riga_di_testo(a: &mut App) -> String {
    render_lines(a, 40, 60)[1].clone()
}

#[test]
fn a_huge_forward_jump_lands_where_the_walk_would_and_is_fast() {
    let (mut a, p) = app(true);
    let t = Instant::now();
    a.page_jump(true, 5000);
    let durata = t.elapsed();
    // 5000 pagine da 56 righe = 280.000 righe: la vista comincia alla riga 280.000 e il cursore e' li'
    assert_eq!(a.cursor(), 280_000 * RIGA);
    assert!(
        prima_riga_di_testo(&mut a).contains("riga 280000"),
        "{}",
        prima_riga_di_testo(&mut a)
    );
    assert!(durata < Duration::from_millis(1500), "{durata:?}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn it_stops_at_the_last_page_and_the_cursor_moves_by_the_same_rows() {
    let (mut a, p) = app(true);
    a.page_jump(true, 10_000);
    // la vista si ferma all'ultima pagina (riga 399.943) e il cursore ha fatto lo stesso cammino
    assert_eq!(a.cursor(), 399_943 * RIGA);
    assert!(prima_riga_di_testo(&mut a).contains("riga 399943"));
    // ancora avanti: non si muove
    a.page_jump(true, 5000);
    assert_eq!(a.cursor(), 399_943 * RIGA);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_huge_backward_jump_and_the_start_of_the_file() {
    let (mut a, p) = app(true);
    a.page_jump(true, 10_000);
    // 5000 pagine indietro = 280.000 righe: dalla 399.943 alla 119.943
    let t = Instant::now();
    a.page_jump(false, 5000);
    assert!(
        t.elapsed() < Duration::from_millis(1500),
        "{:?}",
        t.elapsed()
    );
    assert_eq!(a.cursor(), 119_943 * RIGA);
    assert!(prima_riga_di_testo(&mut a).contains("riga 119943"));
    a.page_jump(false, 10_000);
    assert_eq!(a.cursor(), 0);
    assert!(prima_riga_di_testo(&mut a).contains("riga 000000"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn without_the_index_the_walk_gives_the_same_place() {
    // senza indice (nessun indice costruito) si cammina: stesso risultato, solo piu' lento
    let (mut a, p) = app(false);
    a.page_jump(true, 600);
    assert_eq!(a.cursor(), 600 * 56 * RIGA);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_cursor_ahead_of_the_view_keeps_its_distance() {
    // il cursore sta 10 righe sotto l'inizio della vista: dopo il salto ci sta ancora
    let (mut a, p) = app(true);
    for _ in 0..10 {
        a.handle_key(ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Down,
            ratatui::crossterm::event::KeyModifiers::NONE,
        ));
    }
    assert_eq!(a.cursor(), 10 * RIGA);
    a.page_jump(true, 5000);
    assert_eq!(a.cursor(), (280_000 + 10) * RIGA);
    let _ = std::fs::remove_file(p);
}
