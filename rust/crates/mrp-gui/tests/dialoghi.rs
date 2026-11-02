//! I dialoghi della finestra (B4b): aiuto, menu di tema, lingua e codifica, "vai a" e conferma di uscita si disegnano con finestre
//! egui (campi, liste, pulsanti) quando l'`App` e' senza la cornice del terminale; la finestra guida l'`App` con le azioni
//! `dialog_*` (B4a-1). Scritti da ARCH: gli attesi sono calcolati a mano. Un contesto di egui senza schermo, un `giro` alla
//! volta con eventi finti, come negli altri test della GUI.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::dialogs::DialogView;
use mrp_tui::theme::ThemeId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!(
        "mrp_dialoghi_test_{}_{}.txt",
        std::process::id(),
        n
    ));
    std::fs::write(&p, dati).unwrap();
    p
}

fn cfg(chrome: bool) -> AppConfig {
    AppConfig {
        chrome,
        ..AppConfig::default()
    }
}

fn finestra(dati: &[u8], chrome: bool) -> (Finestra, Context, PathBuf) {
    let p = file(dati);
    let mut app = App::open(&p, cfg(chrome)).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let f = Finestra::da_app(app, Box::new(move || cfg(chrome)));
    (f, Context::default(), p)
}

fn giro(f: &mut Finestra, ctx: &Context, eventi: Vec<Event>) -> FullOutput {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))),
        events: eventi,
        ..Default::default()
    };
    ctx.run(raw, |ctx| f.giro(ctx))
}

fn tasto(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
}

fn pigia(pos: Pos2, premuto: bool) -> Event {
    Event::PointerButton {
        pos,
        button: eframe::egui::PointerButton::Primary,
        pressed: premuto,
        modifiers: Modifiers::NONE,
    }
}

fn clic(f: &mut Finestra, ctx: &Context, pos: Pos2) {
    giro(f, ctx, vec![Event::PointerMoved(pos), pigia(pos, true)]);
    giro(f, ctx, vec![pigia(pos, false)]);
    giro(f, ctx, vec![]);
}

/// Due giri: il primo apre la finestra, il secondo la mette a posto (il campo prende il fuoco).
fn disegna(f: &mut Finestra, ctx: &Context) {
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
}

fn premi(f: &mut Finestra, ctx: &Context, key: Key) {
    giro(f, ctx, vec![tasto(key, Modifiers::NONE)]);
    giro(f, ctx, vec![]);
}

fn bottone(f: &Finestra, testo: &str) -> Pos2 {
    f.dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == testo)
        .unwrap_or_else(|| panic!("manca il pulsante {testo:?} in {:?}", f.dialogo_pulsanti()))
        .0
        .center()
}

fn etichette(f: &Finestra) -> Vec<String> {
    f.dialogo_pulsanti()
        .iter()
        .map(|(_, t)| t.clone())
        .collect()
}

