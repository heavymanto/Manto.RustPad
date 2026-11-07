//! Test della modifica, annulla/ripeti, salvataggio e uscita con modifiche. Scritti da ARCH prima dell'implementazione.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_edit_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, nome: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(nome);
        std::fs::write(&p, data).unwrap();
        p
    }
    fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
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

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_text(a: &mut App, s: &str) {
    for c in s.chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
}

/// App in MODO MODIFICA su `data`, terminale 40 x 10 (7 righe di testo), indice pronto.
fn edit_app(dir: &TestDir, data: &[u8]) -> (App, PathBuf) {
    let p = dir.file("f.txt", data);
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.handle_key(modifica());
    assert!(a.is_edit());
    (a, p)
}

/// La riga di stato a 80 colonne: a 40 il nome del file non ci sta.
fn status_larga(a: &mut App) -> String {
    a.resize(80, 9);
    let r = render_lines(a, 80, 9)[7].trim_end().to_string();
    a.resize(40, 9);
    r
}

/// La parte sinistra della riga di stato (messaggio o posizione): a destra ci sono nome, dimensione e modo, almeno tre spazi dopo.
fn status(a: &mut App) -> String {
    screen(a)[7]
        .split("   ")
        .next()
        .unwrap_or("")
        .trim_end()
        .to_string()
}

fn screen(a: &mut App) -> Vec<String> {
    render_lines(a, 40, 9)
        .into_iter()
        .map(|l| l.trim_end().to_string())
        .collect()
}

// ---------- scrivere ----------

#[test]
fn typing_inserts_at_the_cursor_and_marks_the_document_modified() {
    let dir = TestDir::new();
    let (mut a, p) = edit_app(&dir, b"ab\ncd\n");
    a.handle_key(k(KeyCode::Right));
    type_text(&mut a, "XY");
    assert_eq!(a.document_text(), b"aXYb\ncd\n");
    assert_eq!(a.cursor(), 3);
    assert!(a.is_modified());
    let s = screen(&mut a);
    assert!(
        status_larga(&mut a).contains("f.txt *"),
        "{:?}",
        status_larga(&mut a)
    );
    assert_eq!(s[0], "1│aXYb");
    assert_eq!(s[1], "2│cd");
    assert_eq!(status(&mut a), " Ln 1  Col 4  byte 3  37%", "3 * 100 / 8");
    assert_eq!(
        std::fs::read(&p).unwrap(),
        b"ab\ncd\n",
        "il file non si tocca finche' non si salva"
    );
}

#[test]
fn enter_backspace_delete_and_tab() {
    let dir = TestDir::new();
    let (mut a, _) = edit_app(&dir, b"ab\ncd\n");
    a.handle_key(k(KeyCode::Right));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.document_text(), b"a\nb\ncd\n");
    assert_eq!(a.cursor(), 2);
    let s = screen(&mut a);
    assert_eq!(
        (s[0].as_str(), s[1].as_str(), s[2].as_str()),
        ("1│a", "2│b", "3│cd"),
        "i numeri di riga seguono le modifiche"
    );
    a.handle_key(k(KeyCode::Backspace)); // toglie l'a-capo appena messo
    assert_eq!(a.document_text(), b"ab\ncd\n");
    assert_eq!(a.cursor(), 1);
    a.handle_key(k(KeyCode::Delete)); // toglie 'b'
    assert_eq!(a.document_text(), b"a\ncd\n");
    a.handle_key(k(KeyCode::Delete)); // toglie l'a-capo: le righe si uniscono
    assert_eq!(a.document_text(), b"acd\n");
    a.handle_key(k(KeyCode::Tab));
    assert_eq!(a.document_text(), b"a\tcd\n");
    // agli estremi non succede niente
    a.handle_key(ctrl_home());
    a.handle_key(k(KeyCode::Backspace));
    assert_eq!(a.document_text(), b"a\tcd\n");
    a.handle_key(ctrl_end());
    a.handle_key(k(KeyCode::Delete));
    assert_eq!(a.document_text(), b"a\tcd\n");
}

fn ctrl_home() -> KeyEvent {
    KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL)
}

fn ctrl_end() -> KeyEvent {
    KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL)
}

