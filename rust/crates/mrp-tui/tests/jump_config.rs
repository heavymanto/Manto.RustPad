//! La configurazione dei salti (l'ingranaggio della barra dei salti): un dialogo con le pagine dei tre salti (Alt+Pag, con
//! Maiusc, con Ctrl); si confermano se stanno fra 2 e 10000, si applicano a tutte le schede e alle nuove e si salvano nelle
//! impostazioni (`jump_pages=a,b,c`), e la barra dei salti ne prende le etichette. Scritti da ARCH.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::dialogs::DialogView;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_jumpcfg_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ws(settings: Option<PathBuf>) -> (PathBuf, Workspace) {
    let d = cartella();
    let p = d.join("a.txt");
    std::fs::write(&p, "uno\ndue\n").unwrap();
    let cfg = AppConfig {
        settings_path: settings,
        ..AppConfig::default()
    };
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut w = Workspace::new(app, cfg);
    w.resize(80, 14);
    assert!(w.wait_idle(Duration::from_secs(20)));
    (d, w)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn vista(w: &mut Workspace) -> ([String; 3], [String; 3], u8) {
    match w.active().dialog_view().unwrap() {
        DialogView::Jumps {
            labels,
            values,
            field,
            ..
        } => (labels, values, field),
        altro => panic!("{altro:?}"),
    }
}

#[test]
fn the_dialog_shows_the_three_jumps_with_their_labels() {
    let (d, mut w) = ws(None);
    w.active_mut().open_jump_dialog();
    assert!(w.active().dialog_has_view());
    let DialogView::Jumps { title, hint, .. } = w.active().dialog_view().unwrap() else {
        panic!()
    };
    assert_eq!(title, "JUMPS");
    assert!(hint.contains("2 to 10000"), "{hint}");
    let (etichette, valori, campo) = vista(&mut w);
    assert_eq!(valori, ["10", "100", "1000"]);
    assert_eq!(campo, 0);
    assert!(
        etichette[0].contains("Alt+PgUp") && etichette[2].contains("Ctrl+Alt"),
        "{etichette:?}"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn typing_tab_and_enter_apply_the_numbers_everywhere_and_save_them() {
    let prof = std::env::temp_dir().join(format!("mrp_jumpcfg_prof_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&prof);
    let settings = prof.join("settings.txt");
    let (d, mut w) = ws(Some(settings.clone()));
    let altro = d.join("b.txt");
    std::fs::write(&altro, "x\n").unwrap();
    w.open(&altro).unwrap();
    w.active_mut().open_jump_dialog();
    // il primo campo: si toglie "10" e si scrive 5; Tab; poi 50; Tab; poi 500
    for _ in 0..5 {
        w.handle_key(k(KeyCode::Backspace));
    }
    w.handle_key(k(KeyCode::Char('5')));
    w.handle_key(k(KeyCode::Tab));
    for _ in 0..5 {
        w.handle_key(k(KeyCode::Backspace));
    }
    for c in "50".chars() {
        w.handle_key(k(KeyCode::Char(c)));
    }
    w.handle_key(k(KeyCode::Tab));
    for _ in 0..5 {
        w.handle_key(k(KeyCode::Backspace));
    }
    for c in "500".chars() {
        w.handle_key(k(KeyCode::Char(c)));
    }
    assert_eq!(vista(&mut w).1, ["5", "50", "500"]);
    w.handle_key(k(KeyCode::Enter));
    assert_eq!(w.active().dialog_kind(), None);
    // tutte le schede, e la barra dei salti ne prende le etichette
    assert!(w.tabs().iter().all(|a| a.jump_pages() == [5, 50, 500]));
    assert_eq!(w.jump_pages(), [5, 50, 500]);
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("jump_pages=5,50,500"), "{scritto}");
    // una scheda nuova li eredita
    let terzo = d.join("c.txt");
    std::fs::write(&terzo, "y\n").unwrap();
    w.open(&terzo).unwrap();
    assert_eq!(w.jump_pages(), [5, 50, 500]);
    // e Alt+PgDn salta di 5 pagine (1 pagina = 14 - 3 righe di testo - 1 ... basta che il tasto lo faccia)
    let _ = std::fs::remove_dir_all(prof);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn invalid_numbers_keep_the_dialog_open_and_say_why() {
    let (d, mut w) = ws(None);
    for (testo, ok) in [
        ("1", false),
        ("10001", false),
        ("", false),
        ("2", true),
        ("10000", true),
    ] {
        w.active_mut().open_jump_dialog();
        w.active_mut().dialog_set_jump(1, testo);
        w.handle_key(k(KeyCode::Enter));
        if ok {
            assert_eq!(w.active().dialog_kind(), None, "{testo:?}");
            assert_eq!(w.jump_pages()[1], testo.parse::<usize>().unwrap());
        } else {
            assert!(w.active().dialog_kind().is_some(), "{testo:?}");
            let s = w.render_lines(80, 14).join("\n");
            assert!(s.contains("from 2 to 10000"), "{testo:?}: {s}");
            // il fuoco va al campo sbagliato
            assert_eq!(vista(&mut w).2, 1);
            w.handle_key(k(KeyCode::Esc));
        }
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn only_digits_up_to_five_and_escape_cancels_without_changing_anything() {
    let (d, mut w) = ws(None);
    w.active_mut().open_jump_dialog();
    w.active_mut().dialog_set_jump(0, "12ab3456789");
    assert_eq!(vista(&mut w).1[0], "12345");
    for c in ['x', '9', '-'] {
        w.handle_key(k(KeyCode::Char(c)));
    }
    assert_eq!(vista(&mut w).1[0], "12345", "pieno: niente di piu'");
    w.handle_key(k(KeyCode::Esc));
    assert_eq!(w.jump_pages(), [10, 100, 1000]);
    // la prossima volta riparte dai valori veri
    w.active_mut().open_jump_dialog();
    assert_eq!(vista(&mut w).1, ["10", "100", "1000"]);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn tab_and_the_arrows_cycle_through_the_three_fields() {
    let (d, mut w) = ws(None);
    w.active_mut().open_jump_dialog();
    let mut visti = Vec::new();
    for _ in 0..4 {
        visti.push(vista(&mut w).2);
        w.handle_key(k(KeyCode::Tab));
    }
    assert_eq!(visti, vec![0, 1, 2, 0]);
    // dopo i quattro Tab il fuoco e' sul secondo campo: Su torna al primo e Maiusc+Tab va all'ultimo
    w.handle_key(k(KeyCode::Up));
    assert_eq!(vista(&mut w).2, 0);
    w.handle_key(k(KeyCode::BackTab));
    assert_eq!(vista(&mut w).2, 2);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_dialog_is_drawn_in_the_terminal_with_its_three_rows() {
    let (d, mut w) = ws(None);
    w.active_mut().open_jump_dialog();
    let s = w.render_lines(80, 14).join("\n");
    assert!(s.contains("JUMPS"), "{s}");
    assert!(
        s.contains("> 10") && s.contains("> 100") && s.contains("> 1000"),
        "{s}"
    );
    let _ = std::fs::remove_dir_all(d);
}
