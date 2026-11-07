//! "Salva con nome" con la scelta della codifica (4.1): scelto il nome, un dialogo chiede la codifica (e il BOM); la stessa
//! codifica e' un salvataggio normale, un'altra converte il contenuto mentre lo scrive e il documento diventa il file nuovo.
//! Gli attesi sono byte calcolati a mano dalle tabelle delle codifiche. Scritti da ARCH.

use mrp_core::encoding::Encoding;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::dialogs::DialogView;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_saveenc_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn app(dati: &[u8]) -> (App, PathBuf, PathBuf) {
    let d = cartella();
    let p = d.join("a.txt");
    std::fs::write(&p, dati).unwrap();
    let mut a = App::open(
        &p,
        AppConfig {
            encoding: Some(Encoding::Utf8),
            ..AppConfig::default()
        },
    )
    .unwrap();
    a.resize(80, 23);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p, d)
}

fn indice(nome: &str) -> usize {
    let e = Encoding::by_name(nome).unwrap();
    Encoding::all().iter().position(|&x| x == e).unwrap()
}

fn tasto(a: &mut App, c: KeyCode) {
    a.handle_key(KeyEvent::new(c, KeyModifiers::NONE));
}

fn flash(a: &mut App) -> String {
    a.status().flash.unwrap_or_default()
}

#[test]
fn choosing_the_name_opens_the_encoding_dialog_starting_from_the_current_encoding() {
    let (mut a, _p, d) = app(b"abc\n");
    a.save_as_choose(d.join("b.txt"));
    assert_eq!(a.dialog_kind(), Some(DialogKind::SaveEncoding));
    let Some(DialogView::SaveEncoding {
        items,
        selected,
        bom,
        ..
    }) = a.dialog_view()
    else {
        panic!("manca la vista");
    };
    assert_eq!(items.len(), Encoding::all().len());
    assert_eq!(items[selected].label, "utf-8");
    // UTF-8 puo' avere il BOM (il file non lo ha: spento); una code page no
    assert_eq!(bom, Some(false));
    a.dialog_select(indice("windows-1252"));
    let Some(DialogView::SaveEncoding { bom, .. }) = a.dialog_view() else {
        panic!("manca la vista");
    };
    assert_eq!(bom, None, "le code page non hanno il BOM");
}

#[test]
fn a_different_encoding_converts_while_saving_and_the_document_becomes_the_new_file() {
    let (mut a, p, d) = app("caffè\n".as_bytes());
    let nuovo = d.join("b.txt");
    a.save_as_choose(nuovo.clone());
    a.dialog_select(indice("windows-1252"));
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    // il file nuovo e' in windows-1252: è = E8
    assert_eq!(
        std::fs::read(&nuovo).unwrap(),
        [0x63, 0x61, 0x66, 0x66, 0xE8, 0x0A]
    );
    assert_eq!(
        std::fs::read(&p).unwrap(),
        "caffè\n".as_bytes(),
        "l'originale non si tocca"
    );
    assert_eq!(a.encoding(), Encoding::by_name("windows-1252").unwrap());
    assert_eq!(a.document_text(), [0x63, 0x61, 0x66, 0x66, 0xE8, 0x0A]);
    assert!(!a.is_modified());
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.cursor(), 0);
    assert!(flash(&mut a).contains("windows-1252"), "{}", flash(&mut a));
    assert!(a.status().file_name.ends_with("b.txt"));
}

#[test]
fn the_bom_box_decides_the_bom_of_a_utf16_file() {
    let (mut a, _p, d) = app("A€\n".as_bytes());
    let nuovo = d.join("b.txt");
    a.save_as_choose(nuovo.clone());
    a.dialog_select(indice("utf-16le"));
    a.dialog_set_bom(true);
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    let atteso = [0xFF, 0xFE, 0x41, 0x00, 0xAC, 0x20, 0x0A, 0x00];
    assert_eq!(std::fs::read(&nuovo).unwrap(), atteso);
    assert_eq!(a.encoding(), Encoding::Utf16Le);
    assert_eq!(a.document_text(), atteso);
    // senza BOM
    let (mut a, _p, d) = app("A€\n".as_bytes());
    let nuovo = d.join("c.txt");
    a.save_as_choose(nuovo.clone());
    a.dialog_select(indice("utf-16be"));
    a.dialog_set_bom(false);
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(
        std::fs::read(&nuovo).unwrap(),
        [0x00, 0x41, 0x20, 0xAC, 0x00, 0x0A]
    );
}

