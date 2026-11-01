//! I dialoghi come dati (B4a-1): `App::dialog_view` descrive il dialogo aperto (aiuto, menu di tema, lingua e codifica,
//! "vai a", conferma di uscita, salvataggio) con testi e valori, e le azioni (`dialog_set_text`, `dialog_pick`,
//! `dialog_confirm`, `dialog_submit`, `dialog_cancel`) lo guidano: il TUI continua a disegnarli a testo, la finestra li
//! disegna con widget veri. Senza la cornice del terminale (`chrome = false`) il disegno a testo dei dialoghi che hanno una
//! vista sparisce. Scritti da ARCH: gli attesi sono calcolati a mano.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::dialogs::{ConfirmChoice, DialogView, MenuItem};
use mrp_tui::theme::ThemeId;
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_dlg_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    p
}

fn app(dati: &[u8], chrome: bool) -> (App, PathBuf) {
    let p = file(dati);
    let cfg = AppConfig {
        chrome,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(70, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn voce(label: &str, detail: &str) -> MenuItem {
    MenuItem {
        label: label.to_string(),
        detail: detail.to_string(),
    }
}

// ----- l'aiuto -----

#[test]
fn help_is_a_table_of_keys_and_descriptions() {
    let (mut a, p) = app(b"uno\n", false);
    assert!(a.dialog_view().is_none());
    a.handle_key(k(KeyCode::F(1)));
    let DialogView::Help { title, rows } = a.dialog_view().unwrap() else {
        panic!("non e' l'aiuto");
    };
    assert_eq!(title, "KEYS");
    assert_eq!(rows.len(), 21, "una riga per ogni riga dell'aiuto");
    assert_eq!(
        rows[0],
        ("Arrows PgUp PgDn Home End".to_string(), "move".to_string())
    );
    assert!(rows.contains(&(
        "Ctrl+H".to_string(),
        "replace (text, glob, regex; this or all tabs)".to_string()
    )));
    assert!(rows.contains(&(
        "Alt+L".to_string(),
        "colour log lines (levels, times, IPs)".to_string()
    )));
    assert!(rows.contains(&(
        "Alt+F".to_string(),
        "follow the end of the file (like tail -f)".to_string()
    )));
    assert!(rows.contains(&(
        "Alt+W".to_string(),
        "show spaces, tabs and line ends".to_string()
    )));
    assert_eq!(
        rows.last().unwrap(),
        &("Esc".to_string(), "close a window".to_string())
    );
    // Esc chiude
    a.dialog_cancel();
    assert_eq!(a.dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

// ----- i menu -----

#[test]
fn the_theme_menu_lists_the_six_themes_with_their_blurb_and_the_current_one_is_selected() {
    let (mut a, p) = app(b"uno\n", false);
    a.handle_key(k(KeyCode::F(6)));
    let DialogView::Menu {
        title,
        items,
        selected,
        note,
    } = a.dialog_view().unwrap()
    else {
        panic!("non e' un menu");
    };
    assert_eq!(title, "THEME");
    assert_eq!(
        items,
        vec![
            voce("Night (blue)", "classic blue panels, yellow selection"),
            voce("Debian (blue)", "installer blue, red selection"),
            voce("Console (default)", "white on black, yellow accents"),
            voce("Minimal", "your terminal's own colors"),
            voce("Dark", "soft dark gray, easy on the eyes"),
            voce("Light", "white page, dark text"),
        ]
    );
    assert_eq!(selected, 2, "il tema di partenza e' Console, il terzo");
    assert_eq!(note, None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn picking_a_theme_applies_it_and_closes_the_menu() {
    let (mut a, p) = app(b"uno\n", false);
    a.handle_key(k(KeyCode::F(6)));
    a.dialog_select(0);
    let DialogView::Menu { selected, .. } = a.dialog_view().unwrap() else {
        panic!();
    };
    assert_eq!(selected, 0, "selezionare non applica");
    assert_eq!(a.theme_id(), ThemeId::Console);
    a.dialog_pick(3);
    assert_eq!(a.theme_id(), ThemeId::Minimal);
    assert_eq!(a.dialog_kind(), None);
    assert!(a.dialog_view().is_none());
    // una voce che non c'e': non fa niente e non panica
    a.handle_key(k(KeyCode::F(6)));
    a.dialog_pick(99);
    assert_eq!(a.dialog_kind(), Some(DialogKind::Theme));
    let DialogView::Menu { selected, .. } = a.dialog_view().unwrap() else {
        panic!();
    };
    assert_eq!(selected, 3, "la voce evidenziata resta quella di prima");
    a.dialog_cancel();
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_language_menu_has_english_first_and_picking_changes_the_texts() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang");
    let (lingue, _) = mrp_core::lang::registry_in(Some(dir.as_path()));
    let p = file(b"uno\n");
    let cfg = AppConfig {
        chrome: false,
        langs: lingue,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(70, 24);
    a.handle_key(k(KeyCode::F(7)));
    let DialogView::Menu {
        title,
        items,
        selected,
        note,
    } = a.dialog_view().unwrap()
    else {
        panic!();
    };
    assert_eq!(title, "LANGUAGE");
    // l'avviso sulle traduzioni di una IA c'e' se e solo se una voce porta il marchio
    let con_marchio = items.iter().any(|i| !i.detail.is_empty());
    assert_eq!(note.is_some(), con_marchio);
    if let Some(n) = note {
        assert_eq!(n, "AI-translated: may be imprecise or wrong");
        assert!(items.iter().any(|i| i.detail == "[AI]"));
    }
    assert_eq!(items.len(), 7);
    assert_eq!(items[selected].label, "English", "la lingua corrente");
    let it = items
        .iter()
        .position(|i| i.label == "Italiano")
        .expect("c'e' l'italiano");
    a.dialog_pick(it);
    assert_eq!(a.lang_code(), "it");
    a.handle_key(k(KeyCode::F(1)));
    let DialogView::Help { title, .. } = a.dialog_view().unwrap() else {
        panic!();
    };
    assert_eq!(title, "TASTI");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_encoding_menu_lists_every_encoding_and_picking_one_applies_it() {
    let (mut a, p) = app(b"caf\xe9\n", false);
    a.handle_key(k(KeyCode::F(9)));
    let DialogView::Menu {
        title,
        items,
        selected,
        ..
    } = a.dialog_view().unwrap()
    else {
        panic!();
    };
    assert_eq!(title, "ENCODING");
    assert_eq!(items.len(), mrp_core::encoding::Encoding::all().len());
    assert_eq!(
        items[selected].label,
        a.encoding().name(),
        "parte dalla codifica corrente"
    );
    assert!(items.iter().all(|i| i.detail.is_empty()));
    let w = items
        .iter()
        .position(|i| i.label == "windows-1252")
        .unwrap();
    a.dialog_pick(w);
    assert_eq!(a.encoding().name(), "windows-1252");
    assert_eq!(a.dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

// ----- "vai a" e gli altri prompt -----

#[test]
fn goto_is_a_prompt_with_a_hint_and_runs_the_text() {
    let (mut a, p) = app(b"uno\ndue\ntre\nquattro\n", false);
    a.handle_key(ctrl('g'));
    let DialogView::Prompt { title, hint, text } = a.dialog_view().unwrap() else {
        panic!("non e' un prompt");
    };
    assert_eq!(
        (title.as_str(), hint.as_str(), text.as_str()),
        ("GO TO", "line number, @byte offset or %percent", "")
    );
    a.dialog_set_text("3");
    let DialogView::Prompt { text, .. } = a.dialog_view().unwrap() else {
        panic!();
    };
    assert_eq!(text, "3");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.cursor(), 8, "riga 3: \"uno\\ndue\\n\" sono 8 byte");
    // Esc annulla senza muovere
    a.handle_key(ctrl('g'));
    a.dialog_set_text("1");
    a.dialog_cancel();
    assert_eq!((a.dialog_kind(), a.cursor()), (None, 8));
    let _ = std::fs::remove_file(p);
}

#[test]
fn save_as_is_a_prompt_too_and_an_existing_file_asks_before_replacing_it() {
    let (mut a, p) = app(b"uno\n", false);
    a.handle_key(k(KeyCode::F(4)));
    a.handle_key(k(KeyCode::Char('x')));
    let altro = file(b"vecchio");
    a.handle_key(KeyEvent::new(
        KeyCode::Char('S'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ));
    let DialogView::Prompt { title, text, .. } = a.dialog_view().unwrap() else {
        panic!();
    };
    assert_eq!(title, "SAVE AS");
    assert!(
        text.ends_with(std::path::MAIN_SEPARATOR),
        "parte dalla cartella: {text}"
    );
    a.dialog_set_text(altro.to_str().unwrap());
    a.dialog_submit();
    let DialogView::Overwrite { title, text } = a.dialog_view().unwrap() else {
        panic!("il file esiste");
    };
    assert_eq!(title, "FILE EXISTS");
    assert!(
        text.contains(altro.file_name().unwrap().to_str().unwrap()),
        "{text}"
    );
    a.dialog_cancel();
    assert_eq!(std::fs::read(&altro).unwrap(), b"vecchio");
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(altro);
}

// ----- la conferma di uscita -----

#[test]
fn the_exit_confirmation_has_three_choices_without_the_hotkey_letters() {
    let (mut a, p) = app(b"uno\n", false);
    a.handle_key(k(KeyCode::F(4)));
    a.handle_key(k(KeyCode::Char('x')));
    a.request_close();
    let DialogView::Confirm {
        title,
        text,
        save,
        discard,
        cancel,
    } = a.dialog_view().unwrap()
    else {
        panic!("non e' la conferma");
    };
    assert_eq!(
        (
            title.as_str(),
            text.as_str(),
            save.as_str(),
            discard.as_str(),
            cancel.as_str()
        ),
        (
            "UNSAVED CHANGES",
            "The document has unsaved changes.",
            "Save and quit",
            "Discard changes",
            "Cancel"
        )
    );
    a.dialog_confirm(ConfirmChoice::Cancel);
    assert_eq!(a.dialog_kind(), None);
    assert!(!a.should_quit());
    a.request_close();
    a.dialog_confirm(ConfirmChoice::Discard);
    assert!(a.should_quit());
    assert_eq!(
        std::fs::read(&p).unwrap(),
        b"uno\n",
        "scartato: il file e' quello di prima"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn confirming_with_save_saves_and_quits() {
    let (mut a, p) = app(b"uno\n", false);
    a.handle_key(k(KeyCode::F(4)));
    a.handle_key(k(KeyCode::Char('x')));
    a.request_close();
    a.dialog_confirm(ConfirmChoice::Save);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(a.should_quit());
    assert_eq!(std::fs::read(&p).unwrap(), b"xuno\n");
    let _ = std::fs::remove_file(p);
}

// ----- chi disegna cosa -----

#[test]
fn a_dialog_with_a_view_is_not_drawn_as_text_without_the_terminal_chrome() {
    let (mut a, p) = app(b"uno\n", false);
    a.handle_key(k(KeyCode::F(1)));
    assert!(a.dialog_has_view());
    let tutto = render_lines(&mut a, 70, 24).join("\n");
    assert!(
        !tutto.contains("KEYS"),
        "la finestra lo disegna con i suoi widget:\n{tutto}"
    );
    a.dialog_cancel();
    assert!(!a.dialog_has_view());
    // con la cornice (il terminale) si disegna come sempre
    let (mut b, q) = app(b"uno\n", true);
    b.handle_key(k(KeyCode::F(1)));
    assert!(b.dialog_has_view());
    let tutto = render_lines(&mut b, 70, 24).join("\n");
    assert!(tutto.contains("KEYS"), "{tutto}");
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}
