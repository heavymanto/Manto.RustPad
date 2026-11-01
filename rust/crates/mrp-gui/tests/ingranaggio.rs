//! L'ingranaggio in fondo alla barra dei salti: un clic apre la finestra con le pagine dei tre salti; si scrivono, si
//! conferma, e la barra ne prende le etichette. Scritti da ARCH.

use eframe::egui::{self, Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cfg() -> AppConfig {
    AppConfig {
        chrome: false,
        nav_bar: true,
        ..AppConfig::default()
    }
}

fn finestra() -> (Finestra, Context, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_ingr_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, b"uno\ndue\n").unwrap();
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

fn clic(f: &mut Finestra, ctx: &Context, pos: Pos2) {
    for premuto in [true, false] {
        giro(
            f,
            ctx,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: premuto,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    giro(f, ctx, vec![]);
}

fn etichette(f: &Finestra) -> Vec<String> {
    f.pulsanti_salti()
        .iter()
        .map(|(_, s)| s.etichetta.clone())
        .collect()
}

#[test]
fn the_gear_sits_at_the_right_end_of_the_jump_bar() {
    let (f, _ctx, p) = finestra();
    let barra = f.salti_rect().unwrap();
    let g = f.ingranaggio_rect().expect("l'ingranaggio c'e'");
    assert!(barra.contains_rect(g), "{g:?} dentro {barra:?}");
    // a destra di tutti i pulsanti dei salti, attaccato al bordo
    let ultimo = f.pulsanti_salti().last().unwrap().0;
    assert!(g.min.x > ultimo.max.x, "{g:?} dopo {ultimo:?}");
    assert!(barra.max.x - g.max.x < 20.0, "{g:?} sul bordo di {barra:?}");
    // i dieci pulsanti dei salti sono sempre quelli di prima
    assert_eq!(f.pulsanti_salti().len(), 10);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_gear_opens_the_jump_window_and_ok_changes_the_buttons() {
    let (mut f, ctx, p) = finestra();
    assert_eq!(etichette(&f)[3], "-10");
    let g = f.ingranaggio_rect().unwrap().center();
    clic(&mut f, &ctx, g);
    giro(&mut f, &ctx, vec![]);
    assert_eq!(f.dialogo_titolo(), Some("JUMPS"));
    assert_eq!(f.dialogo_campi().len(), 3);
    // il primo campo ha il fuoco: si scrive il numero nuovo al posto del vecchio
    f.app().dialog_set_jump(0, "7");
    f.app().dialog_set_jump(1, "70");
    f.app().dialog_set_jump(2, "700");
    giro(&mut f, &ctx, vec![]);
    let ok = f
        .dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == "OK")
        .expect("OK")
        .0
        .center();
    clic(&mut f, &ctx, ok);
    giro(&mut f, &ctx, vec![]);
    assert_eq!(f.dialogo_titolo(), None);
    assert_eq!(
        etichette(&f)[1..9],
        ["-700", "-70", "-7", "-1", "+1", "+7", "+70", "+700"]
    );
    assert_eq!(f.workspace().jump_pages(), [7, 70, 700]);
    let _ = std::fs::remove_file(p);
}

#[test]
fn cancel_and_a_bad_number_leave_the_buttons_alone() {
    let (mut f, ctx, p) = finestra();
    let g = f.ingranaggio_rect().unwrap().center();
    clic(&mut f, &ctx, g);
    giro(&mut f, &ctx, vec![]);
    f.app().dialog_set_jump(0, "1");
    giro(&mut f, &ctx, vec![]);
    let ok = f
        .dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == "OK")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, ok);
    // numero non valido: la finestra resta aperta
    assert_eq!(f.dialogo_titolo(), Some("JUMPS"));
    let annulla = f
        .dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == "Cancel")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, annulla);
    giro(&mut f, &ctx, vec![]);
    assert_eq!(f.dialogo_titolo(), None);
    assert_eq!(etichette(&f)[3], "-10");
    // Esc chiude anche lui
    let g = f.ingranaggio_rect().unwrap().center();
    clic(&mut f, &ctx, g);
    giro(&mut f, &ctx, vec![]);
    assert!(f.dialogo_titolo().is_some());
    giro(
        &mut f,
        &ctx,
        vec![Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
    );
    giro(&mut f, &ctx, vec![]);
    assert_eq!(f.dialogo_titolo(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn there_is_no_gear_without_the_bar() {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_ingr_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, b"uno\n").unwrap();
    let c = || AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut app = App::open(&p, c()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(c));
    let ctx = Context::default();
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    assert!(f.ingranaggio_rect().is_none());
    let _ = std::fs::remove_file(p);
}
