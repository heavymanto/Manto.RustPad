//! I tre modi d'uso nel terminale, a mutua esclusione: VIEW (sola lettura), EDIT (modifica), TAIL (sola lettura che segue il
//! file). Si scelgono dal menu (Alt+M, o un clic sul modo nella riga di stato), con Alt+V (VIEW), Alt+E (EDIT) e Alt+T (TAIL).
//! Scritti da ARCH.

use mrp_tui::app::{App, AppConfig, DialogKind, Modo};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn app() -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_modi_{}_{}.log", std::process::id(), n));
    let testo: String = (0..30).map(|i| format!("linea {i:02}\n")).collect();
    std::fs::write(&p, testo).unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(80, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let _ = render_lines(&mut a, 80, 10);
    (a, p)
}

fn alt(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

fn tasto(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn schermo(a: &mut App) -> Vec<String> {
    render_lines(a, 80, 10)
}

#[test]
fn alt_v_goes_to_view_from_every_mode() {
    let (mut a, p) = app();
    assert_eq!(a.work_mode(), Modo::View);
    a.handle_key(tasto(KeyCode::F(4)));
    assert_eq!(a.work_mode(), Modo::Edit);
    a.handle_key(alt('v'));
    assert_eq!(a.work_mode(), Modo::View);
    a.handle_key(alt('t'));
    assert_eq!(a.work_mode(), Modo::Tail);
    a.handle_key(alt('v'));
    assert_eq!(a.work_mode(), Modo::View);
    assert!(!a.follow());
    // da VIEW resta VIEW
    a.handle_key(alt('v'));
    assert_eq!(a.work_mode(), Modo::View);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_mode_word_in_the_status_row_follows_the_mode() {
    let (mut a, p) = app();
    let riga = |a: &mut App| schermo(a)[8].trim_end().to_string();
    assert!(riga(&mut a).ends_with("VIEW"), "{}", riga(&mut a));
    a.handle_key(tasto(KeyCode::F(4)));
    assert!(riga(&mut a).ends_with("EDIT"), "{}", riga(&mut a));
    a.handle_key(alt('t'));
    // il messaggio del passaggio occupa il posto a sinistra, il modo resta a destra
    assert!(riga(&mut a).ends_with("TAIL"), "{}", riga(&mut a));
    a.handle_key(alt('v'));
    assert!(riga(&mut a).ends_with("VIEW"), "{}", riga(&mut a));
    let _ = std::fs::remove_file(p);
}

#[test]
fn alt_m_opens_the_menu_with_the_current_mode_highlighted_and_each_choice_works() {
    let (mut a, p) = app();
    a.handle_key(alt('m'));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Mode));
    assert_eq!(a.menu_index(), Some(0), "parte da VIEW");
    let s = schermo(&mut a).join("\n");
    assert!(
        s.contains("MODE") && s.contains("VIEW") && s.contains("EDIT") && s.contains("TAIL"),
        "{s}"
    );
    assert!(s.contains("read-only, follows the file as it grows"), "{s}");
    // Giu, Giu, Invio: TAIL
    a.handle_key(tasto(KeyCode::Down));
    a.handle_key(tasto(KeyCode::Down));
    a.handle_key(tasto(KeyCode::Down)); // fermo all'ultima voce
    a.handle_key(tasto(KeyCode::Enter));
    assert_eq!(a.work_mode(), Modo::Tail);
    assert_eq!(a.dialog_kind(), None);
    // il menu riparte dalla scelta di adesso; Su, Invio: EDIT
    a.handle_key(alt('m'));
    assert_eq!(a.menu_index(), Some(2));
    a.handle_key(tasto(KeyCode::Up));
    a.handle_key(tasto(KeyCode::Enter));
    assert_eq!(a.work_mode(), Modo::Edit);
    // Esc chiude senza cambiare
    a.handle_key(alt('m'));
    assert_eq!(a.menu_index(), Some(1));
    a.handle_key(tasto(KeyCode::Up));
    a.handle_key(tasto(KeyCode::Esc));
    assert_eq!(a.work_mode(), Modo::Edit);
    // Su, Su, Invio: VIEW
    a.handle_key(alt('m'));
    a.handle_key(tasto(KeyCode::Up));
    a.handle_key(tasto(KeyCode::Up));
    a.handle_key(tasto(KeyCode::Enter));
    assert_eq!(a.work_mode(), Modo::View);
    let _ = std::fs::remove_file(p);
}

#[test]
fn choosing_the_tail_from_editing_with_unsaved_changes_stays_in_edit_and_says_why() {
    let (mut a, p) = app();
    a.handle_key(tasto(KeyCode::F(4)));
    a.handle_key(tasto(KeyCode::Char('x')));
    a.handle_key(alt('m'));
    a.handle_key(tasto(KeyCode::Down));
    a.handle_key(tasto(KeyCode::Enter));
    assert_eq!(a.work_mode(), Modo::Edit, "le modifiche non si perdono");
    let s = schermo(&mut a).join("\n");
    assert!(s.contains("save or discard the changes"), "{s}");
    let _ = std::fs::remove_file(p);
}

fn clic(a: &mut App, colonna: u16, riga: u16) {
    a.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: colonna,
        row: riga,
        modifiers: KeyModifiers::NONE,
    });
}

