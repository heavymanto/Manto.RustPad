//! La vista filtrata nell'editor (fase 2, 3.3): Alt+G apre il dialogo del filtro (testo, glob o regex, maiuscole, Alt+I per
//! nascondere le righe che corrispondono); il lavoro gira in background (Esc lo annulla) e finisce in una scheda nuova di sola
//! lettura, "[filter] testo", con le righe che corrispondono; Invio su una riga porta alla stessa riga del file originale. Il
//! file di appoggio sta fra i temporanei e se ne va con la scheda. Scritti da ARCH: il file ha 5 righe e gli offset sono
//! calcolati a mano: "2026 INFO uno\n" (14 byte, a 0), "2026 ERROR due\n" (15, a 14), "2026 INFO tre\n" (14, a 29),
//! "2026 ERROR quattro\n" (19, a 43), "2026 WARN cinque\n" (a 62).

use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::{self, Cmd};
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const TESTO: &str =
    "2026 INFO uno\n2026 ERROR due\n2026 INFO tre\n2026 ERROR quattro\n2026 WARN cinque\n";

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_filtro_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ws_con(testo: &str) -> (PathBuf, Workspace) {
    let d = cartella();
    let p = d.join("a.log");
    std::fs::write(&p, testo).unwrap();
    let cfg = AppConfig::default();
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut w = Workspace::new(app, cfg);
    w.resize(80, 14);
    assert!(w.wait_idle(Duration::from_secs(20)));
    (d, w)
}

