//! Scartare le modifiche non salvate e tornare al file com'e' sul disco (Alt+R): la conferma, la scheda che si rifa' dal file,
//! il caso della sessione che riporta modifiche vecchie. Scritti da ARCH: gli attesi sono calcolati a mano sul testo di prova.
//! Il file di prova e' `uno\ndue\ntre\n`: le righe cominciano a 0, 4 e 8.

use mrp_core::encoding::Encoding;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::commands::{self, Cmd};
use mrp_tui::dialogs::DialogView;
use mrp_tui::ui::render_lines;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartelle() -> (PathBuf, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let base = std::env::temp_dir().join(format!("mrp_revert_{}_{}", std::process::id(), n));
    let dir = base.join("files");
    let sess = base.join("session");
    std::fs::create_dir_all(&dir).unwrap();
    (dir, sess)
}

fn pulisci(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn alt_r() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT)
}

/// Un `Workspace` con due schede (a.txt e b.txt, la prima avanti), la sessione in `sess`, 60 x 12.
fn due_schede(dir: &Path, sess: &Path) -> Workspace {
    std::fs::write(dir.join("a.txt"), b"uno\ndue\ntre\n").unwrap();
    std::fs::write(dir.join("b.txt"), b"altro\nfile\n").unwrap();
    let cfg = AppConfig::default();
    let primo = App::open(&dir.join("a.txt"), cfg.clone()).unwrap();
    let mut w = Workspace::new(primo, cfg)
        .with_session(sess.to_path_buf())
        .with_session_timing(Duration::ZERO, Duration::ZERO);
    w.open(&dir.join("b.txt")).unwrap();
    w.select(0);
    w.resize(60, 12);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w
}

/// Passa in modifica, va alla seconda riga e scrive `XY` (il cursore finisce a 4 + 2 = 6).
fn modifica(w: &mut Workspace) {
    w.handle_key(k(KeyCode::F(4)));
    w.handle_key(k(KeyCode::Down));
    w.handle_key(k(KeyCode::Char('X')));
    w.handle_key(k(KeyCode::Char('Y')));
}

fn schermo(w: &mut Workspace) -> Vec<String> {
    render_lines(w.active_mut(), 60, 12)
}

#[test]
fn with_nothing_to_discard_it_says_so_and_asks_nothing() {
    let (dir, sess) = cartelle();
    let mut w = due_schede(&dir, &sess);
    w.handle_key(alt_r());
    assert_eq!(w.active().dialog_kind(), None);
    assert_eq!(w.active().flash(), Some("no unsaved changes to discard"));
    // il registro lo sa: spento senza modifiche, acceso con
    assert!(!commands::enabled(&w, Cmd::Revert));
    modifica(&mut w);
    assert!(commands::enabled(&w, Cmd::Revert));
    pulisci(&dir);
}

#[test]
fn it_asks_first_and_escape_keeps_the_changes() {
    let (dir, sess) = cartelle();
    let mut w = due_schede(&dir, &sess);
    modifica(&mut w);
    w.handle_key(alt_r());
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::Revert));
    // la domanda dice quale file, e i tasti
    let righe = schermo(&mut w).join("\n");
    assert!(righe.contains("DISCARD CHANGES"), "{righe}");
    assert!(righe.contains("every unsaved change to a.txt"), "{righe}");
    assert!(righe.contains("Enter Discard   Esc Cancel"), "{righe}");
    // per la finestra e' la stessa domanda a due pulsanti
    let Some(DialogView::Overwrite { title, text }) = w.active().dialog_view() else {
        panic!("non e' la domanda a due pulsanti");
    };
    assert_eq!(title, "DISCARD CHANGES");
    assert!(text.contains("a.txt"), "{text}");
    // Esc e N rinunciano: le modifiche restano
    w.handle_key(k(KeyCode::Esc));
    assert_eq!(w.active().dialog_kind(), None);
    assert!(w.active().is_modified());
    w.handle_key(alt_r());
    w.handle_key(k(KeyCode::Char('n')));
    assert_eq!(w.active().dialog_kind(), None);
    assert!(w.active().is_modified());
    assert!(schermo(&mut w).iter().any(|r| r.contains("XYdue")));
    pulisci(&dir);
}