#[test]
fn a_dialog_with_a_view_gets_a_window_only_without_the_terminal_chrome() {
    let (mut f, ctx, p) = finestra(b"uno\n", true);
    premi(&mut f, &ctx, Key::F1);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Help));
    assert!(
        f.dialogo_rect().is_none() && f.dialogo_titolo().is_none(),
        "con la cornice lo disegna l'App a celle"
    );
    let (mut g, ctx2, q) = finestra(b"uno\n", false);
    assert!(
        g.dialogo_rect().is_none(),
        "nessun dialogo, nessuna finestra"
    );
    premi(&mut g, &ctx2, Key::F1);
    disegna(&mut g, &ctx2);
    assert_eq!(g.dialogo_titolo(), Some("KEYS"));
    let r = g.dialogo_rect().unwrap();
    // al centro della finestra del programma
    assert!(
        (r.center().x - 500.0).abs() < 60.0 && r.width() > 200.0 && r.height() > 100.0,
        "{r:?}"
    );
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn help_has_one_button_and_it_closes_the_window() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F1);
    disegna(&mut f, &ctx);
    assert_eq!(etichette(&f), vec!["OK"]);
    let pos = bottone(&f, "OK");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().dialog_kind(), None);
    disegna(&mut f, &ctx);
    assert!(f.dialogo_rect().is_none());
    // Esc fa lo stesso
    premi(&mut f, &ctx, Key::F1);
    disegna(&mut f, &ctx);
    assert!(f.dialogo_rect().is_some());
    premi(&mut f, &ctx, Key::Escape);
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn keys_and_text_go_to_the_window_not_to_the_document_while_a_dialog_is_open() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F12);
    premi(&mut f, &ctx, Key::F1);
    disegna(&mut f, &ctx);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    giro(&mut f, &ctx, vec![]);
    assert_eq!(
        f.app().document_text(),
        b"uno\n",
        "col dialogo aperto il testo non entra nel documento"
    );
    premi(&mut f, &ctx, Key::Escape);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    assert_eq!(
        f.app().document_text(),
        b"xuno\n",
        "chiuso il dialogo si scrive di nuovo"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_on_the_window_does_not_reach_the_text_underneath() {
    let (mut f, ctx, p) = finestra(b"abc\ndef\nghi\n", false);
    premi(&mut f, &ctx, Key::F1);
    disegna(&mut f, &ctx);
    let pos = bottone(&f, "OK");
    let (o, c, colonne, righe) = f.geometria().unwrap();
    assert!(
        mrp_gui::input::cella_sotto(pos, o, c, colonne, righe).is_some(),
        "il pulsante sta sopra la griglia: senza finestra sarebbe un clic sul testo"
    );
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().cursor(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_theme_menu_lists_four_rows_a_click_selects_and_ok_applies() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F6);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("THEME"));
    assert_eq!(
        f.dialogo_voci().len(),
        4,
        "nella finestra: Blu, Debian, Scuro e Chiaro"
    );
    // sotto la lista, OK e Annulla: la scelta si conferma, non scatta da sola
    let testi: Vec<String> = f
        .dialogo_pulsanti()
        .iter()
        .map(|(_, t)| t.clone())
        .collect();
    assert_eq!(testi, vec!["OK", "Cancel"]);
    let pos = f.dialogo_voci()[3].center();
    clic(&mut f, &ctx, pos);
    assert_eq!(
        f.app().theme_id(),
        ThemeId::Console,
        "un clic sulla voce non applica"
    );
    assert_eq!(f.app().menu_index(), Some(3));
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Theme));
    let ok = f.dialogo_pulsanti()[0].0.center();
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().theme_id(), ThemeId::Light);
    assert_eq!(f.app().dialog_kind(), None);
    // Annulla lascia tutto com'e'
    premi(&mut f, &ctx, Key::F6);
    disegna(&mut f, &ctx);
    let pos = f.dialogo_voci()[0].center();
    clic(&mut f, &ctx, pos);
    let annulla = f.dialogo_pulsanti()[1].0.center();
    clic(&mut f, &ctx, annulla);
    assert_eq!(f.app().theme_id(), ThemeId::Light);
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_menu_answers_to_the_arrow_keys_and_enter() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F6);
    disegna(&mut f, &ctx);
    let selezionata = |f: &mut Finestra| match f.app().dialog_view().unwrap() {
        DialogView::Menu { selected, .. } => selected,
        altro => panic!("{altro:?}"),
    };
    assert_eq!(
        selezionata(&mut f),
        0,
        "il tema corrente (Console) non c'e': parte dal primo"
    );
    premi(&mut f, &ctx, Key::ArrowDown);
    assert_eq!(selezionata(&mut f), 1);
    for _ in 0..6 {
        premi(&mut f, &ctx, Key::ArrowDown);
    }
    assert_eq!(
        selezionata(&mut f),
        3,
        "ferma all'ultima voce (quattro temi)"
    );
    for _ in 0..6 {
        premi(&mut f, &ctx, Key::ArrowUp);
    }
    assert_eq!(selezionata(&mut f), 0);
    assert_eq!(f.app().theme_id(), ThemeId::Console, "scorrere non applica");
    premi(&mut f, &ctx, Key::ArrowDown);
    premi(&mut f, &ctx, Key::Enter);
    assert_eq!(f.app().theme_id(), ThemeId::Debian);
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_encoding_menu_is_a_scrolling_list_and_escape_closes_it() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F9);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("ENCODING"));
    let n = f.dialogo_voci().len();
    assert!(n >= 8, "almeno qualche voce visibile: {n}");
    let r = f.dialogo_rect().unwrap();
    assert!(r.height() <= 700.0, "la finestra sta nello schermo: {r:?}");
    premi(&mut f, &ctx, Key::Escape);
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn goto_has_a_text_field_with_focus_and_runs_on_enter() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\ntre\nquattro\n", false);
    giro(&mut f, &ctx, vec![tasto(Key::G, ctrl())]);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("GO TO"));
    assert!(f.dialogo_campo().is_some());
    assert_eq!(etichette(&f), vec!["OK", "Cancel"]);
    giro(&mut f, &ctx, vec![Event::Text("3".into())]);
    giro(&mut f, &ctx, vec![]);
    match f.app().dialog_view().unwrap() {
        DialogView::Prompt { text, .. } => assert_eq!(text, "3"),
        altro => panic!("{altro:?}"),
    }
    premi(&mut f, &ctx, Key::Enter);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.app().cursor(), 8, "riga 3 comincia al byte 8");
    let _ = std::fs::remove_file(p);
}