fn ws() -> (PathBuf, Workspace) {
    ws_con(TESTO)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn alt(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

/// Il campo del dialogo parte col testo dell'ultima ricerca: si svuota prima di scrivere.
fn svuota(w: &mut Workspace) {
    for _ in 0..40 {
        w.handle_key(k(KeyCode::Backspace));
    }
}

fn digita(w: &mut Workspace, testo: &str) {
    for c in testo.chars() {
        w.handle_key(k(KeyCode::Char(c)));
    }
}

/// Apre il dialogo del filtro, scrive `testo` e conferma (con `invert` preme Alt+I prima).
fn filtra(w: &mut Workspace, testo: &str, invert: bool) {
    w.handle_key(alt('g'));
    svuota(w);
    digita(w, testo);
    if invert {
        w.handle_key(alt('i'));
    }
    w.handle_key(k(KeyCode::Enter));
    assert!(w.wait_idle(Duration::from_secs(30)));
    // la scheda nuova (se c'e') costruisce il suo indice righe: serve a Invio
    assert!(w.wait_idle(Duration::from_secs(30)));
}

fn schermo(w: &mut Workspace) -> String {
    w.render_lines(80, 14).join("\n")
}

fn pulisci(d: PathBuf) {
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_command_is_alt_g_and_opens_a_dialog_with_the_filter_title() {
    let c = Cmd::Filter.info();
    assert_eq!((c.label, c.shortcut), ("gui.filter", "Alt+G"));
    assert_eq!(c.key, Some(alt('g')));
    assert_eq!(commands::from_key(alt('g')), Some(Cmd::Filter));
    let (d, mut w) = ws();
    assert!(commands::enabled(&w, Cmd::Filter));
    w.handle_key(alt('g'));
    assert!(w.active().filter_mode());
    let s = schermo(&mut w);
    assert!(s.contains("FILTER LINES"), "{s}");
    assert!(s.contains("Alt+I"), "{s}");
    // Ctrl+F e' sempre la ricerca
    w.handle_key(k(KeyCode::Esc));
    w.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    assert!(!w.active().filter_mode());
    pulisci(d);
}

#[test]
fn the_matching_lines_open_in_a_new_read_only_tab() {
    let (d, mut w) = ws();
    filtra(&mut w, "ERROR", false);
    assert_eq!(w.len(), 2);
    assert_eq!(w.active_index(), 1);
    assert!(w.active().is_filter_view());
    assert_eq!(w.active().file_name(), "[filter] ERROR");
    assert_eq!(
        w.active().document_text(),
        b"2026 ERROR due\n2026 ERROR quattro\n"
    );
    // l'originale non e' cambiato
    assert_eq!(w.tabs()[0].document_text(), TESTO.as_bytes());
    assert!(!w.tabs()[0].is_filter_view());
    // il file di appoggio sta fra i temporanei
    let scratch = w.active().filter_scratch().unwrap().to_path_buf();
    assert!(scratch.starts_with(std::env::temp_dir()), "{scratch:?}");
    assert!(scratch.exists());
    // il messaggio dice quante righe e come andare al file
    assert!(schermo(&mut w).contains("2 lines"), "{}", schermo(&mut w));
    pulisci(d);
}

#[test]
fn enter_on_a_line_goes_to_the_same_line_of_the_original() {
    let (d, mut w) = ws();
    filtra(&mut w, "ERROR", false);
    // la prima riga della vista e' la riga a 14 dell'originale
    w.handle_key(k(KeyCode::Enter));
    w.tick();
    assert_eq!(w.active_index(), 0);
    assert_eq!(w.active().cursor(), 14);
    // di nuovo: la seconda riga
    w.select(1);
    w.handle_key(k(KeyCode::Down));
    w.handle_key(k(KeyCode::Enter));
    w.tick();
    assert_eq!(w.active_index(), 0);
    assert_eq!(w.active().cursor(), 43);
    pulisci(d);
}

#[test]
fn enter_works_when_the_original_tab_was_closed() {
    let (d, mut w) = ws();
    filtra(&mut w, "WARN", false);
    w.close(0);
    assert_eq!(w.len(), 1);
    assert!(w.active().is_filter_view());
    w.handle_key(k(KeyCode::Enter));
    w.tick();
    // il file originale si e' riaperto e il cursore e' sulla riga della WARN (a 62)
    assert_eq!(w.len(), 2);
    assert!(!w.active().is_filter_view());
    assert_eq!(w.active().cursor(), 62);
    pulisci(d);
}

#[test]
fn alt_i_shows_the_lines_that_do_not_match() {
    let (d, mut w) = ws();
    filtra(&mut w, "ERROR", true);
    assert_eq!(
        w.active().document_text(),
        b"2026 INFO uno\n2026 INFO tre\n2026 WARN cinque\n"
    );
    // e Invio sulla seconda va alla riga a 29
    w.handle_key(k(KeyCode::Down));
    w.handle_key(k(KeyCode::Enter));
    w.tick();
    assert_eq!(w.active().cursor(), 29);
    pulisci(d);
}

#[test]
fn modes_case_and_unsaved_edits() {
    let (d, mut w) = ws();
    // senza maiuscole: "error" trova ERROR (Alt+C le toglie: di fabbrica contano)
    w.handle_key(alt('g'));
    svuota(&mut w);
    digita(&mut w, "error");
    w.handle_key(alt('c'));
    w.handle_key(k(KeyCode::Enter));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert_eq!(
        w.active().document_text(),
        b"2026 ERROR due\n2026 ERROR quattro\n"
    );
    w.close(1);
    // regex (Alt+M due volte: testo -> glob -> regex): le righe con una cifra seguita da " WARN"
    w.handle_key(alt('g'));
    svuota(&mut w);
    digita(&mut w, r"\d WARN");
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    w.handle_key(k(KeyCode::Enter));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert_eq!(w.active().document_text(), b"2026 WARN cinque\n");
    w.close(1);
    // le modifiche non salvate entrano nel filtro
    w.select(0);
    w.handle_key(modifica());
    w.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
    digita(&mut w, "ERROR nuova");
    w.handle_key(k(KeyCode::Enter));
    filtra(&mut w, "nuova", false);
    assert_eq!(w.active().document_text(), b"ERROR nuova\n");
    pulisci(d);
}

#[test]
fn nothing_matching_and_a_bad_pattern_open_no_tab_and_say_so() {
    let (d, mut w) = ws();
    filtra(&mut w, "zzz", false);
    assert_eq!(w.len(), 1);
    assert!(
        schermo(&mut w).contains("no line matches"),
        "{}",
        schermo(&mut w)
    );
    // un'espressione regolare sbagliata
    w.handle_key(alt('g'));
    svuota(&mut w);
    digita(&mut w, "(");
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    w.handle_key(k(KeyCode::Enter));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert_eq!(w.len(), 1);
    pulisci(d);
}

#[test]
fn closing_the_view_removes_its_scratch_file_and_the_view_is_not_in_the_session() {
    let (d, mut w) = ws();
    filtra(&mut w, "ERROR", false);
    assert!(w.active().session_record().is_none());
    let scratch = w.active().filter_scratch().unwrap().to_path_buf();
    assert!(scratch.exists());
    w.close(1);
    assert_eq!(w.len(), 1);
    assert!(!scratch.exists(), "il file di appoggio e' stato tolto");
    // anche all'uscita senza aver chiuso la scheda
    filtra(&mut w, "ERROR", false);
    let scratch = w.active().filter_scratch().unwrap().to_path_buf();
    assert!(scratch.exists());
    drop(w);
    assert!(!scratch.exists(), "tolto quando il Workspace sparisce");
    pulisci(d);
}

#[test]
fn a_filtered_view_cannot_be_edited() {
    let (d, mut w) = ws();
    filtra(&mut w, "ERROR", false);
    w.handle_key(modifica());
    assert!(!w.active().is_edit());
    assert!(schermo(&mut w).contains("read-only"), "{}", schermo(&mut w));
    pulisci(d);
}

#[test]
fn the_line_limit_is_said() {
    let (d, mut w) = ws();
    w.active_mut().set_filter_limit(1);
    filtra(&mut w, "ERROR", false);
    assert_eq!(w.active().document_text(), b"2026 ERROR due\n");
    assert!(
        schermo(&mut w).contains("only the first 1 lines"),
        "{}",
        schermo(&mut w)
    );
    pulisci(d);
}

#[test]
fn escape_cancels_a_filter_that_is_still_running() {
    let grande = "contatto mario@rossi nome cognome altro testo ancora\n".repeat(1_200_000);
    let (d, mut w) = ws_con(&grande);
    w.handle_key(alt('g'));
    svuota(&mut w);
    digita(&mut w, r"\w+@\w+");
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    w.handle_key(k(KeyCode::Enter));
    w.handle_key(k(KeyCode::Esc));
    assert!(w.wait_idle(Duration::from_secs(60)));
    assert_eq!(w.len(), 1, "nessuna scheda nuova");
    assert!(schermo(&mut w).contains("cancelled"), "{}", schermo(&mut w));
    pulisci(d);
}

#[test]
fn with_a_session_the_scratch_file_of_a_view_goes_away_on_exit_too() {
    let d = cartella();
    let p = d.join("a.log");
    std::fs::write(&p, TESTO).unwrap();
    let cfg = AppConfig::default();
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut w = Workspace::new(app, cfg).with_session(d.join("sessione"));
    w.resize(80, 14);
    assert!(w.wait_idle(Duration::from_secs(20)));
    filtra(&mut w, "ERROR", false);
    let scratch = w.active().filter_scratch().unwrap().to_path_buf();
    assert!(scratch.exists());
    drop(w);
    assert!(!scratch.exists());
    pulisci(d);
}

/// Alt+E: passa alla modifica (F4 e' Sostituisci).
fn modifica() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
}
