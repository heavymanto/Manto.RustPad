//! Il documento senza nome (E42): una scheda nuova e vuota ("Untitled 1"), in modifica, che si salva con nome. Ctrl+N la
//! apre, Ctrl+S sul documento senza nome (o Ctrl+Maiusc+S su qualunque) chiede il nome, il file vuoto che la regge sta nella
//! cartella `untitled` (accanto alla sessione, o fra i temporanei) e sparisce quando la scheda si chiude o prende un nome.
//! Scritti da ARCH: gli attesi sono calcolati a mano.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::ui::render_lines;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Una cartella nuova per ogni prova.
fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_untitled_test_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn pulisci(d: PathBuf) {
    let _ = std::fs::remove_dir_all(d);
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn ctrl_maiusc(c: char) -> KeyEvent {
    KeyEvent::new(
        KeyCode::Char(c.to_ascii_uppercase()),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    )
}

fn scrivi(a: &mut App, testo: &str) {
    for c in testo.chars() {
        if c == '\n' {
            a.handle_key(k(KeyCode::Enter));
        } else {
            a.handle_key(k(KeyCode::Char(c)));
        }
    }
}

/// L'a-capo che Invio scrive in un documento nuovo: CRLF su Windows, LF altrove.
fn a_capo() -> &'static str {
    if cfg!(windows) {
        "\r\n"
    } else {
        "\n"
    }
}

fn nuova(dir: &Path) -> App {
    let mut a = App::new_untitled(&dir.join("untitled"), 1, AppConfig::default()).unwrap();
    a.resize(70, 12);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a
}

fn scratch(dir: &Path, n: u32) -> PathBuf {
    dir.join("untitled").join(format!("untitled-{n}.txt"))
}

// ----- l'App -----

#[test]
fn a_new_untitled_document_is_empty_in_edit_mode_and_not_modified() {
    let d = cartella();
    let a = nuova(&d);
    assert!(a.is_untitled());
    assert_eq!(a.untitled_number(), Some(1));
    assert_eq!(a.file_name(), "Untitled 1");
    assert!(a.is_edit());
    assert!(!a.is_modified());
    assert_eq!(a.document_text(), b"");
    assert_eq!(a.cursor(), 0);
    assert_eq!(std::fs::read(scratch(&d, 1)).unwrap(), b"");
    pulisci(d);
}

#[test]
fn a_taken_scratch_name_is_never_overwritten() {
    let d = cartella();
    std::fs::create_dir_all(d.join("untitled")).unwrap();
    std::fs::write(scratch(&d, 1), b"da non perdere").unwrap();
    let a = App::new_untitled(&d.join("untitled"), 1, AppConfig::default()).unwrap();
    assert_eq!(a.untitled_number(), Some(2));
    assert_eq!(a.file_name(), "Untitled 2");
    assert_eq!(std::fs::read(scratch(&d, 1)).unwrap(), b"da non perdere");
    pulisci(d);
}

#[test]
fn typing_in_it_uses_the_platform_line_ending_and_marks_it_modified() {
    let d = cartella();
    let mut a = nuova(&d);
    scrivi(&mut a, "ab\ncd");
    assert!(a.is_modified());
    assert_eq!(
        a.document_text(),
        format!("ab{}cd", a_capo()).as_bytes(),
        "Invio scrive l'a-capo della piattaforma"
    );
    pulisci(d);
}

#[test]
fn save_on_an_untitled_document_asks_for_a_name_and_an_unmodified_one_has_nothing_to_save() {
    let d = cartella();
    let mut a = nuova(&d);
    a.handle_key(ctrl('s'));
    assert_eq!(
        a.dialog_kind(),
        None,
        "vuoto e non modificato: niente da salvare"
    );
    scrivi(&mut a, "x");
    a.handle_key(ctrl('s'));
    assert_eq!(a.dialog_kind(), Some(DialogKind::SaveAs));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.dialog_kind(), None);
    assert!(a.is_untitled() && a.is_modified(), "annullato: com'era");
    a.handle_key(k(KeyCode::F(2)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::SaveAs), "anche F2");
    pulisci(d);
}