#[test]
fn multibyte_characters_are_edited_as_one_unit() {
    let dir = TestDir::new();
    let (mut a, _) = edit_app(&dir, b"xy\n");
    a.handle_key(k(KeyCode::Right));
    a.handle_key(KeyEvent::new(KeyCode::Char('è'), KeyModifiers::NONE));
    assert_eq!(a.document_text(), "xèy\n".as_bytes());
    assert_eq!(a.cursor(), 3, "due byte");
    a.handle_key(k(KeyCode::Left));
    assert_eq!(a.cursor(), 1, "Left salta il carattere intero");
    a.handle_key(k(KeyCode::Delete));
    assert_eq!(
        a.document_text(),
        b"xy\n",
        "Delete toglie tutti e due i byte"
    );
    a.handle_key(KeyEvent::new(KeyCode::Char('日'), KeyModifiers::NONE));
    assert_eq!(a.cursor(), 4);
    a.handle_key(k(KeyCode::Backspace));
    assert_eq!(
        a.document_text(),
        b"xy\n",
        "Backspace toglie il carattere intero (3 byte)"
    );
    assert_eq!(a.cursor(), 1);
    // una lettera maiuscola arriva con SHIFT: si inserisce
    a.handle_key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::SHIFT));
    assert_eq!(a.document_text(), b"xQy\n");
    // Ctrl+lettera non si inserisce come testo
    a.handle_key(ctrl('w'));
    assert_eq!(a.document_text(), b"xQy\n");
}

#[test]
fn ctrl_end_in_edit_mode_reaches_the_empty_last_row() {
    let dir = TestDir::new();
    let (mut a, _) = edit_app(&dir, b"ab\n");
    a.handle_key(ctrl_end());
    assert_eq!(a.cursor(), 3);
    let s = screen(&mut a);
    assert_eq!(s[0], "1│ab");
    assert_eq!(s[1], "2│", "la riga vuota finale, con il suo numero");
    assert_eq!(status(&mut a), " Ln 2  Col 1  byte 3  100%");
    type_text(&mut a, "z");
    assert_eq!(a.document_text(), b"ab\nz");
    // tornando in lettura il cursore non resta su una riga che non c'e' (qui non finisce piu' con a-capo: resta)
    a.handle_key(k(KeyCode::Enter));
    a.handle_key(vista());
    assert_eq!(
        a.cursor(),
        4,
        "il documento ora finisce con a-capo: in lettura il cursore va a len - 1"
    );
}

// ---------- annulla e ripeti ----------

#[test]
fn undo_and_redo_with_cursor_placement() {
    let dir = TestDir::new();
    let (mut a, _) = edit_app(&dir, b"ab\ncd\n");
    type_text(&mut a, "abc");
    assert_eq!(a.document_text(), b"abcab\ncd\n");
    a.handle_key(ctrl('z'));
    assert_eq!(a.document_text(), b"ab\ncd\n");
    assert_eq!(a.cursor(), 0);
    assert!(!a.is_modified(), "tornati allo stato del file");
    assert_eq!(status(&mut a), " undo");
    a.handle_key(ctrl('y'));
    assert_eq!(a.document_text(), b"abcab\ncd\n");
    assert_eq!(
        a.cursor(),
        3,
        "dopo il ripeti il cursore sta dopo il testo rimesso"
    );
    assert!(a.is_modified());
    assert_eq!(status(&mut a), " redo");
    // annullare una cancellazione rimette il testo e porta il cursore dopo
    a.handle_key(ctrl_home());
    a.handle_key(k(KeyCode::Delete));
    assert_eq!(a.document_text(), b"bcab\ncd\n");
    a.handle_key(ctrl('z'));
    assert_eq!(a.document_text(), b"abcab\ncd\n");
    assert_eq!(a.cursor(), 1);
}

#[test]
fn typing_in_a_row_is_one_step_and_moving_closes_it() {
    let dir = TestDir::new();
    let (mut a, _) = edit_app(&dir, b"\n");
    type_text(&mut a, "abc");
    a.handle_key(k(KeyCode::Left)); // chiude il passo
    type_text(&mut a, "X");
    assert_eq!(a.document_text(), b"abXc\n");
    a.handle_key(ctrl('z'));
    assert_eq!(a.document_text(), b"abc\n", "annulla solo la X");
    a.handle_key(ctrl('z'));
    assert_eq!(a.document_text(), b"\n", "poi tutto 'abc'");
    a.handle_key(ctrl('z'));
    assert_eq!(status(&mut a), " nothing to undo");
    a.handle_key(ctrl('y'));
    a.handle_key(ctrl('y'));
    assert_eq!(a.document_text(), b"abXc\n");
    a.handle_key(ctrl('y'));
    assert_eq!(status(&mut a), " nothing to redo");
    // Enter chiude il passo
    a.handle_key(ctrl_home());
    type_text(&mut a, "1");
    a.handle_key(k(KeyCode::Enter));
    type_text(&mut a, "2");
    a.handle_key(ctrl('z'));
    assert_eq!(a.document_text(), b"1\nabXc\n");
    a.handle_key(ctrl('z'));
    assert_eq!(a.document_text(), b"abXc\n");
}

