//! Ctrl+K cancella la riga VERA (FL59, il difetto X-09): anche il record mostruoso oltre le righe
//! visuali da 128 KiB, in un colpo solo. E F3 riapre la maschera dopo una ricerca che non ha trovato
//! niente (U-16). Scritti da ARCH prima dell'implementazione: gli attesi sono il contenuto del file,
//! contato a mano, mai preso dall'output del programma.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}
fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}
fn modifica() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
}

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_killline_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    p
}

fn app_edit(dati: &[u8]) -> (App, PathBuf) {
    let p = file(dati);
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(60, 10);
    assert!(a.wait_idle(Duration::from_secs(60)));
    a.handle_key(modifica());
    assert!(a.is_edit());
    (a, p)
}

/// Il file del difetto: "a\n" + 17 MiB di x + "\nb\n" (il record oltre il tetto delle righe visuali).
fn file_mostruoso() -> Vec<u8> {
    let mut dati = b"a\n".to_vec();
    dati.extend(std::iter::repeat_n(b'x', 17 * 1024 * 1024));
    dati.extend_from_slice(b"\nb\n");
    dati
}

#[test]
fn ctrl_k_cancella_la_riga_mostruosa_in_un_colpo_solo() {
    let dati = file_mostruoso();
    let (mut a, p) = app_edit(&dati);
    // il cursore in mezzo al record (byte 9 MiB): due pagine giu' non bastano, vado con Ctrl+G a byte
    a.handle_key(ctrl('g'));
    for c in "@9437184".chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(60)));

    // Ctrl+K: la riga VERA intera, col suo a-capo
    a.handle_key(ctrl('k'));
    assert_eq!(
        a.document_text(),
        b"a\nb\n",
        "il record e il suo a-capo via in un colpo"
    );
    assert_eq!(a.cursor(), 2, "il cursore sta dove cominciava il record");
    assert!(a.is_modified());

    // Ctrl+Z: tutto torna
    a.handle_key(ctrl('z'));
    assert_eq!(
        a.document_text().len(),
        dati.len(),
        "l'annullamento ripristina il record"
    );

    // di nuovo Ctrl+K e salvo: il disco deve avere "a\nb\n"
    a.handle_key(ctrl('y')); // ripeti la cancellazione
    assert_eq!(a.document_text(), b"a\nb\n");
    a.handle_key(k(KeyCode::F(2)));
    assert!(a.wait_idle(Duration::from_secs(60)));
    assert_eq!(
        std::fs::read(&p).unwrap(),
        b"a\nb\n",
        "il disco ha il record cancellato"
    );
    assert!(!a.is_modified());
}

#[test]
fn ctrl_k_su_righe_normali() {
    let (mut a, _p) = app_edit(b"ab\ncd\nef\n");
    // cursore sulla seconda riga: Ctrl+K toglie "cd\n" intera
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Right));
    a.handle_key(ctrl('k'));
    assert_eq!(a.document_text(), b"ab\nef\n");
    assert_eq!(a.cursor(), 3);
    // l'ultima riga col suo a-capo (il cursore torna in testa alla riga che resta)
    a.handle_key(k(KeyCode::Home));
    a.handle_key(ctrl('k'));
    assert_eq!(a.document_text(), b"ab\n");
    // anche l'ultima riga di tutte: il cursore in fondo sta sulla riga vuota finale, Su lo riporta nella riga
    a.handle_key(k(KeyCode::Up));
    a.handle_key(ctrl('k'));
    assert_eq!(a.document_text(), b"");
}

#[test]
fn ctrl_k_in_lettura_non_tocca_il_documento() {
    let (mut a, p) = {
        let p = file(b"ab\ncd\n");
        let mut a = App::open(&p, AppConfig::default()).unwrap();
        a.resize(60, 10);
        assert!(a.wait_idle(Duration::from_secs(20)));
        (a, p)
    };
    assert!(!a.is_edit());
    a.handle_key(ctrl('k'));
    assert!(!a.is_modified());
    assert_eq!(a.document_text(), b"ab\ncd\n");
    assert_eq!(std::fs::read(&p).unwrap(), b"ab\ncd\n");
}

#[test]
fn f3_riapre_la_maschera_dopo_non_trovato() {
    let (mut a, _p) = app_edit(b"riga uno\nriga due\n");
    // la prima F3 apre la maschera
    a.handle_key(k(KeyCode::F(3)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Find));
    // cerco qualcosa che non c'e'
    for c in "zzzz".chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(
        a.dialog_kind(),
        None,
        "la maschera si chiude con la ricerca"
    );
    // la ricerca non ha trovato: F3 RIAPRE la maschera, col testo di prima gia' dentro
    a.handle_key(k(KeyCode::F(3)));
    assert_eq!(
        a.dialog_kind(),
        Some(DialogKind::Find),
        "dopo un non-trovato F3 riapre la maschera"
    );
    let s = render_lines(&mut a, 60, 10).join("\n");
    assert!(
        s.contains("> zzzz"),
        "il campo riprende il testo cercato: {s}"
    );
    // cambio testo e cerco qualcosa che c'e'
    for _ in 0..4 {
        a.handle_key(k(KeyCode::Backspace));
    }
    for c in "riga".chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.dialog_kind(), None);
    // trovato: F3 adesso RIPETE la ricerca (la maschera non si riapre)
    a.handle_key(k(KeyCode::F(3)));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(
        a.dialog_kind(),
        None,
        "dopo un trovato F3 ripete la ricerca"
    );
}
