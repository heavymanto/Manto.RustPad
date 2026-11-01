//! Il tema della finestra (B5): i temi Chiaro e Scuro, il tema del sistema come predefinito quando l'utente non ne ha scelto uno,
//! e i controlli di egui (menu, pulsanti, finestre) che seguono la scelta. Scritti da ARCH: gli attesi sono calcolati a mano.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::theme::ThemeId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tema_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, b"uno\n").unwrap();
    p
}

fn cfg(tema: ThemeId) -> AppConfig {
    AppConfig {
        chrome: false,
        theme: tema,
        ..AppConfig::default()
    }
}

fn finestra(tema: ThemeId) -> (Finestra, Context, PathBuf) {
    let p = file();
    let mut app = App::open(&p, cfg(tema)).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let f = Finestra::da_app(app, Box::new(move || cfg(tema)));
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

fn scuri(ctx: &Context) -> bool {
    ctx.style().visuals.dark_mode
}

#[test]
fn the_controls_follow_the_theme_light_or_dark() {
    for (tema, scuro) in [
        (ThemeId::Dark, true),
        (ThemeId::Light, false),
        (ThemeId::Night, true),
        (ThemeId::Debian, true),
        (ThemeId::Console, true),
        (ThemeId::Minimal, true),
    ] {
        let (mut f, ctx, p) = finestra(tema);
        giro(&mut f, &ctx, vec![]);
        giro(&mut f, &ctx, vec![]);
        assert_eq!(scuri(&ctx), scuro, "{tema:?}");
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn the_system_theme_is_the_default_only_when_nothing_was_chosen() {
    // il sistema e' scuro -> Scuro, chiaro -> Chiaro, sconosciuto -> il tema com'e'
    for (sistema, atteso) in [
        (Some(true), ThemeId::Dark),
        (Some(false), ThemeId::Light),
        (None, ThemeId::Console),
    ] {
        let (f, _ctx, p) = finestra(ThemeId::Console);
        let mut f = f.con_tema_di_sistema(sistema);
        assert_eq!(f.app().theme_id(), atteso, "{sistema:?}");
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn the_system_theme_reaches_every_tab() {
    let (mut f, _ctx, p) = finestra(ThemeId::Console);
    let q = file();
    f.workspace().open(&q).unwrap();
    // senza un giro in mezzo: lo cambia davvero in tutte le schede (non e' il giro che le allinea)
    let mut f = f.con_tema_di_sistema(Some(false));
    assert_eq!(f.workspace().len(), 2);
    assert!(f
        .workspace()
        .tabs()
        .iter()
        .all(|a| a.theme_id() == ThemeId::Light));
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn picking_a_theme_in_the_menu_switches_the_controls() {
    let (mut f, ctx, p) = finestra(ThemeId::Dark);
    giro(&mut f, &ctx, vec![]);
    assert!(scuri(&ctx));
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
    let pos = f.dialogo_voci()[5].center();
    giro(
        &mut f,
        &ctx,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    giro(
        &mut f,
        &ctx,
        vec![Event::PointerButton {
            pos,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    assert_eq!(f.app().theme_id(), ThemeId::Light);
    assert!(!scuri(&ctx), "i controlli sono passati al chiaro");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_area_around_the_grid_has_the_theme_background() {
    // il resto dell'area che non basta per una riga intera non e' piu' nero: ha il fondo del tema (Scuro #1e1e1e, Chiaro bianco)
    for (tema, fondo) in [
        (ThemeId::Dark, [0x1e, 0x1e, 0x1e]),
        (ThemeId::Light, [0xff, 0xff, 0xff]),
    ] {
        let (mut f, ctx, p) = finestra(tema);
        giro(&mut f, &ctx, vec![]);
        giro(&mut f, &ctx, vec![]);
        assert_eq!(f.fondo(), fondo, "{tema:?}");
        let _ = std::fs::remove_file(p);
    }
}
