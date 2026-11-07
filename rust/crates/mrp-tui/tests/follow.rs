//! Seguire la coda nell'editor (fase 2, 3.1): Alt+F accende "segui" su un documento in lettura; `App::follow_poll` (lo chiama
//! `tick` ogni mezzo secondo) guarda se il file e' cresciuto, lo riallinea e porta la vista in fondo; qualunque movimento che
//! lascia la fine del file ferma il seguire; la modifica e il seguire non stanno insieme. Scritti da ARCH: gli attesi sono
//! calcolati a mano. Il file ha 30 righe "linea NN\n" (NN da 00 a 29); il terminale e' 40 x 10 (7 righe di testo).

use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::{self, Cmd};
use mrp_tui::ui::render_lines;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn app() -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_follow_app_{}_{}.log", std::process::id(), n));
    let testo: String = (0..30).map(|i| format!("linea {i:02}\n")).collect();
    std::fs::write(&p, testo).unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let _ = render_lines(&mut a, 40, 9);
    (a, p)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn alt_f() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT)
}

fn appendi(p: &PathBuf, testo: &str) {
    let mut f = std::fs::OpenOptions::new().append(true).open(p).unwrap();
    f.write_all(testo.as_bytes()).unwrap();
}

fn schermo(a: &mut App) -> String {
    render_lines(a, 40, 9).join("\n")
}

#[test]
fn the_command_is_alt_f_and_it_starts_off() {
    let c = Cmd::Follow.info();
    assert_eq!(c.label, "gui.follow");
    assert_eq!(c.shortcut, "Alt+F");
    assert_eq!(c.key, Some(alt_f()));
    assert_eq!(commands::from_key(alt_f()), Some(Cmd::Follow));
    let (a, p) = app();
    assert!(!a.follow());
    let _ = std::fs::remove_file(p);
}

#[test]
fn alt_f_turns_it_on_and_shows_the_end_of_the_file() {
    let (mut a, p) = app();
    assert!(
        schermo(&mut a).contains("linea 00"),
        "all'apertura si vede l'inizio"
    );
    a.handle_key(alt_f());
    assert!(a.follow());
    let s = schermo(&mut a);
    assert!(s.contains("linea 29"), "{s}");
    assert!(!s.contains("linea 00"), "{s}");
    assert!(s.contains("TAIL"), "il titolo dice che si segue: {s}");
    // Alt+F di nuovo lo spegne
    a.handle_key(alt_f());
    assert!(!a.follow());
    assert!(!schermo(&mut a).contains("TAIL"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_file_that_grows_is_followed_to_the_new_end() {
    let (mut a, p) = app();
    a.handle_key(alt_f());
    appendi(&p, "nuova A\nnuova B\nnuova C\n");
    assert!(a.follow_poll(), "il documento e' cambiato");
    let s = schermo(&mut a);
    assert!(s.contains("nuova C"), "{s}");
    assert!(!s.contains("linea 20"), "la vista e' scesa: {s}");
    assert_eq!(a.status().size, 30 * 9 + 24);
    // niente di nuovo: nessun cambiamento
    assert!(!a.follow_poll());
    // e ancora: una riga alla volta
    appendi(&p, "nuova D\n");
    assert!(a.follow_poll());
    assert!(schermo(&mut a).contains("nuova D"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_line_numbers_follow_the_growth() {
    let (mut a, p) = app();
    a.handle_key(alt_f());
    appendi(&p, "x\ny\n");
    assert!(a.follow_poll());
    // 32 righe: il cursore sta sull'ultima (la riga vuota dopo l'ultimo a-capo non si conta)
    let s = a.status();
    assert!(s.line.is_some());
    assert!(
        schermo(&mut a).contains("32"),
        "il margine mostra il numero 32"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn without_following_a_growing_file_is_left_alone() {
    let (mut a, p) = app();
    appendi(&p, "nuova\n");
    assert!(!a.follow_poll());
    assert_eq!(a.status().size, 30 * 9);
    let _ = std::fs::remove_file(p);
}

#[test]
fn leaving_the_end_stops_following_but_moving_at_the_end_does_not() {
    let (mut a, p) = app();
    a.handle_key(alt_f());
    // alla fine un Giu', Fine e Pag.giu non la lasciano
    for c in [KeyCode::Down, KeyCode::End, KeyCode::PageDown] {
        a.handle_key(k(c));
        assert!(a.follow(), "{c:?}");
    }
    // Su la lascia: il seguire si ferma e il file che cresce non sposta piu' niente
    a.handle_key(k(KeyCode::Up));
    assert!(!a.follow());
    appendi(&p, "dopo\n");
    assert!(!a.follow_poll());
    assert_eq!(a.status().size, 30 * 9);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_wheel_and_the_keys_that_go_up_stop_following_too() {
    use ratatui::crossterm::event::{MouseEvent, MouseEventKind};
    for scheda in 0..3 {
        let (mut a, p) = app();
        a.handle_key(alt_f());
        match scheda {
            0 => a.handle_key(k(KeyCode::PageUp)),
            1 => a.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL)),
            _ => a.handle_mouse(MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 5,
                row: 3,
                modifiers: KeyModifiers::NONE,
            }),
        }
        assert!(!a.follow(), "scheda {scheda}");
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn turning_it_on_again_returns_to_the_end() {
    let (mut a, p) = app();
    a.handle_key(alt_f());
    a.handle_key(k(KeyCode::Up));
    assert!(!a.follow());
    appendi(&p, "tardi\n");
    a.handle_key(alt_f());
    assert!(a.follow());
    assert!(a.follow_poll(), "recupera quello che e' arrivato");
    assert!(schermo(&mut a).contains("tardi"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn editing_and_following_do_not_go_together() {
    let (mut a, p) = app();
    a.handle_key(k(KeyCode::F(4)));
    assert!(a.is_edit());
    a.handle_key(alt_f());
    assert!(!a.follow(), "in modifica non si segue");
    a.handle_key(k(KeyCode::F(4)));
    assert!(!a.is_edit());
    a.handle_key(alt_f());
    assert!(a.follow());
    // accendere la modifica ferma il seguire
    a.handle_key(k(KeyCode::F(4)));
    assert!(a.is_edit());
    assert!(!a.follow());
    let _ = std::fs::remove_file(p);
}

// su Windows un file mappato non si puo' accorciare (errore 1224): la prova gira dove si puo'
#[cfg(unix)]
#[test]
fn a_file_that_shrinks_stops_following_with_a_message() {
    let (mut a, p) = app();
    a.handle_key(alt_f());
    std::fs::write(&p, "linea 00\n").unwrap();
    assert!(a.follow_poll(), "il seguire si ferma: lo stato e' cambiato");
    assert!(!a.follow());
    let m = a.status().flash.unwrap_or_default();
    assert!(m.contains("shorter"), "{m:?}");
    // il documento non cambia da solo
    assert_eq!(a.status().size, 30 * 9);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_workspace_runs_the_command_and_its_tick_polls_twice_a_second() {
    let (a, p) = app();
    let mut ws = Workspace::new(a, AppConfig::default());
    ws.resize(40, 9);
    assert!(commands::enabled(&ws, Cmd::Follow));
    assert!(ws.exec(Cmd::Follow));
    assert!(ws.active().follow());
    appendi(
        &p, "w
",
    );
    // il controllo ha un passo di mezzo secondo: subito dopo l'accensione non c'e' ancora niente, dopo 700 ms si'
    std::thread::sleep(Duration::from_millis(700));
    ws.tick();
    assert_eq!(ws.active_mut().status().size, 30 * 9 + 2);
    let _ = std::fs::remove_file(p);
}