#[test]
fn naming_it_saves_the_content_and_turns_it_into_a_normal_document() {
    let d = cartella();
    let mut a = nuova(&d);
    scrivi(&mut a, "ciao\nmondo");
    a.handle_key(ctrl('s'));
    let meta = d.join("nota.txt");
    scrivi(&mut a, meta.to_str().unwrap());
    a.handle_key(k(KeyCode::Enter));
    // dopo il nome, la codifica (la stessa del documento: un salvataggio normale)
    assert_eq!(a.dialog_kind(), Some(DialogKind::SaveEncoding));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(
        std::fs::read(&meta).unwrap(),
        format!("ciao{}mondo", a_capo()).as_bytes()
    );
    assert!(!a.is_untitled());
    assert_eq!(a.file_name(), "nota.txt");
    assert_eq!(a.path(), meta.canonicalize().unwrap().as_path());
    assert!(!a.is_modified());
    assert_eq!(a.dialog_kind(), None);
    assert!(
        !scratch(&d, 1).exists(),
        "il file vuoto di appoggio sparisce"
    );
    // e ora e' un documento come gli altri: Ctrl+S salva sullo stesso file
    scrivi(&mut a, "!");
    a.handle_key(ctrl('s'));
    assert!(a.wait_idle(Duration::from_secs(20)));
    let letto = std::fs::read(&meta).unwrap();
    // il cursore era in fondo: il '!' si aggiunge dopo "mondo"
    assert!(letto.ends_with(b"mondo!"), "{letto:?}");
    pulisci(d);
}

#[test]
fn an_existing_target_asks_before_replacing_it() {
    let d = cartella();
    let mut a = nuova(&d);
    scrivi(&mut a, "nuovo");
    let meta = d.join("c.txt");
    std::fs::write(&meta, b"vecchio").unwrap();
    a.handle_key(ctrl('s'));
    scrivi(&mut a, meta.to_str().unwrap());
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Overwrite));
    assert_eq!(std::fs::read(&meta).unwrap(), b"vecchio");
    // no: niente cambia
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.dialog_kind(), None);
    assert!(a.is_untitled() && a.is_modified());
    assert_eq!(std::fs::read(&meta).unwrap(), b"vecchio");
    // si': lo sostituisce
    a.handle_key(ctrl('s'));
    scrivi(&mut a, meta.to_str().unwrap());
    a.handle_key(k(KeyCode::Enter));
    a.handle_key(k(KeyCode::Enter));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(&meta).unwrap(), b"nuovo");
    assert!(!a.is_untitled());
    pulisci(d);
}

#[test]
fn a_target_in_a_missing_folder_fails_and_the_document_is_kept() {
    let d = cartella();
    let mut a = nuova(&d);
    scrivi(&mut a, "dati");
    a.handle_key(ctrl('s'));
    let ko = d.join("non_esiste").join("x.txt");
    scrivi(&mut a, ko.to_str().unwrap());
    a.handle_key(k(KeyCode::Enter));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(a.is_untitled() && a.is_modified());
    assert_eq!(a.document_text(), b"dati");
    assert!(a.flash().is_some(), "l'errore e' nella riga di stato");
    assert!(scratch(&d, 1).exists());
    pulisci(d);
}

#[test]
fn an_empty_name_does_nothing() {
    let d = cartella();
    let mut a = nuova(&d);
    scrivi(&mut a, "dati");
    a.handle_key(ctrl('s'));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
    assert!(a.is_untitled() && a.is_modified());
    pulisci(d);
}