#[test]
fn undo_and_save_are_not_for_view_mode_but_the_old_changes_stay() {
    let dir = TestDir::new();
    let (mut a, _) = edit_app(&dir, b"ab\n");
    type_text(&mut a, "x");
    a.handle_key(vista()); // torna in lettura con una modifica non salvata
    assert!(!a.is_edit());
    assert!(a.is_modified());
    a.handle_key(ctrl('z'));
    assert_eq!(status(&mut a), " read-only: press Alt+E to edit");
    assert_eq!(a.document_text(), b"xab\n");
}

// ---------- salvataggio ----------

#[test]
fn save_writes_the_file_and_the_index_follows() {
    let dir = TestDir::new();
    let (mut a, p) = edit_app(&dir, b"ab\ncd\n");
    a.handle_key(ctrl('s'));
    assert_eq!(status(&mut a), " nothing to save");
    a.handle_key(k(KeyCode::Right));
    type_text(&mut a, "XY");
    a.handle_key(ctrl('s'));
    assert_eq!(
        a.dialog_kind(),
        Some(DialogKind::Saving),
        "il salvataggio gira in background"
    );
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(std::fs::read(&p).unwrap(), b"aXYb\ncd\n");
    assert!(!a.is_modified());
    let s = screen(&mut a);
    assert_eq!(status(&mut a), " saved: f.txt");
    let larga = status_larga(&mut a);
    assert!(larga.contains("f.txt") && !larga.contains('*'), "{larga:?}");
    assert_eq!(
        s[0], "1│aXYb",
        "i numeri di riga tornano dopo la ricostruzione dell'indice"
    );
    assert_eq!(
        dir.names(),
        vec!["f.txt".to_string()],
        "nessun file temporaneo"
    );
    // F2 equivale a Ctrl+S (il cursore e' rimasto dov'era: prima della 'b')
    type_text(&mut a, "!");
    a.handle_key(k(KeyCode::F(2)));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(&p).unwrap(), b"aXY!b\ncd\n");
}

#[test]
fn save_works_from_view_mode_too_when_there_are_changes() {
    let dir = TestDir::new();
    let (mut a, p) = edit_app(&dir, b"ab\n");
    type_text(&mut a, "x");
    a.handle_key(modifica());
    a.handle_key(ctrl('s'));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(&p).unwrap(), b"xab\n");
}

#[test]
fn save_refuses_when_the_file_changed_on_disk() {
    let dir = TestDir::new();
    let (mut a, p) = edit_app(&dir, b"ab\n");
    type_text(&mut a, "x");
    // un altro programma allunga il file
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"altro\n").unwrap();
    }
    a.handle_key(ctrl('s'));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(status(&mut a), " the file changed on disk: save refused");
    assert!(a.is_modified(), "le modifiche restano");
    assert_eq!(
        std::fs::read(&p).unwrap(),
        b"ab\naltro\n",
        "il lavoro dell'altro non si sovrascrive"
    );
    assert_eq!(dir.names(), vec!["f.txt".to_string()]);
}

#[test]
fn escape_while_saving_cancels_or_the_save_had_already_finished() {
    // il salvataggio di un file piccolo puo' finire prima dell'Esc: i due esiti sono entrambi corretti, ma coerenti
    let dir = TestDir::new();
    let big: Vec<u8> = (0..8 * 1024 * 1024)
        .map(|i| b'a' + (i % 23) as u8)
        .collect();
    let (mut a, p) = edit_app(&dir, &big);
    type_text(&mut a, "Z");
    a.handle_key(ctrl('s'));
    a.handle_key(k(KeyCode::Esc));
    assert!(a.wait_idle(Duration::from_secs(30)));
    let su_disco = std::fs::read(&p).unwrap();
    if a.is_modified() {
        assert_eq!(su_disco, big, "annullato: il file originale e' intatto");
        assert_eq!(status(&mut a), " save cancelled");
    } else {
        assert_eq!(su_disco.len(), big.len() + 1);
        assert_eq!(su_disco[0], b'Z');
    }
    assert_eq!(
        dir.names(),
        vec!["f.txt".to_string()],
        "nessun file temporaneo in nessun caso"
    );
}

// ---------- uscita con modifiche ----------