#[test]
fn goto_buttons_ok_runs_and_cancel_does_not() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\ntre\nquattro\n", false);
    giro(&mut f, &ctx, vec![tasto(Key::G, ctrl())]);
    disegna(&mut f, &ctx);
    giro(&mut f, &ctx, vec![Event::Text("4".into())]);
    giro(&mut f, &ctx, vec![]);
    let pos = bottone(&f, "Cancel");
    clic(&mut f, &ctx, pos);
    assert_eq!((f.app().dialog_kind(), f.app().cursor()), (None, 0));
    giro(&mut f, &ctx, vec![tasto(Key::G, ctrl())]);
    disegna(&mut f, &ctx);
    giro(&mut f, &ctx, vec![Event::Text("4".into())]);
    giro(&mut f, &ctx, vec![]);
    let pos = bottone(&f, "OK");
    clic(&mut f, &ctx, pos);
    assert_eq!(
        f.app().cursor(),
        12,
        "riga 4: \"uno\\ndue\\ntre\\n\" sono 12 byte"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_exit_confirmation_has_three_buttons() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F12);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    f.workspace().active_mut().request_close();
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("UNSAVED CHANGES"));
    assert_eq!(
        etichette(&f),
        vec!["Save and quit", "Discard changes", "Cancel"]
    );
    let pos = bottone(&f, "Cancel");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().dialog_kind(), None);
    assert!(!f.workspace().should_quit());
    f.workspace().active_mut().request_close();
    disegna(&mut f, &ctx);
    let pos = bottone(&f, "Discard changes");
    clic(&mut f, &ctx, pos);
    assert!(f.workspace().should_quit());
    assert_eq!(std::fs::read(&p).unwrap(), b"uno\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn saving_and_quitting_from_the_confirmation_writes_the_file() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F12);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    f.workspace().active_mut().request_close();
    disegna(&mut f, &ctx);
    let pos = bottone(&f, "Save and quit");
    clic(&mut f, &ctx, pos);
    assert!(f.app().wait_idle(Duration::from_secs(20)));
    giro(&mut f, &ctx, vec![]);
    assert!(f.workspace().should_quit());
    assert_eq!(std::fs::read(&p).unwrap(), b"xuno\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_dialog_texts_follow_the_language() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang");
    let (lingue, _) = mrp_core::lang::registry_in(Some(dir.as_path()));
    let p = file(b"uno\n");
    let c = AppConfig {
        chrome: false,
        lang_code: Some("it".to_string()),
        langs: lingue,
        ..AppConfig::default()
    };
    let mut app = App::open(&p, c.clone()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(move || c.clone()));
    let ctx = Context::default();
    giro(&mut f, &ctx, vec![tasto(Key::G, ctrl())]);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("VAI A"));
    assert_eq!(etichette(&f), vec!["OK", "Annulla"]);
    let _ = std::fs::remove_file(p);
}