#[test]
fn confirming_brings_back_the_file_as_it_is_on_disk() {
    let (dir, sess) = cartelle();
    let mut w = due_schede(&dir, &sess);
    modifica(&mut w);
    assert_eq!(w.active().cursor(), 6);
    w.handle_key(alt_r());
    w.handle_key(k(KeyCode::Enter));
    assert_eq!(w.active().dialog_kind(), None);
    // la stessa scheda, nello stesso posto, pulita e in lettura; il testo e' quello del file; il cursore all'inizio della riga di prima
    assert_eq!(w.len(), 2);
    assert_eq!(w.active_index(), 0);
    assert!(!w.active().is_modified());
    assert!(!w.active().is_edit());
    assert_eq!(w.active().cursor(), 4);
    assert_eq!(w.active().flash(), Some("changes discarded"));
    let righe = schermo(&mut w);
    assert!(righe.iter().any(|r| r.contains("due")), "{righe:#?}");
    assert!(!righe.iter().any(|r| r.contains("XY")), "{righe:#?}");
    // il file sul disco non e' stato toccato, ne' scritto
    assert_eq!(
        std::fs::read(dir.join("a.txt")).unwrap(),
        b"uno\ndue\ntre\n"
    );
    // e adesso si puo' di nuovo modificare da capo, e salvare: niente e' rimasto della storia di prima
    w.handle_key(k(KeyCode::F(4)));
    w.handle_key(k(KeyCode::Char('Z')));
    assert!(w.active().is_modified());
    pulisci(&dir);
}

#[test]
fn undo_has_nothing_left_after_discarding() {
    let (dir, sess) = cartelle();
    let mut w = due_schede(&dir, &sess);
    modifica(&mut w);
    w.handle_key(alt_r());
    w.handle_key(k(KeyCode::Enter));
    w.handle_key(k(KeyCode::F(4)));
    w.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
    assert_eq!(w.active().flash(), Some("nothing to undo"));
    pulisci(&dir);
}

#[test]
fn it_keeps_the_other_tabs_and_the_encoding() {
    let (dir, sess) = cartelle();
    let mut w = due_schede(&dir, &sess);
    // la seconda scheda modificata, la prima no
    w.select(1);
    w.handle_key(k(KeyCode::F(4)));
    w.handle_key(k(KeyCode::Char('Q')));
    assert!(w.active().is_modified());
    w.handle_key(alt_r());
    w.handle_key(k(KeyCode::Char('y')));
    assert_eq!(w.active_index(), 1);
    assert!(!w.active().is_modified());
    assert_eq!(w.active().file_name(), "b.txt");
    w.select(0);
    assert!(!w.active().is_modified());
    pulisci(&dir);
}

#[test]
fn an_untitled_document_has_no_file_to_go_back_to() {
    let (dir, sess) = cartelle();
    let mut w = due_schede(&dir, &sess);
    w.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    w.handle_key(k(KeyCode::Char('x')));
    assert!(w.active().is_modified());
    assert!(w.active().is_untitled());
    w.handle_key(alt_r());
    assert_eq!(w.active().dialog_kind(), None);
    assert_eq!(w.active().flash(), Some("no unsaved changes to discard"));
    assert!(w.active().is_modified(), "il testo scritto resta");
    assert!(!commands::enabled(&w, Cmd::Revert));
    pulisci(&dir);
}

#[test]
fn changes_brought_back_by_the_session_can_be_discarded() {
    // il caso vero: si chiude con modifiche non salvate, alla riapertura tornano; Alt+R le scarta e la sessione non le ricorda piu'
    let (dir, sess) = cartelle();
    let mut w = due_schede(&dir, &sess);
    modifica(&mut w);
    w.handle_key(k(KeyCode::F(10)));
    assert!(w.should_quit());
    drop(w);
    let riaperta = |sess: &Path| {
        let mut w = Workspace::from_session(sess.to_path_buf(), AppConfig::default())
            .expect("c'e' una sessione")
            .with_session_timing(Duration::ZERO, Duration::ZERO);
        w.resize(60, 12);
        assert!(w.wait_idle(Duration::from_secs(20)));
        w
    };
    let mut w = riaperta(&sess);
    assert!(w.active().is_modified(), "le modifiche sono tornate");
    w.handle_key(alt_r());
    w.handle_key(k(KeyCode::Enter));
    assert!(!w.active().is_modified());
    // chiusa di nuovo e riaperta: pulita
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    let w = riaperta(&sess);
    assert!(
        !w.active().is_modified(),
        "la sessione non ricorda piu' le modifiche"
    );
    assert_eq!(
        std::fs::read(dir.join("a.txt")).unwrap(),
        b"uno\ndue\ntre\n"
    );
    pulisci(&dir);
}