#[test]
fn quitting_with_changes_asks_what_to_do() {
    let dir = TestDir::new();
    let (mut a, p) = edit_app(&dir, b"ab\n");
    type_text(&mut a, "x");
    a.handle_key(k(KeyCode::F(10)));
    assert!(!a.should_quit());
    assert_eq!(a.dialog_kind(), Some(DialogKind::Confirm));
    let s = screen(&mut a);
    for atteso in [
        "UNSAVED CHANGES",
        "The document has unsaved changes.",
        "S Save and quit",
        "D Discard changes",
        "C Cancel",
    ] {
        assert!(s.iter().any(|l| l.contains(atteso)), "manca {atteso:?}");
    }
    a.handle_key(k(KeyCode::Char('c')));
    assert_eq!(a.dialog_kind(), None);
    assert!(!a.should_quit());
    a.handle_key(ctrl('q'));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.dialog_kind(), None, "Esc come Annulla");
    assert!(!a.should_quit());
    a.handle_key(ctrl('q'));
    a.handle_key(k(KeyCode::Char('d')));
    assert!(a.should_quit(), "scarta ed esce");
    assert_eq!(
        std::fs::read(&p).unwrap(),
        b"ab\n",
        "il file non e' stato toccato"
    );
}

#[test]
fn save_and_quit() {
    let dir = TestDir::new();
    let (mut a, p) = edit_app(&dir, b"ab\n");
    type_text(&mut a, "x");
    a.handle_key(k(KeyCode::F(10)));
    a.handle_key(k(KeyCode::Char('s')));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(a.should_quit(), "salvato, poi esce");
    assert_eq!(std::fs::read(&p).unwrap(), b"xab\n");
}

#[test]
fn save_and_quit_stays_open_if_the_save_fails() {
    let dir = TestDir::new();
    let (mut a, p) = edit_app(&dir, b"ab\n");
    type_text(&mut a, "x");
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"altro\n").unwrap();
    }
    a.handle_key(k(KeyCode::F(10)));
    a.handle_key(k(KeyCode::Char('s')));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(
        !a.should_quit(),
        "il salvataggio e' fallito: non si perde il lavoro uscendo"
    );
    assert!(a.is_modified());
    assert_eq!(status(&mut a), " the file changed on disk: save refused");
}

#[test]
fn quitting_without_changes_is_immediate_even_after_undoing_everything() {
    let dir = TestDir::new();
    let (mut a, _) = edit_app(&dir, b"ab\n");
    type_text(&mut a, "x");
    a.handle_key(ctrl('z'));
    a.handle_key(k(KeyCode::F(10)));
    assert!(a.should_quit());
}

// ---- trovato dalla revisione R26A ----

/// Una riga lunga piu' del limite di scansione (128 KiB) con una "é" (2 byte) a cavallo del taglio: le righe visuali si
/// spezzano li', e il cursore (Giu', un clic) puo' finire in mezzo al carattere.
fn riga_con_taglio_dentro_a_un_carattere() -> (App, std::path::PathBuf) {
    let mut dati = vec![b'a'; 131_071];
    dati.extend_from_slice("é".as_bytes()); // byte 131071 e 131072: il taglio cade fra i due
    dati.extend_from_slice(b"b\n");
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_taglio_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, &dati).unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 9);
    assert!(a.wait_idle(std::time::Duration::from_secs(20)));
    a.handle_key(modifica());
    // Giu': la seconda riga visuale comincia al taglio, dentro la "é"
    a.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(a.cursor(), 131_072, "il cursore e' in mezzo alla é");
    (a, p)
}

#[test]
fn typing_at_a_cut_inside_a_character_does_not_split_it() {
    let (mut a, p) = riga_con_taglio_dentro_a_un_carattere();
    a.handle_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
    let t = a.document_text();
    assert_eq!(&t[131_071..], "Xéb\n".as_bytes(), "la X va prima della é");
    assert!(std::str::from_utf8(&t).is_ok());
    let _ = std::fs::remove_file(p);
}

#[test]
fn delete_and_backspace_at_a_cut_inside_a_character_take_whole_characters() {
    let (mut a, p) = riga_con_taglio_dentro_a_un_carattere();
    a.handle_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    let t = a.document_text();
    assert_eq!(&t[131_071..], b"b\n", "Delete toglie la é intera");
    let _ = std::fs::remove_file(p);
    let (mut a, p) = riga_con_taglio_dentro_a_un_carattere();
    a.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    let t = a.document_text();
    assert_eq!(
        &t[131_070..],
        "éb\n".as_bytes(),
        "Backspace toglie la a prima"
    );
    assert!(std::str::from_utf8(&t).is_ok());
    let _ = std::fs::remove_file(p);
}

/// Alt+E: passa alla modifica (F4 e' Sostituisci).
fn modifica() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
}

/// Alt+V: torna in sola lettura.
fn vista() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT)
}
