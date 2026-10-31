//! La sessione nel TUI (E35): chiudendo l'editor le schede e le modifiche non salvate restano su disco e tornano alla
//! prossima apertura (come Notepad++, ma senza copiare il file: si salvano solo le modifiche). Scritti da ARCH: gli attesi
//! sono calcolati a mano sui testi di prova.

use mrp_core::encoding::Encoding;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Una cartella nuova per i file e una per la sessione.
fn cartelle() -> (PathBuf, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let base = std::env::temp_dir().join(format!("mrp_sess_test_{}_{}", std::process::id(), n));
    let dir = base.join("files");
    let sess = base.join("session");
    std::fs::create_dir_all(&dir).unwrap();
    (dir, sess)
}

fn pulisci(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

fn scrivi(dir: &Path, nome: &str, dati: &[u8]) -> PathBuf {
    let p = dir.join(nome);
    std::fs::write(&p, dati).unwrap();
    p
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// Un `Workspace` sui file `nomi` (il primo avanti), con la sessione in `sess` e senza attese fra le scritture.
fn ws(dir: &Path, sess: &Path, nomi: &[&str]) -> Workspace {
    let cfg = AppConfig::default();
    let primo = App::open(&dir.join(nomi[0]), cfg.clone()).unwrap();
    let mut w = Workspace::new(primo, cfg)
        .with_session(sess.to_path_buf())
        .with_session_timing(Duration::ZERO, Duration::ZERO);
    for n in &nomi[1..] {
        w.open(&dir.join(n)).unwrap();
    }
    w.select(0);
    w.resize(40, 8);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w
}

/// Scrive `testo` in testa al documento della scheda attiva (modalita' modifica, cursore all'inizio).
fn scrivi_in_testa(w: &mut Workspace, testo: &str) {
    w.handle_key(k(KeyCode::F(4)));
    w.handle_key(ctrl_home());
    for c in testo.chars() {
        w.handle_key(k(KeyCode::Char(c)));
    }
}

fn ctrl_home() -> KeyEvent {
    KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL)
}

fn due_schede() -> (PathBuf, PathBuf, Workspace) {
    let (dir, sess) = cartelle();
    scrivi(&dir, "a.txt", b"uno\ndue\n");
    scrivi(&dir, "b.txt", b"tre\nquattro\n");
    let mut w = ws(&dir, &sess, &["a.txt", "b.txt"]);
    scrivi_in_testa(&mut w, "X");
    (dir, sess, w)
}

fn riapri(sess: &Path) -> Workspace {
    let mut w = Workspace::from_session(sess.to_path_buf(), AppConfig::default())
        .expect("c'e' una sessione")
        .with_session_timing(Duration::ZERO, Duration::ZERO);
    w.resize(40, 8);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w
}

#[test]
fn closing_with_a_session_saves_and_quits_without_asking() {
    let (dir, sess, mut w) = due_schede();
    assert!(w.has_session());
    assert!(w.active().is_modified());
    w.handle_key(k(KeyCode::F(10)));
    assert!(w.should_quit());
    assert_eq!(w.active().dialog_kind(), None);
    // i file sul disco non sono stati toccati
    assert_eq!(std::fs::read(dir.join("a.txt")).unwrap(), b"uno\ndue\n");
    assert!(sess.join("manifest.txt").exists());
    assert!(sess.join("tab-0.mrps").exists());
    assert!(sess.join("tab-1.mrps").exists());
    assert!(!sess.join("tab-2.mrps").exists());
    pulisci(&dir);
}

#[test]
fn the_session_brings_back_tabs_text_cursor_mode_and_the_active_tab() {
    let (dir, sess, mut w) = due_schede();
    // nella seconda scheda: il cursore sulla seconda riga, in lettura
    w.select(1);
    w.handle_key(k(KeyCode::Down));
    assert_eq!(w.active().cursor(), 4);
    w.handle_key(ctrl('q'));
    assert!(w.should_quit());
    drop(w);

    let mut n = riapri(&sess);
    assert_eq!(n.len(), 2);
    assert_eq!(n.active_index(), 1, "era avanti la seconda");
    assert_eq!(n.active().file_name(), "b.txt");
    assert_eq!(n.active().cursor(), 4);
    assert!(!n.active().is_modified());
    assert!(!n.active().is_edit());
    n.select(0);
    assert_eq!(n.active().document_text(), b"Xuno\ndue\n");
    assert!(n.active().is_modified());
    assert!(n.active().is_edit());
    assert_eq!(n.active().cursor(), 1, "dopo la X scritta");
    // niente storia da annullare, ma il testo si puo' salvare
    n.handle_key(ctrl('z'));
    assert_eq!(n.active().document_text(), b"Xuno\ndue\n");
    n.handle_key(k(KeyCode::F(2)));
    for _ in 0..400 {
        n.tick();
        if !n.active().is_modified() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(!n.active().is_modified());
    assert_eq!(std::fs::read(dir.join("a.txt")).unwrap(), b"Xuno\ndue\n");
    pulisci(&dir);
}

#[test]
fn the_restored_tab_says_how_many_tabs_came_back_with_changes() {
    let (dir, sess, mut w) = due_schede();
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    let n = riapri(&sess);
    assert_eq!(
        n.active().flash(),
        Some("restored 1 tab(s) with unsaved changes")
    );
    pulisci(&dir);
}

#[test]
fn a_modified_tab_closed_by_hand_still_asks_even_with_a_session() {
    let (dir, _sess, mut w) = due_schede();
    w.handle_key(ctrl('w'));
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::Confirm));
    assert!(!w.should_quit());
    pulisci(&dir);
}