#[test]
fn the_bom_box_starts_on_when_the_file_has_a_bom() {
    let (mut a, _p, d) = app(&[0xEF, 0xBB, 0xBF, b'a', b'\n']);
    a.save_as_choose(d.join("b.txt"));
    let Some(DialogView::SaveEncoding { bom, .. }) = a.dialog_view() else {
        panic!("manca la vista");
    };
    assert_eq!(bom, Some(true));
    // tolto il BOM con la stessa codifica: converte (nel file non c'e' piu')
    a.dialog_set_bom(false);
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(d.join("b.txt")).unwrap(), [b'a', b'\n']);
    assert_eq!(a.encoding(), Encoding::Utf8);
}

#[test]
fn the_same_encoding_and_bom_is_a_plain_save_as() {
    let (mut a, _p, d) = app("é\n".as_bytes());
    let nuovo = d.join("b.txt");
    a.save_as_choose(nuovo.clone());
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(&nuovo).unwrap(), "é\n".as_bytes());
    assert_eq!(a.encoding(), Encoding::Utf8);
    assert!(
        flash(&mut a).to_lowercase().contains("saved"),
        "{}",
        flash(&mut a)
    );
}

#[test]
fn an_unrepresentable_character_saves_nothing_and_puts_the_cursor_on_it() {
    let (mut a, p, d) = app("ab😀cd\n".as_bytes());
    let nuovo = d.join("b.txt");
    a.save_as_choose(nuovo.clone());
    a.dialog_select(indice("windows-1252"));
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(!nuovo.exists());
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.encoding(), Encoding::Utf8, "il documento resta com'era");
    assert_eq!(a.document_text(), "ab😀cd\n".as_bytes());
    assert_eq!(a.cursor(), 2, "il cursore va sul carattere");
    let f = flash(&mut a);
    assert!(f.contains("U+1F600") && f.contains("windows-1252"), "{f}");
    assert_eq!(std::fs::read(&p).unwrap(), "ab😀cd\n".as_bytes());
}

#[test]
fn escape_gives_up_without_writing_anything() {
    let (mut a, _p, d) = app(b"abc\n");
    a.save_as_choose(d.join("b.txt"));
    tasto(&mut a, KeyCode::Esc);
    assert_eq!(a.dialog_kind(), None);
    assert!(!d.join("b.txt").exists());
    assert_eq!(a.encoding(), Encoding::Utf8);
}

#[test]
fn the_keys_choose_toggle_the_bom_and_confirm_in_the_terminal() {
    let (mut a, _p, d) = app("A\n".as_bytes());
    let nuovo = d.join("b.txt");
    a.save_as_choose(nuovo.clone());
    // utf-16le e' la voce dopo utf-8: Giu', Tab per il BOM, Invio
    tasto(&mut a, KeyCode::Down);
    tasto(&mut a, KeyCode::Tab);
    tasto(&mut a, KeyCode::Enter);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(
        std::fs::read(&nuovo).unwrap(),
        [0xFF, 0xFE, 0x41, 0x00, 0x0A, 0x00]
    );
}

#[test]
fn unsaved_edits_are_part_of_the_converted_file() {
    let (mut a, _p, d) = app(b"abc\n");
    a.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT));
    tasto(&mut a, KeyCode::End);
    a.handle_key(KeyEvent::new(KeyCode::Char('é'), KeyModifiers::NONE));
    assert!(a.is_modified());
    let nuovo = d.join("b.txt");
    a.save_as_choose(nuovo.clone());
    a.dialog_select(indice("windows-1252"));
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(
        std::fs::read(&nuovo).unwrap(),
        [0x61, 0x62, 0x63, 0xE9, 0x0A]
    );
    assert!(!a.is_modified());
}

#[test]
fn the_prompt_flow_asks_the_encoding_after_the_name() {
    let (mut a, _p, d) = app(b"abc\n");
    a.handle_key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ));
    assert_eq!(a.dialog_kind(), Some(DialogKind::SaveAs));
    a.dialog_set_text(d.join("b.txt").to_str().unwrap());
    tasto(&mut a, KeyCode::Enter);
    assert_eq!(
        a.dialog_kind(),
        Some(DialogKind::SaveEncoding),
        "dopo il nome, la codifica"
    );
    tasto(&mut a, KeyCode::Enter);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(std::fs::read(d.join("b.txt")).unwrap(), b"abc\n");
}