#[test]
fn ctrl_shift_s_saves_any_document_under_a_new_name_and_leaves_the_old_file() {
    let d = cartella();
    let vecchio = d.join("a.txt");
    std::fs::write(&vecchio, b"uno\n").unwrap();
    let mut a = App::open(&vecchio, AppConfig::default()).unwrap();
    a.resize(70, 12);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.handle_key(k(KeyCode::F(4)));
    scrivi(&mut a, "+");
    a.handle_key(ctrl_maiusc('s'));
    assert_eq!(a.dialog_kind(), Some(DialogKind::SaveAs));
    // il campo parte dalla cartella del file, con il separatore in fondo: si scrive solo il nome
    let attesa = format!(
        "{}{}",
        d.canonicalize().unwrap().display(),
        std::path::MAIN_SEPARATOR
    );
    assert_eq!(a.save_as_text(), attesa);
    scrivi(&mut a, "b.txt");
    a.handle_key(k(KeyCode::Enter));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(d.join("b.txt")).unwrap(), b"+uno\n");
    assert_eq!(std::fs::read(&vecchio).unwrap(), b"uno\n");
    assert_eq!(a.file_name(), "b.txt");
    assert!(!a.is_modified());
    pulisci(d);
}

#[test]
fn the_save_as_prompt_shows_on_screen() {
    let d = cartella();
    let mut a = nuova(&d);
    scrivi(&mut a, "x");
    a.handle_key(ctrl('s'));
    scrivi(&mut a, "abc");
    let righe = render_lines(&mut a, 70, 12).join("\n");
    assert!(righe.contains("SAVE AS"), "{righe}");
    assert!(righe.contains("> abc"), "{righe}");
    pulisci(d);
}

#[test]
fn closing_an_untitled_document_with_changes_asks_and_s_goes_through_the_name() {
    let d = cartella();
    let mut a = nuova(&d);
    scrivi(&mut a, "x");
    a.request_close();
    assert_eq!(a.dialog_kind(), Some(DialogKind::Confirm));
    a.handle_key(k(KeyCode::Char('s')));
    assert_eq!(a.dialog_kind(), Some(DialogKind::SaveAs), "prima il nome");
    assert!(!a.should_quit());
    // Esc sul nome: si resta, senza uscire
    a.handle_key(k(KeyCode::Esc));
    assert!(!a.should_quit());
    assert!(a.is_modified());
    // di nuovo, stavolta con un nome: salva ed esce
    a.request_close();
    a.handle_key(k(KeyCode::Char('s')));
    let meta = d.join("fine.txt");
    scrivi(&mut a, meta.to_str().unwrap());
    a.handle_key(k(KeyCode::Enter));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(a.should_quit());
    assert_eq!(std::fs::read(&meta).unwrap(), b"x");
    pulisci(d);
}

// ----- il Workspace -----

fn ws_con_file(d: &Path) -> Workspace {
    let f = d.join("uno.txt");
    std::fs::write(&f, b"uno\n").unwrap();
    let cfg = AppConfig::default();
    let primo = App::open(&f, cfg.clone()).unwrap();
    // ogni prova ha la sua sessione (e quindi la sua cartella `untitled`): le prove girano in parallelo
    let mut ws = Workspace::new(primo, cfg).with_session(d.join("sessione"));
    ws.resize(70, 12);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    ws
}

fn titoli(ws: &Workspace) -> Vec<String> {
    ws.titles().into_iter().map(|(t, _)| t).collect()
}

#[test]
fn ctrl_n_opens_a_new_untitled_tab_and_brings_it_forward() {
    let d = cartella();
    let mut ws = ws_con_file(&d);
    ws.handle_key(ctrl('n'));
    assert_eq!(titoli(&ws), ["uno.txt", "Untitled 1"]);
    assert_eq!(ws.active_index(), 1);
    assert!(ws.active().is_edit() && ws.active().is_untitled());
    ws.handle_key(ctrl('n'));
    assert_eq!(titoli(&ws), ["uno.txt", "Untitled 1", "Untitled 2"]);
    assert_eq!(ws.active_index(), 2);
    pulisci(d);
}

#[test]
fn the_lowest_free_number_is_reused() {
    let d = cartella();
    let mut ws = ws_con_file(&d);
    ws.handle_key(ctrl('n'));
    ws.handle_key(ctrl('n'));
    ws.close(1); // "Untitled 1", vuoto e non modificato: si chiude subito
    assert_eq!(titoli(&ws), ["uno.txt", "Untitled 2"]);
    ws.handle_key(ctrl('n'));
    assert_eq!(titoli(&ws), ["uno.txt", "Untitled 2", "Untitled 1"]);
    pulisci(d);
}