#[test]
fn closing_the_last_tab_by_hand_clears_the_session() {
    let (dir, sess) = cartelle();
    scrivi(&dir, "a.txt", b"uno\n");
    let mut w = ws(&dir, &sess, &["a.txt"]);
    w.save_session().unwrap();
    assert!(sess.join("tab-0.mrps").exists());
    w.handle_key(ctrl('w'));
    assert!(w.should_quit());
    assert!(!sess.join("tab-0.mrps").exists());
    drop(w);
    assert!(Workspace::from_session(sess.clone(), AppConfig::default()).is_none());
    pulisci(&dir);
}

#[test]
fn with_files_on_the_command_line_only_the_unsaved_tabs_come_back() {
    let (dir, sess, mut w) = due_schede();
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    // la nuova esecuzione apre solo b.txt: a.txt (con modifiche) torna, b.txt non si duplica
    let cfg = AppConfig::default();
    let primo = App::open(&dir.join("b.txt"), cfg.clone()).unwrap();
    let mut n = Workspace::new(primo, cfg).with_session(sess.clone());
    n.merge_session();
    n.resize(40, 8);
    assert_eq!(n.len(), 2);
    assert_eq!(
        n.active().file_name(),
        "b.txt",
        "resta avanti quello della riga di comando"
    );
    n.select(1);
    assert_eq!(n.active().file_name(), "a.txt");
    assert_eq!(n.active().document_text(), b"Xuno\ndue\n");
    pulisci(&dir);
}

#[test]
fn a_command_line_file_with_saved_changes_is_replaced_by_the_restored_tab() {
    let (dir, sess, mut w) = due_schede();
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    let cfg = AppConfig::default();
    let primo = App::open(&dir.join("a.txt"), cfg.clone()).unwrap();
    let mut n = Workspace::new(primo, cfg).with_session(sess.clone());
    n.merge_session();
    assert_eq!(n.len(), 1);
    assert_eq!(n.active().document_text(), b"Xuno\ndue\n");
    assert!(n.active().is_modified());
    pulisci(&dir);
}

#[test]
fn a_file_that_changed_on_disk_opens_clean_with_a_warning_and_the_copy_is_kept() {
    let (dir, sess, mut w) = due_schede();
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    scrivi(&dir, "a.txt", b"tutto diverso\n");
    let n = riapri(&sess);
    // a.txt c'e' ma senza le modifiche, b.txt come prima
    assert_eq!(n.len(), 2);
    let a = &n.tabs()[0];
    assert_eq!(a.file_name(), "a.txt");
    assert!(!a.is_modified());
    assert_eq!(a.document_text(), b"tutto diverso\n");
    let avviso = n.active().flash().or(a.flash()).unwrap_or("").to_string();
    assert!(
        avviso.contains("a.txt") && avviso.contains("changed"),
        "{avviso}"
    );
    let da_parte: Vec<_> = std::fs::read_dir(sess.join("stale")).unwrap().collect();
    assert_eq!(da_parte.len(), 1);
    pulisci(&dir);
}

