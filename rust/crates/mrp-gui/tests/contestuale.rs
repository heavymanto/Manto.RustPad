//! Il menu contestuale della griglia (B5): il tasto destro sul testo apre un piccolo menu con i comandi di modifica; si sceglie
//! una voce, oppure si chiude con Esc o con un clic fuori. Scritti da ARCH: gli attesi sono calcolati a mano. Un contesto di
//! egui senza schermo, un `giro` alla volta con eventi finti, come negli altri test della GUI.

use eframe::egui::{
    Context, Event, FullOutput, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2,
};
use mrp_gui::gui::Finestra;
use mrp_gui::menu::CONTESTUALE;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::Cmd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!(
        "mrp_contestuale_test_{}_{}.txt",
        std::process::id(),
        n
    ));
    std::fs::write(&p, b"uno\ndue\ntre\n").unwrap();
    p
}

fn finestra() -> (Finestra, Context, PathBuf) {
    let cfg = || AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let p = file();
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    (f, ctx, p)
}

fn giro(f: &mut Finestra, ctx: &Context, eventi: Vec<Event>) -> FullOutput {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))),
        events: eventi,
        ..Default::default()
    };
    ctx.run(raw, |ctx| f.giro(ctx))
}

fn clic(f: &mut Finestra, ctx: &Context, pos: Pos2, tasto: PointerButton) {
    giro(
        f,
        ctx,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: tasto,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    giro(
        f,
        ctx,
        vec![Event::PointerButton {
            pos,
            button: tasto,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    giro(f, ctx, vec![]);
}

fn esc() -> Event {
    Event::Key {
        key: Key::Escape,
        physical_key: Some(Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// Un punto sul testo, lontano dai bordi.
fn sul_testo() -> Pos2 {
    Pos2::new(300.0, 300.0)
}

fn comandi(f: &Finestra) -> Vec<Cmd> {
    f.contestuale_voci().iter().map(|(_, c)| *c).collect()
}

#[test]
fn the_context_menu_lists_the_editing_commands_with_clean_separators() {
    use Cmd::*;
    assert_eq!(
        CONTESTUALE,
        [
            Some(Undo),
            Some(Redo),
            None,
            Some(Cut),
            Some(Copy),
            Some(Paste),
            Some(SelectAll),
            None,
            Some(Find),
        ]
    );
}

#[test]
fn no_menu_is_open_until_the_right_button_is_pressed() {
    let (f, _ctx, p) = finestra();
    assert!(f.contestuale_voci().is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_right_click_on_the_text_opens_the_menu_at_the_pointer() {
    let (mut f, ctx, p) = finestra();
    clic(&mut f, &ctx, sul_testo(), PointerButton::Secondary);
    use Cmd::*;
    assert_eq!(
        comandi(&f),
        vec![Undo, Redo, Cut, Copy, Paste, SelectAll, Find]
    );
    let voci = f.contestuale_voci();
    // la prima voce sta all'angolo del clic (a destra e sotto), e le altre una sotto l'altra senza sovrapporsi
    assert!(
        voci[0].0.min.x >= 300.0 && voci[0].0.min.x < 320.0,
        "{:?}",
        voci[0].0
    );
    assert!(
        voci[0].0.min.y >= 300.0 && voci[0].0.min.y < 320.0,
        "{:?}",
        voci[0].0
    );
    for w in voci.windows(2) {
        assert!(
            w[1].0.min.y >= w[0].0.max.y - 0.5,
            "{:?} {:?}",
            w[0].0,
            w[1].0
        );
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_left_button_does_not_open_the_menu() {
    let (mut f, ctx, p) = finestra();
    clic(&mut f, &ctx, sul_testo(), PointerButton::Primary);
    assert!(f.contestuale_voci().is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn choosing_an_entry_runs_the_command_and_closes_the_menu() {
    let (mut f, ctx, p) = finestra();
    assert!(!f.app().has_selection());
    clic(&mut f, &ctx, sul_testo(), PointerButton::Secondary);
    let i = comandi(&f)
        .iter()
        .position(|c| *c == Cmd::SelectAll)
        .unwrap();
    let pos = f.contestuale_voci()[i].0.center();
    clic(&mut f, &ctx, pos, PointerButton::Primary);
    assert!(f.app().has_selection(), "Seleziona tutto ha selezionato");
    assert!(f.contestuale_voci().is_empty(), "il menu si e' chiuso");
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_entry_that_makes_no_sense_now_does_nothing_and_leaves_the_menu_open() {
    let (mut f, ctx, p) = finestra();
    // senza selezione "Copia" e' spenta
    clic(&mut f, &ctx, sul_testo(), PointerButton::Secondary);
    let i = comandi(&f).iter().position(|c| *c == Cmd::Copy).unwrap();
    let pos = f.contestuale_voci()[i].0.center();
    clic(&mut f, &ctx, pos, PointerButton::Primary);
    assert!(!f.app().has_selection());
    assert!(!f.contestuale_voci().is_empty(), "il menu e' ancora aperto");
    let _ = std::fs::remove_file(p);
}

#[test]
fn escape_closes_the_menu_without_reaching_the_document() {
    let (mut f, ctx, p) = finestra();
    clic(&mut f, &ctx, sul_testo(), PointerButton::Secondary);
    assert!(!f.contestuale_voci().is_empty());
    let prima = f.app().is_edit();
    giro(&mut f, &ctx, vec![esc()]);
    giro(&mut f, &ctx, vec![]);
    assert!(f.contestuale_voci().is_empty());
    assert_eq!(f.app().is_edit(), prima);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_outside_closes_the_menu() {
    let (mut f, ctx, p) = finestra();
    clic(&mut f, &ctx, sul_testo(), PointerButton::Secondary);
    assert!(!f.contestuale_voci().is_empty());
    // lontano dal menu (che e' piccolo e sta in basso a destra del clic)
    clic(&mut f, &ctx, Pos2::new(40.0, 120.0), PointerButton::Primary);
    assert!(f.contestuale_voci().is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_second_right_click_moves_the_menu() {
    let (mut f, ctx, p) = finestra();
    clic(&mut f, &ctx, sul_testo(), PointerButton::Secondary);
    let prima = f.contestuale_voci()[0].0.min;
    clic(
        &mut f,
        &ctx,
        Pos2::new(500.0, 200.0),
        PointerButton::Secondary,
    );
    let dopo = f.contestuale_voci()[0].0.min;
    assert!(
        dopo.x >= 500.0 && dopo.y >= 200.0 && dopo != prima,
        "{dopo:?}"
    );
    assert!(dopo.x < 520.0 && dopo.y < 220.0, "{dopo:?}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_toolbar_and_the_menu_bar_have_no_context_menu() {
    let (mut f, ctx, p) = finestra();
    clic(
        &mut f,
        &ctx,
        Pos2::new(900.0, 5.0),
        PointerButton::Secondary,
    );
    assert!(f.contestuale_voci().is_empty(), "barra dei menu");
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_open_dialog_keeps_the_right_button_to_itself() {
    let (mut f, ctx, p) = finestra();
    giro(
        &mut f,
        &ctx,
        vec![Event::Key {
            key: Key::F6,
            physical_key: Some(Key::F6),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
    );
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    assert!(f.app().dialog_has_view(), "il menu dei temi e' aperto");
    clic(
        &mut f,
        &ctx,
        Pos2::new(30.0, 400.0),
        PointerButton::Secondary,
    );
    assert!(f.contestuale_voci().is_empty());
    let _ = std::fs::remove_file(p);
}