#[test]
fn closing_an_untitled_tab_removes_its_scratch_file() {
    let d = cartella();
    let mut ws = ws_con_file(&d);
    ws.handle_key(ctrl('n'));
    let file = ws.active().path().to_path_buf();
    assert!(file.exists());
    ws.close(1);
    assert!(!file.exists());
    assert_eq!(ws.len(), 1);
    pulisci(d);
}

#[test]
fn opening_a_file_replaces_an_untouched_untitled_tab_but_not_a_modified_one() {
    let d = cartella();
    let mut ws = ws_con_file(&d);
    ws.handle_key(ctrl('n'));
    let vuoto = ws.active().path().to_path_buf();
    let due = d.join("due.txt");
    std::fs::write(&due, b"due\n").unwrap();
    ws.open(&due).unwrap();
    assert_eq!(
        titoli(&ws),
        ["uno.txt", "due.txt"],
        "la scheda vuota e' stata sostituita"
    );
    assert_eq!(ws.active().file_name(), "due.txt");
    assert!(!vuoto.exists());
    // con del testo scritto resta
    ws.handle_key(ctrl('n'));
    scrivi(ws.active_mut(), "appunti");
    let tre = d.join("tre.txt");
    std::fs::write(&tre, b"tre\n").unwrap();
    ws.open(&tre).unwrap();
    assert_eq!(titoli(&ws), ["uno.txt", "due.txt", "Untitled 1", "tre.txt"]);
    pulisci(d);
}

#[test]
fn closing_the_only_untitled_tab_quits_and_cleans_up() {
    let d = cartella();
    let dir = d.join("untitled");
    let a = App::new_untitled(&dir, 1, AppConfig::default()).unwrap();
    let file = a.path().to_path_buf();
    let mut ws = Workspace::new(a, AppConfig::default());
    ws.resize(70, 12);
    ws.close(0);
    assert!(ws.should_quit());
    assert!(!file.exists());
    pulisci(d);
}

#[test]
fn an_untitled_tab_with_text_comes_back_with_the_session() {
    let d = cartella();
    let sessione = d.join("sessione");
    let mut ws = ws_con_file(&d);
    ws.handle_key(ctrl('n'));
    scrivi(ws.active_mut(), "non ancora salvato");
    let file = ws.active().path().to_path_buf();
    assert!(
        file.starts_with(sessione.canonicalize().unwrap().join("untitled")),
        "il file vuoto sta nella cartella `untitled` della sessione: {file:?}"
    );
    // F10: la sessione si scrive e si esce senza chiedere
    ws.handle_key(k(KeyCode::F(10)));
    assert!(ws.should_quit());
    assert!(file.exists(), "uscendo la scheda non si cancella");
    drop(ws);
    let mut ws2 = Workspace::from_session(sessione.clone(), AppConfig::default()).unwrap();
    ws2.resize(70, 12);
    assert_eq!(titoli(&ws2), ["uno.txt", "Untitled 1"]);
    assert_eq!(ws2.active_index(), 1);
    let a = ws2.active();
    assert!(a.is_untitled() && a.is_modified());
    assert_eq!(a.document_text(), b"non ancora salvato");
    assert!(a.is_edit());
    pulisci(d);
}

#[test]
fn after_saving_with_a_name_the_session_points_to_the_real_file() {
    let d = cartella();
    let sessione = d.join("sessione");
    let mut ws = ws_con_file(&d);
    ws.handle_key(ctrl('n'));
    scrivi(ws.active_mut(), "salvato");
    ws.handle_key(ctrl('s'));
    let meta = d.join("vero.txt");
    scrivi(ws.active_mut(), meta.to_str().unwrap());
    ws.handle_key(k(KeyCode::Enter));
    ws.handle_key(k(KeyCode::Enter));
    assert!(ws.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(&meta).unwrap(), b"salvato");
    ws.handle_key(k(KeyCode::F(10)));
    drop(ws);
    let ws2 = Workspace::from_session(sessione, AppConfig::default()).unwrap();
    assert_eq!(titoli(&ws2), ["uno.txt", "vero.txt"]);
    assert!(!ws2.active().is_untitled());
    assert!(!ws2.active().is_modified());
    pulisci(d);
}

