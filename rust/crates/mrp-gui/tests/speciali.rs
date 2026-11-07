//! I caratteri speciali nella finestra (fase 2, 1.2): Alt+W e la voce del menu Visualizza li mostrano; la finestra non
//! cambia le colonne. Scritti da ARCH: gli attesi sono calcolati a mano.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_gui::menu::MENU;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::Cmd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn finestra() -> (Finestra, Context, PathBuf) {
    let cfg = || AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!(
        "mrp_speciali_test_{}_{}.txt",
        std::process::id(),
        n
    ));
    std::fs::write(&p, b"a b\tc\n").unwrap();
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
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

fn alt_w() -> Event {
    Event::Key {
        key: Key::W,
        physical_key: Some(Key::W),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    }
}

#[test]
fn alt_w_toggles_the_special_characters() {
    let (mut f, ctx, p) = finestra();
    assert!(!f.workspace().special_chars());
    giro(&mut f, &ctx, vec![alt_w()]);
    assert!(f.workspace().special_chars());
    assert!(f.app().special_chars());
    giro(&mut f, &ctx, vec![alt_w()]);
    assert!(!f.workspace().special_chars());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_menu_command_toggles_them_too_and_sits_in_the_view_menu() {
    let (mut f, _ctx, p) = finestra();
    let vista = MENU
        .iter()
        .find(|(t, _)| *t == "gui.menu.view")
        .map(|(_, v)| *v)
        .unwrap();
    assert!(vista.contains(&Some(Cmd::Special)));
    f.azione(Cmd::Special);
    assert!(f.workspace().special_chars());
    let _ = std::fs::remove_file(p);
}

#[test]
fn alt_t_toggles_following_in_the_window_and_the_status_says_so() {
    let (mut f, ctx, p) = finestra();
    assert!(!f.app().follow());
    let alt_t = Event::Key {
        key: Key::T,
        physical_key: Some(Key::T),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::ALT,
    };
    giro(&mut f, &ctx, vec![alt_t.clone()]);
    assert!(f.app().follow());
    assert!(f.app().status().follow);
    giro(&mut f, &ctx, vec![alt_t]);
    assert!(!f.app().follow());
    // il tail e' una delle tre modalita' d'uso: sta nel menu Modifica, non nel menu Log
    let voci = |titolo: &str| {
        MENU.iter()
            .find(|(t, _)| *t == titolo)
            .map(|(_, v)| *v)
            .unwrap()
    };
    assert!(voci("gui.menu.edit").contains(&Some(Cmd::Follow)));
    assert!(!voci("gui.menu.log").contains(&Some(Cmd::Follow)));
    let _ = std::fs::remove_file(p);
}