#[test]
fn a_click_on_the_mode_in_the_status_row_opens_the_menu_and_elsewhere_does_not() {
    let (mut a, p) = app();
    // la riga di stato e' la penultima (h - 2 = 8); "VIEW" occupa le 4 colonne prima dell'ultima
    let riga = schermo(&mut a)[8].clone();
    let colonna = u16::try_from(riga.trim_end().chars().count()).unwrap() - 2;
    clic(&mut a, colonna, 8);
    assert_eq!(a.dialog_kind(), Some(DialogKind::Mode), "{riga}");
    a.handle_key(tasto(KeyCode::Esc));
    // un clic sul testo sposta il cursore e non apre niente; uno a sinistra della riga di stato neppure
    clic(&mut a, 10, 3);
    assert_eq!(a.dialog_kind(), None);
    clic(&mut a, 2, 8);
    assert_eq!(a.dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_column_mode_word_follows_the_mode_word() {
    let (mut a, p) = app();
    a.handle_key(tasto(KeyCode::F(8)));
    assert_eq!(a.mode_text(), "VIEW COL");
    a.handle_key(alt('t'));
    assert_eq!(a.mode_text(), "TAIL COL");
    let _ = std::fs::remove_file(p);
}

#[test]
fn alt_e_and_alt_t_switch_their_mode_on_and_off_and_f4_is_the_same_as_alt_e() {
    let (mut a, p) = app();
    a.handle_key(alt('e'));
    assert_eq!(a.work_mode(), Modo::Edit);
    // Alt+E di nuovo: si torna in lettura
    a.handle_key(alt('e'));
    assert_eq!(a.work_mode(), Modo::View);
    // da TAIL, Alt+E passa alla modifica
    a.handle_key(alt('t'));
    assert_eq!(a.work_mode(), Modo::Tail);
    a.handle_key(alt('e'));
    assert_eq!(a.work_mode(), Modo::Edit);
    // da EDIT senza modifiche, Alt+T passa a TAIL; di nuovo, si torna in lettura
    a.handle_key(alt('t'));
    assert_eq!(a.work_mode(), Modo::Tail);
    a.handle_key(alt('t'));
    assert_eq!(a.work_mode(), Modo::View);
    // F4 resta com'era
    a.handle_key(tasto(KeyCode::F(4)));
    assert_eq!(a.work_mode(), Modo::Edit);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_footer_shows_the_three_modes_in_every_mode() {
    let (mut a, p) = app();
    let piede = |a: &mut App| schermo(a)[9].trim_end().to_string();
    for passo in 0..3 {
        let riga = piede(&mut a);
        assert!(
            riga.starts_with(" Alt+V View")
                && riga.contains("Alt+E Edit")
                && riga.contains("Alt+T Tail"),
            "{riga:?}"
        );
        match passo {
            0 => a.handle_key(alt('e')),
            1 => a.handle_key(alt('t')),
            _ => {}
        }
    }
    let _ = std::fs::remove_file(p);
}
