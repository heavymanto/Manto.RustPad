//! La barra orizzontale (BUG 1 dell'utente, 11 nov 2026: le righe piu' larghe della finestra si vedevano solo nella
//! prima parte, e l'unico modo di scorrere era camminarci col cursore). La barra sta sotto il testo, sopra la riga di
//! stato: c'e' solo quando una riga e' piu' larga della finestra (o si e' gia' scorsi) e prende una riga al testo; il
//! pollice copre la parte di riga che si vede; il clic sulla pista porta la finestra li' (il pollice centrato sul
//! puntatore), il trascinamento lo segue per come lo si e' afferrato — e il cursore resta dov'e'. Scritti da ARCH: il
//! file ha una riga da 100 celle su un terminale da 40 (le larghezze contate qui), gli attesi sono l'aritmetica di
//! quella costruzione.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// "ab", una riga di 100 celle, "cd": la seconda e' piu' larga della finestra da 40.
fn dati() -> Vec<u8> {
    let mut v = b"ab\n".to_vec();
    v.extend(vec![b'x'; 100]);
    v.extend(b"\ncd\n");
    v
}

fn app(chrome: bool) -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_hbar_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, dati()).unwrap();
    let cfg = AppConfig {
        chrome,
        line_numbers: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
    }
}

/// Due disegni: il primo misura la riga piu' larga, il secondo disegna la barra (che gli toglie una riga al testo).
fn disegna(a: &mut App) -> Vec<String> {
    let _ = render_lines(a, 40, 9);
    render_lines(a, 40, 9)
}

#[test]
fn the_bar_appears_when_a_line_is_wider_and_takes_a_text_row() {
    let (mut a, p) = app(true);
    let righe = disegna(&mut a);
    // la riga da 100 celle sulla finestra da 40: il pollice copre 40/100 della pista, all'inizio
    assert_eq!(righe[6], format!("{}{}", "█".repeat(16), "░".repeat(24)));
    // il testo perde una riga (sei invece di sette); la riga lunga mostra le prime 39 celle e il "c'e' altro"
    assert_eq!(righe[1], format!("{}»", "x".repeat(39)));
    assert_eq!((righe[0].as_str(), righe[2].as_str()), ("ab", "cd"));
    assert_eq!(righe[5], "");
    // la barra sta solo li': riga di stato e tasti sono le ultime due, senza barra
    assert!(righe
        .iter()
        .enumerate()
        .all(|(i, r)| i == 6 || (!r.contains('█') && !r.contains('░'))));
    assert!(!righe[7].is_empty() && !righe[8].is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_bar_stays_hidden_when_every_line_fits() {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_hbar_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, "uno\ndue\ntre\nquattro\ncinque\nsei\nsettimo\n").unwrap();
    let cfg = AppConfig {
        line_numbers: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let righe = disegna(&mut a);
    // niente barra: il testo tiene tutte e sette le righe (la settima e' l'ultima visibile)
    assert!(!a.hbar_needed());
    assert!(righe.iter().all(|r| !r.contains('█') && !r.contains('░')));
    assert_eq!(righe[6], "settimo");
    let _ = std::fs::remove_file(p);
}

// SEGUE

#[test]
fn the_click_on_the_track_moves_the_window_and_the_cursor_stays() {
    let (mut a, p) = app(true);
    disegna(&mut a);
    // un clic sulla pista alla colonna 30 (oltre il pollice, che comincia a 0 e copre 16 celle): il pollice si
    // centra sul puntatore (presa 8) e la sua cima va alla frazione (30-8)/40 = 0,55 -> 55 celle
    a.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 30, 6));
    a.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 30, 6));
    assert_eq!(a.hscroll(), 55);
    // il cursore sta dov'era: la barra scorre la vista, non il cursore
    assert_eq!(a.cursor(), 0);
    let righe = disegna(&mut a);
    // la finestra [55, 95) della riga lunga: "c'e' altro" da tutte e due le parti
    assert_eq!(righe[1], format!("«{}»", "x".repeat(38)));
    assert_eq!(
        righe[6],
        format!("{}{}{}", "░".repeat(22), "█".repeat(16), "░".repeat(2))
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_drag_follows_the_pointer_from_any_row_and_up_releases() {
    let (mut a, p) = app(true);
    disegna(&mut a);
    // si afferra il pollice alla colonna 10 (dentro il pollice 0..16: presa 10)
    a.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 10, 6));
    // il trascinamento vale da qualunque riga: conta solo la colonna del puntatore
    a.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 30, 4));
    assert_eq!(a.hscroll(), 50, "(30-10)/40 = 0,5 -> 50 celle");
    a.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 30, 4));
    // rilasciato: la finestra resta dov'e'
    assert_eq!(a.hscroll(), 50);
    // un nuovo clic oltre il pollice (che ora comincia a 20): la cima va alla frazione (38-8)/40 = 0,75
    // -> 75, ma la finestra non passa oltre la fine della riga meno la sua larghezza: 60
    a.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 38, 6));
    assert_eq!(a.hscroll(), 60);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_wheel_of_the_window_scrolls_by_eight_cells_and_stops_at_the_ends() {
    let (mut a, p) = app(true);
    disegna(&mut a);
    a.hbar_wheel(true, 1);
    assert_eq!(a.hscroll(), 8);
    // il limite e' la riga meno la finestra: 100 - 40 = 60
    a.hbar_wheel(true, 10);
    assert_eq!(a.hscroll(), 60);
    a.hbar_wheel(false, 100);
    assert_eq!(a.hscroll(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn without_the_chrome_the_app_leaves_the_bar_to_the_window() {
    let (mut a, p) = app(false);
    let righe = disegna(&mut a);
    // serve (la riga e' piu' larga della finestra), ma la barra a celle e' del terminale: qui non c'e'
    assert!(a.hbar_needed());
    assert!(righe.iter().all(|r| !r.contains('█') && !r.contains('░')));
    let _ = std::fs::remove_file(p);
}