#[test]
fn a_file_that_vanished_is_skipped_and_the_active_tab_keeps_pointing_at_the_right_one() {
    let (dir, sess) = cartelle();
    for n in ["a.txt", "b.txt", "c.txt"] {
        scrivi(&dir, n, b"uno\n");
    }
    let mut w = ws(&dir, &sess, &["a.txt", "b.txt", "c.txt"]);
    // b.txt con modifiche, e la terza scheda avanti
    w.select(1);
    scrivi_in_testa(&mut w, "Q");
    w.select(2);
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    std::fs::remove_file(dir.join("b.txt")).unwrap();
    let n = riapri(&sess);
    assert_eq!(n.len(), 2);
    assert_eq!(n.tabs()[0].file_name(), "a.txt");
    assert_eq!(n.tabs()[1].file_name(), "c.txt");
    assert_eq!(n.active().file_name(), "c.txt");
    // b.txt aveva modifiche: la sua copia e' stata messa da parte, e si dice
    assert_eq!(std::fs::read_dir(sess.join("stale")).unwrap().count(), 1);
    let avviso = n.active().flash().unwrap_or("").to_string();
    assert!(avviso.contains("b.txt"), "{avviso}");
    pulisci(&dir);
}

#[test]
fn clean_tabs_whose_file_vanished_are_dropped_without_noise() {
    let (dir, sess) = cartelle();
    scrivi(&dir, "a.txt", b"uno\n");
    scrivi(&dir, "b.txt", b"due\n");
    let mut w = ws(&dir, &sess, &["a.txt", "b.txt"]);
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    std::fs::remove_file(dir.join("b.txt")).unwrap();
    let n = riapri(&sess);
    assert_eq!(n.len(), 1);
    assert_eq!(n.active().flash(), None);
    assert!(!sess.join("stale").exists());
    pulisci(&dir);
}

#[test]
fn a_damaged_tab_file_is_skipped_and_a_damaged_manifest_is_ignored() {
    let (dir, sess, mut w) = due_schede();
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    // la scheda 0 con un byte alterato
    let p = sess.join("tab-0.mrps");
    let mut dati = std::fs::read(&p).unwrap();
    let mezzo = dati.len() / 2;
    dati[mezzo] ^= 0x55;
    std::fs::write(&p, dati).unwrap();
    let n = riapri(&sess);
    assert_eq!(n.len(), 1);
    assert_eq!(n.active().file_name(), "b.txt");
    assert_eq!(
        n.active().flash(),
        Some("a saved tab could not be read and was skipped")
    );
    drop(n);
    // l'elenco rovinato: nessuna sessione, nessun panico
    std::fs::write(sess.join("manifest.txt"), b"non e' un elenco").unwrap();
    assert!(Workspace::from_session(sess.clone(), AppConfig::default()).is_none());
    std::fs::write(sess.join("manifest.txt"), [0xFFu8, 0xFE, 0x00]).unwrap();
    assert!(Workspace::from_session(sess.clone(), AppConfig::default()).is_none());
    pulisci(&dir);
}

#[test]
fn the_session_rewrites_itself_on_ticks_only_when_something_changed() {
    let (dir, sess) = cartelle();
    scrivi(&dir, "a.txt", b"uno\n");
    let mut w = ws(&dir, &sess, &["a.txt"]);
    w.tick();
    assert!(sess.join("tab-0.mrps").exists(), "il primo giro scrive");
    // senza cambiamenti non si riscrive: tolgo il file e controllo che non torni
    std::fs::remove_file(sess.join("tab-0.mrps")).unwrap();
    w.tick();
    w.tick();
    assert!(!sess.join("tab-0.mrps").exists());
    // una modifica la riscrive, con il testo dentro
    scrivi_in_testa(&mut w, "Z");
    w.tick();
    assert!(sess.join("tab-0.mrps").exists());
    drop(w);
    let n = riapri(&sess);
    assert_eq!(n.active().document_text(), b"Zuno\n");
    pulisci(&dir);
}

#[test]
fn the_cursor_moving_is_a_change_too() {
    let (dir, sess) = cartelle();
    scrivi(&dir, "a.txt", b"uno\ndue\n");
    let mut w = ws(&dir, &sess, &["a.txt"]);
    w.tick();
    std::fs::remove_file(sess.join("tab-0.mrps")).unwrap();
    w.handle_key(k(KeyCode::Down));
    w.tick();
    assert!(sess.join("tab-0.mrps").exists());
    pulisci(&dir);
}

#[test]
fn only_one_process_owns_the_session_and_the_lock_goes_with_it() {
    let (dir, sess) = cartelle();
    scrivi(&dir, "a.txt", b"uno\n");
    let w1 = ws(&dir, &sess, &["a.txt"]);
    assert!(w1.has_session());
    // un secondo: senza sessione, e non riapre quella del primo
    let cfg = AppConfig::default();
    let primo = App::open(&dir.join("a.txt"), cfg.clone()).unwrap();
    let w2 = Workspace::new(primo, cfg.clone()).with_session(sess.clone());
    assert!(!w2.has_session());
    assert!(Workspace::from_session(sess.clone(), cfg.clone()).is_none());
    // chiuso il primo, il blocco e' libero
    drop(w1);
    let primo = App::open(&dir.join("a.txt"), cfg.clone()).unwrap();
    let w3 = Workspace::new(primo, cfg).with_session(sess.clone());
    assert!(w3.has_session());
    pulisci(&dir);
}