#[test]
fn the_file_comes_back_in_the_same_encoding() {
    // un file in Shift-JIS aperto da solo (la configurazione del Workspace non lo dice, e le codifiche a piu' byte non si
    // indovinano): dopo averlo scartato si legge ancora in Shift-JIS, non come UTF-8 (dove sarebbero caratteri di sostituzione)
    let (dir, sess) = cartelle();
    let p = dir.join("giapponese.txt");
    // 日本語 in Shift-JIS: 93 FA, 96 7B, 8C EA
    std::fs::write(&p, b"\x93\xfa\x96\x7b\x8c\xea\nlatte\n").unwrap();
    let sj = Encoding::by_name("shift-jis").unwrap();
    let primo = App::open(
        &p,
        AppConfig {
            encoding: Some(sj),
            ..AppConfig::default()
        },
    )
    .unwrap();
    let mut w = Workspace::new(primo, AppConfig::default())
        .with_session(sess.clone())
        .with_session_timing(Duration::ZERO, Duration::ZERO);
    w.resize(60, 12);
    assert!(w.wait_idle(Duration::from_secs(20)));
    assert!(schermo(&mut w)
        .iter()
        .any(|r| r.replace(' ', "").contains("日本語")));
    w.handle_key(k(KeyCode::F(4)));
    w.handle_key(k(KeyCode::Char('Q')));
    w.handle_key(alt_r());
    w.handle_key(k(KeyCode::Enter));
    assert!(!w.active().is_modified());
    assert_eq!(w.active().encoding(), sj);
    assert!(schermo(&mut w)
        .iter()
        .any(|r| r.replace(' ', "").contains("日本語")));
    pulisci(&dir);
}

#[test]
fn the_new_tab_takes_the_size_of_the_screen() {
    // 40 righe in uno schermo di 12 (9 di testo): dopo lo scarto, Ctrl+Fine deve far vedere l'ultima riga; con la scheda
    // rimasta della grandezza di fabbrica (24 righe) la finestra del testo resterebbe piu' in alto
    let (dir, sess) = cartelle();
    let mut testo = String::new();
    for i in 1..=40 {
        testo.push_str(&format!("linea {i}\n"));
    }
    let p = dir.join("lunga.txt");
    std::fs::write(&p, &testo).unwrap();
    let cfg = AppConfig::default();
    let primo = App::open(&p, cfg.clone()).unwrap();
    let mut w = Workspace::new(primo, cfg)
        .with_session(sess.clone())
        .with_session_timing(Duration::ZERO, Duration::ZERO);
    w.resize(60, 12);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w.handle_key(k(KeyCode::F(4)));
    w.handle_key(k(KeyCode::Char('Q')));
    w.handle_key(alt_r());
    w.handle_key(k(KeyCode::Enter));
    assert!(!w.active().is_modified());
    w.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL));
    let righe = schermo(&mut w);
    assert!(righe.iter().any(|r| r.contains("linea 40")), "{righe:#?}");
    pulisci(&dir);
}

#[test]
fn a_cursor_past_the_end_of_the_original_goes_to_its_last_line() {
    // si aggiunge testo in fondo (il cursore va oltre la lunghezza del file com'e' sul disco, 12 byte): dopo averlo scartato
    // il cursore sta nel file, all'inizio dell'ultima riga ("tre", a 8: in lettura non si va oltre l'ultimo a-capo, a 11)
    let (dir, sess) = cartelle();
    let mut w = due_schede(&dir, &sess);
    w.handle_key(k(KeyCode::F(4)));
    w.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL));
    for c in "abcdefgh".chars() {
        w.handle_key(k(KeyCode::Char(c)));
    }
    assert_eq!(w.active().cursor(), 20);
    w.handle_key(alt_r());
    w.handle_key(k(KeyCode::Enter));
    assert!(!w.active().is_modified());
    assert_eq!(w.active().cursor(), 8);
    pulisci(&dir);
}
