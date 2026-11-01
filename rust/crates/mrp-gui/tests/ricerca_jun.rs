//! I test del JUN per le finestre di Cerca e Sostituisci (B4c): i casi che il file di ARCH (`ricerca.rs`) non copre,
//! con gli attesi calcolati a mano dal PLAN. Come li': un contesto di egui senza schermo, un giro alla volta con
//! eventi finti. Il file di prova di Cerca e' "foo Bar baz bar\n" ("Bar" al byte 4, "bar" al 12); quello di
//! Sostituisci "a a a\n".

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
    let p = std::env::temp_dir().join(format!(
        "mrp_ricerca_jun_test_{}_{}.txt",
        std::process::id(),
        n
    ));
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

fn opzione(f: &Finestra, testo: &str) -> Pos2 {
    f.dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == testo)
        .unwrap_or_else(|| panic!("manca l'opzione {testo:?} in {:?}", f.dialogo_opzioni()))
        .0
        .center()
}

/// Aspetta che l'App finisca i lavori in background (la ricerca) e fa un giro.
fn aspetta(f: &mut Finestra, ctx: &Context) {
    assert!(f.workspace().wait_idle(Duration::from_secs(20)));
    giro(f, ctx, vec![]);
}

/// La terza scelta del modo (regex) si clicca come le altre: la vista dice Regex.
#[test]
fn the_regex_choice_of_find_sets_the_mode() {
    let (mut f, ctx, p) = finestra(b"foo Bar baz bar\n");
    giro(&mut f, &ctx, vec![tasto(Key::F, ctrl())]);
    disegna(&mut f, &ctx);
    let pos = opzione(&f, "regex");
    clic(&mut f, &ctx, pos);
    match f.app().dialog_view().unwrap() {
        DialogView::Find { mode, .. } => assert_eq!(mode, Mode::Regex),
        altro => panic!("{altro:?}"),
    }
    let _ = std::fs::remove_file(p);
}

/// Un clic sul secondo campo di Sostituisci porta li' il fuoco (non solo Tab): quello che si scrive dopo va a "con
/// cosa sostituire" e l'App lo sa (`field` = 1). Il campo del prompt resta il primo dei due.
#[test]
fn a_click_on_the_second_field_of_replace_takes_the_focus_there() {
    let (mut f, ctx, p) = finestra(b"a a a\n");
    giro(&mut f, &ctx, vec![tasto(Key::H, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "a");
    let secondo = f.dialogo_campi()[1];
    assert_eq!(
        f.dialogo_campo(),
        Some(f.dialogo_campi()[0]),
        "il campo del prompt e' il primo"
    );
    clic(&mut f, &ctx, secondo.center());
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

/// Riaperto, Cerca riparte dall'ultimo testo cercato: quello che si scrive dopo si aggiunge in coda. Il file e'
/// "foo Bar baz bar\n": con le maiuscole che contano "bar" sta al byte 12 (il "Bar" al 4 non vale).
#[test]
fn find_reopens_with_the_last_searched_text() {
    let (mut f, ctx, p) = finestra(b"foo Bar baz bar\n");
    giro(&mut f, &ctx, vec![tasto(Key::F, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "bar");
    premi(&mut f, &ctx, Key::Enter);
    aspetta(&mut f, &ctx);
    assert_eq!(f.app().found(), Some((12, 15)));
    giro(&mut f, &ctx, vec![tasto(Key::F, ctrl())]);
    disegna(&mut f, &ctx);
    scrivi(&mut f, &ctx, "x");
    match f.app().dialog_view().unwrap() {
        DialogView::Find { text, .. } => assert_eq!(text, "barx"),
        altro => panic!("{altro:?}"),
    }
    let _ = std::fs::remove_file(p);
}