#[test]
fn no_temporary_files_are_left_in_the_session_folder() {
    let (dir, sess, mut w) = due_schede();
    w.handle_key(k(KeyCode::F(10)));
    for v in std::fs::read_dir(&sess).unwrap().flatten() {
        let n = v.file_name().to_string_lossy().into_owned();
        assert!(!n.ends_with(".tmp"), "{n}");
    }
    // e le schede tolte non lasciano il loro file
    let mut n = w;
    n.save_session().unwrap();
    pulisci(&dir);
}

#[test]
fn the_encoding_comes_back_with_the_edits() {
    let (dir, sess) = cartelle();
    let sj = Encoding::by_name("shift-jis").unwrap();
    let originale = sj.encode_str("日本\n").unwrap();
    scrivi(&dir, "j.txt", &originale);
    let cfg = AppConfig {
        encoding: Some(sj),
        ..AppConfig::default()
    };
    let primo = App::open(&dir.join("j.txt"), cfg.clone()).unwrap();
    let mut w = Workspace::new(primo, cfg).with_session(sess.clone());
    w.resize(40, 8);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w.handle_key(k(KeyCode::F(4)));
    w.paste_text("語");
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    let n = riapri(&sess);
    assert_eq!(n.active().encoding().name(), "shift-jis");
    assert_eq!(
        n.active().document_text(),
        sj.encode_str("語日本\n").unwrap()
    );
    pulisci(&dir);
}

#[test]
fn the_active_tab_is_found_again_when_an_earlier_tab_is_dropped() {
    let (dir, sess) = cartelle();
    for n in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        scrivi(&dir, n, b"uno\n");
    }
    let mut w = ws(&dir, &sess, &["a.txt", "b.txt", "c.txt", "d.txt"]);
    w.select(2);
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    let n = riapri(&sess);
    assert_eq!(n.len(), 3);
    assert_eq!(n.active().file_name(), "c.txt");
    assert_eq!(n.active_index(), 1);
    pulisci(&dir);
}

#[test]
fn a_change_that_leaves_the_cursor_where_it_was_still_rewrites_the_session() {
    let (dir, sess) = cartelle();
    scrivi(&dir, "a.txt", b"uno\ndue\n");
    let mut w = ws(&dir, &sess, &["a.txt"]);
    w.handle_key(k(KeyCode::F(4)));
    w.tick();
    assert!(sess.join("tab-0.mrps").exists());
    std::fs::remove_file(sess.join("tab-0.mrps")).unwrap();
    w.tick();
    assert!(!sess.join("tab-0.mrps").exists(), "niente di nuovo");
    // Canc davanti al cursore: il cursore sta fermo, il testo cambia
    w.handle_key(k(KeyCode::Delete));
    assert_eq!(w.active().cursor(), 0);
    assert_eq!(w.active().document_text(), b"no\ndue\n");
    w.tick();
    assert!(sess.join("tab-0.mrps").exists());
    pulisci(&dir);
}

#[test]
fn a_tab_restored_over_a_changed_file_opens_in_view_mode() {
    let (dir, sess, mut w) = due_schede();
    assert!(w.active().is_edit());
    w.handle_key(k(KeyCode::F(10)));
    drop(w);
    scrivi(&dir, "a.txt", b"tutto diverso\n");
    let n = riapri(&sess);
    assert_eq!(n.tabs()[0].file_name(), "a.txt");
    assert!(!n.tabs()[0].is_edit());
    pulisci(&dir);
}

#[test]
fn leftover_temporary_files_are_cleaned_on_the_next_write() {
    let (dir, sess) = cartelle();
    scrivi(&dir, "a.txt", b"uno\n");
    let mut w = ws(&dir, &sess, &["a.txt"]);
    w.save_session().unwrap();
    std::fs::write(sess.join("tab-7.mrps.tmp"), b"mezzo scritto").unwrap();
    std::fs::write(sess.join("manifest.txt.tmp"), b"mezzo scritto").unwrap();
    w.save_session().unwrap();
    assert!(!sess.join("tab-7.mrps.tmp").exists());
    assert!(!sess.join("manifest.txt.tmp").exists());
    pulisci(&dir);
}
