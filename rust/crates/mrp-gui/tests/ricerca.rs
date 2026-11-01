//! Le finestre di Cerca e Sostituisci (B4c): campi di testo, scelta del modo, maiuscole, ambito, anteprima e applica, con
//! widget egui; guidano l'`App` con le azioni `dialog_*` (B4a-2). Scritti da ARCH: gli attesi sono calcolati a mano. Il file di
//! prova di Cerca e' "foo Bar baz bar\n": "Bar" al byte 4, "bar" al 12; quello di Sostituisci "a a a\n".

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_core::pattern::Mode;
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::dialogs::DialogView;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_ricerca_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    p
}

fn cfg() -> AppConfig {
    AppConfig {
        chrome: false,
        ..AppConfig::default()
    }
}

fn finestra(dati: &[u8]) -> (Finestra, Context, PathBuf) {
    let p = file(dati);
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let f = Finestra::da_app(app, Box::new(cfg));
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

fn disegna(f: &mut Finestra, ctx: &Context) {
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
}

fn premi(f: &mut Finestra, ctx: &Context, key: Key) {
    giro(f, ctx, vec![tasto(key, Modifiers::NONE)]);
    giro(f, ctx, vec![]);
}

fn scrivi(f: &mut Finestra, ctx: &Context, testo: &str) {
    giro(f, ctx, vec![Event::Text(testo.into())]);
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

fn opzione(f: &Finestra, testo: &str) -> Pos2 {
    f.dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == testo)
        .unwrap_or_else(|| panic!("manca l'opzione {testo:?} in {:?}", f.dialogo_opzioni()))
        .0
        .center()
}

fn etichette(f: &Finestra) -> Vec<String> {
    f.dialogo_pulsanti()
        .iter()
        .map(|(_, t)| t.clone())
        .collect()
}

fn opzioni(f: &Finestra) -> Vec<String> {
    f.dialogo_opzioni().iter().map(|(_, t)| t.clone()).collect()
}

/// Aspetta che l'App finisca i lavori in background (la ricerca) e fa un giro.
fn aspetta(f: &mut Finestra, ctx: &Context) {
    assert!(f.workspace().wait_idle(Duration::from_secs(20)));
    giro(f, ctx, vec![]);
}

// ---- Cerca ----

#[test]
fn find_is_a_window_with_a_field_the_options_and_two_buttons() {
    let (mut f, ctx, p) = finestra(b"foo Bar baz bar\n");
    giro(&mut f, &ctx, vec![tasto(Key::F, ctrl())]);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("FIND"));
    assert_eq!(f.dialogo_campi().len(), 1);
    assert_eq!(opzioni(&f), vec!["text", "glob", "regex", "match case"]);
    assert_eq!(etichette(&f), vec!["Find", "Cancel"]);
    let _ = std::fs::remove_file(p);
}