#[test]
fn without_a_session_the_scratch_file_lives_in_the_temp_folder() {
    let d = cartella();
    let f = d.join("uno.txt");
    std::fs::write(
        &f, b"uno
",
    )
    .unwrap();
    let cfg = AppConfig::default();
    let mut ws = Workspace::new(App::open(&f, cfg.clone()).unwrap(), cfg);
    ws.resize(70, 12);
    ws.handle_key(ctrl('n'));
    let file = ws.active().path().to_path_buf();
    assert!(file.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    let cartella_scratch = file
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy();
    assert!(
        cartella_scratch.starts_with("mrp-untitled-"),
        "{cartella_scratch}"
    );
    ws.close(1);
    assert!(!file.exists());
    pulisci(d);
}

// ----- l'avvio senza file -----

fn cfg_default() -> AppConfig {
    AppConfig::default()
}

#[test]
fn starting_without_files_and_without_a_session_opens_a_new_document() {
    let ws = Workspace::start(cfg_default(), None).unwrap();
    assert_eq!(titoli(&ws), ["Untitled 1"]);
    assert!(ws.active().is_untitled() && ws.active().is_edit());
    assert!(!ws.has_session());
}

#[test]
fn starting_without_files_with_an_empty_session_folder_opens_a_new_document_and_keeps_the_session_on(
) {
    let d = cartella();
    let sessione = d.join("sessione");
    let ws = Workspace::start(cfg_default(), Some(sessione.clone())).unwrap();
    assert_eq!(titoli(&ws), ["Untitled 1"]);
    assert!(ws.has_session());
    assert!(ws
        .active()
        .path()
        .starts_with(sessione.canonicalize().unwrap().join("untitled")));
    drop(ws);
    pulisci(d);
}

#[test]
fn starting_without_files_brings_back_the_last_session_untitled_text_included() {
    let d = cartella();
    let sessione = d.join("sessione");
    let mut ws = Workspace::start(cfg_default(), Some(sessione.clone())).unwrap();
    ws.resize(70, 12);
    scrivi(ws.active_mut(), "appunti di ieri");
    ws.handle_key(k(KeyCode::F(10)));
    assert!(ws.should_quit());
    drop(ws);
    let ws2 = Workspace::start(cfg_default(), Some(sessione)).unwrap();
    assert_eq!(titoli(&ws2), ["Untitled 1"]);
    assert_eq!(ws2.active().document_text(), b"appunti di ieri");
    drop(ws2);
    pulisci(d);
}

#[test]
fn starting_without_files_brings_back_the_last_session_files() {
    let d = cartella();
    let sessione = d.join("sessione");
    let mut ws = ws_con_file(&d);
    ws.handle_key(k(KeyCode::F(10)));
    drop(ws);
    let ws2 = Workspace::start(cfg_default(), Some(sessione)).unwrap();
    assert_eq!(
        titoli(&ws2),
        ["uno.txt"],
        "la sessione vince sul documento nuovo"
    );
    drop(ws2);
    pulisci(d);
}

#[test]
fn leaving_without_a_session_removes_the_scratch_files_and_leaving_with_one_keeps_them() {
    let ws = Workspace::start(cfg_default(), None).unwrap();
    let file = ws.active().path().to_path_buf();
    assert!(file.exists());
    drop(ws);
    assert!(!file.exists(), "senza sessione nessuno li riprenderebbe");
    let d = cartella();
    let ws = Workspace::start(cfg_default(), Some(d.join("sessione"))).unwrap();
    let file = ws.active().path().to_path_buf();
    drop(ws);
    assert!(
        file.exists(),
        "con la sessione tornano alla prossima apertura"
    );
    pulisci(d);
}