#[test]
fn typing_and_enter_search_forward() {
    let (mut f, ctx, p) = finestra(b"foo Bar baz bar\n");
    giro(&mut f, &ctx, vec![tasto(Key::F, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "bar");
    match f.app().dialog_view().unwrap() {
        DialogView::Find { text, .. } => assert_eq!(text, "bar"),
        altro => panic!("{altro:?}"),
    }
    premi(&mut f, &ctx, Key::Enter);
    aspetta(&mut f, &ctx);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(
        f.app().found(),
        Some((12, 15)),
        "maiuscole che contano: solo \"bar\""
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_options_of_find_are_clickable() {
    let (mut f, ctx, p) = finestra(b"foo Bar baz bar\n");
    giro(&mut f, &ctx, vec![tasto(Key::F, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "bar");
    let pos = opzione(&f, "match case");
    clic(&mut f, &ctx, pos);
    let pos = opzione(&f, "glob");
    clic(&mut f, &ctx, pos);
    match f.app().dialog_view().unwrap() {
        DialogView::Find {
            mode, case, text, ..
        } => {
            assert_eq!((mode, case, text.as_str()), (Mode::Glob, false, "bar"));
        }
        altro => panic!("{altro:?}"),
    }
    let pos = bottone(&f, "Find");
    clic(&mut f, &ctx, pos);
    aspetta(&mut f, &ctx);
    assert_eq!(
        f.app().found(),
        Some((4, 7)),
        "senza maiuscole il primo e' \"Bar\""
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn cancel_and_escape_close_find_without_searching() {
    let (mut f, ctx, p) = finestra(b"foo Bar baz bar\n");
    giro(&mut f, &ctx, vec![tasto(Key::F, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "bar");
    let pos = bottone(&f, "Cancel");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().dialog_kind(), None);
    assert!(f.app().found().is_none());
    giro(&mut f, &ctx, vec![tasto(Key::F, ctrl())]);
    disegna(&mut f, &ctx);
    premi(&mut f, &ctx, Key::Escape);
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

// ---- Sostituisci ----

#[test]
fn replace_is_a_window_with_two_fields_and_the_scope() {
    let (mut f, ctx, p) = finestra(b"a a a\n");
    giro(&mut f, &ctx, vec![tasto(Key::H, ctrl())]);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("REPLACE"));
    assert_eq!(f.dialogo_campi().len(), 2);
    assert_eq!(
        opzioni(&f),
        vec![
            "text",
            "glob",
            "regex",
            "match case",
            "this tab",
            "all tabs"
        ]
    );
    assert_eq!(etichette(&f), vec!["Preview", "Cancel"]);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_first_field_has_the_focus_and_tab_moves_to_the_second() {
    let (mut f, ctx, p) = finestra(b"a a a\n");
    giro(&mut f, &ctx, vec![tasto(Key::H, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "a");
    premi(&mut f, &ctx, Key::Tab);
    scrivi(&mut f, &ctx, "b");
    match f.app().dialog_view().unwrap() {
        DialogView::Replace {
            find, with, field, ..
        } => {
            assert_eq!((find.as_str(), with.as_str(), field), ("a", "b", 1));
        }
        altro => panic!("{altro:?}"),
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn preview_then_replace_all_with_the_buttons() {
    let (mut f, ctx, p) = finestra(b"a a a\n");
    giro(&mut f, &ctx, vec![tasto(Key::H, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "a");
    premi(&mut f, &ctx, Key::Tab);
    scrivi(&mut f, &ctx, "b");
    let pos = bottone(&f, "Preview");
    clic(&mut f, &ctx, pos);
    aspetta(&mut f, &ctx);
    disegna(&mut f, &ctx);
    match f.app().dialog_view().unwrap() {
        DialogView::Replace { status, ready, .. } => {
            assert_eq!(
                status,
                "3 matches in 1 tab(s): Enter replaces all, Esc cancels"
            );
            assert!(ready);
        }
        altro => panic!("{altro:?}"),
    }
    assert_eq!(
        f.app().document_text(),
        b"a a a\n",
        "l'anteprima non cambia niente"
    );
    assert_eq!(
        etichette(&f),
        vec!["Replace all", "Cancel"],
        "con l'anteprima pronta il pulsante applica"
    );
    let pos = bottone(&f, "Replace all");
    clic(&mut f, &ctx, pos);
    aspetta(&mut f, &ctx);
    assert_eq!(f.app().document_text(), b"b b b\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn changing_an_option_takes_the_preview_away_and_the_button_goes_back() {
    let (mut f, ctx, p) = finestra(b"a a a\n");
    giro(&mut f, &ctx, vec![tasto(Key::H, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "a");
    let pos = bottone(&f, "Preview");
    clic(&mut f, &ctx, pos);
    aspetta(&mut f, &ctx);
    disegna(&mut f, &ctx);
    assert_eq!(etichette(&f)[0], "Replace all");
    let pos = opzione(&f, "all tabs");
    clic(&mut f, &ctx, pos);
    disegna(&mut f, &ctx);
    match f.app().dialog_view().unwrap() {
        DialogView::Replace {
            all_tabs, ready, ..
        } => assert!(all_tabs && !ready),
        altro => panic!("{altro:?}"),
    }
    assert_eq!(etichette(&f)[0], "Preview");
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_empty_find_field_shows_the_message() {
    let (mut f, ctx, p) = finestra(b"a a a\n");
    giro(&mut f, &ctx, vec![tasto(Key::H, ctrl())]);
    disegna(&mut f, &ctx);
    let pos = bottone(&f, "Preview");
    clic(&mut f, &ctx, pos);
    disegna(&mut f, &ctx);
    match f.app().dialog_view().unwrap() {
        DialogView::Replace { status, .. } => assert_eq!(status, "type what to find"),
        altro => panic!("{altro:?}"),
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn enter_in_a_field_is_the_default_button_and_escape_closes() {
    let (mut f, ctx, p) = finestra(b"a a a\n");
    giro(&mut f, &ctx, vec![tasto(Key::H, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "a");
    premi(&mut f, &ctx, Key::Enter);
    aspetta(&mut f, &ctx);
    disegna(&mut f, &ctx);
    match f.app().dialog_view().unwrap() {
        DialogView::Replace { ready, .. } => assert!(ready, "Invio fa l'anteprima"),
        altro => panic!("{altro:?}"),
    }
    premi(&mut f, &ctx, Key::Escape);
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_scope_radio_goes_both_ways() {
    let (mut f, ctx, p) = finestra(b"a a a\n");
    giro(&mut f, &ctx, vec![tasto(Key::H, ctrl())]);
    disegna(&mut f, &ctx);
    let scope = |f: &mut Finestra| match f.app().dialog_view().unwrap() {
        DialogView::Replace { all_tabs, .. } => all_tabs,
        altro => panic!("{altro:?}"),
    };
    assert!(!scope(&mut f));
    let pos = opzione(&f, "all tabs");
    clic(&mut f, &ctx, pos);
    assert!(scope(&mut f));
    disegna(&mut f, &ctx);
    let pos = opzione(&f, "this tab");
    clic(&mut f, &ctx, pos);
    assert!(!scope(&mut f), "tornare a questa scheda");
    let _ = std::fs::remove_file(p);
}

// ---- il filtro (Alt+G) ----

#[test]
fn the_filter_is_the_find_window_with_one_more_option_and_the_invert_box_is_clickable() {
    let (mut f, ctx, p) = finestra(b"uno ERROR\ndue\ntre ERROR\n");
    giro(&mut f, &ctx, vec![tasto(Key::G, Modifiers::ALT)]);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("FILTER LINES"));
    assert_eq!(
        opzioni(&f),
        vec![
            "text",
            "glob",
            "regex",
            "match case",
            "Hide the lines that match instead"
        ]
    );
    assert_eq!(etichette(&f), vec!["Filter", "Cancel"]);
    assert!(!f.app().find_invert());
    // il testo si scrive prima: il clic sulla casella toglie il fuoco al campo
    scrivi(&mut f, &ctx, "ERROR");
    let pos = opzione(&f, "Hide the lines that match instead");
    clic(&mut f, &ctx, pos);
    assert!(f.app().find_invert());
    // il pulsante filtra: il lavoro parte e finisce in una scheda nuova con le righe che NON corrispondono
    let pos = bottone(&f, "Filter");
    clic(&mut f, &ctx, pos);
    aspetta(&mut f, &ctx);
    aspetta(&mut f, &ctx);
    assert_eq!(f.workspace().len(), 2);
    assert_eq!(f.app().document_text(), b"due\n");
    let _ = std::fs::remove_file(p);
}
