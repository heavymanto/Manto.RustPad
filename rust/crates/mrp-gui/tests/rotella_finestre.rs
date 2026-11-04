//! La rotellina sopra una finestra (le impostazioni, un dialogo) e' della finestra: non fa scorrere anche il documento che sta sotto.
//! Scritti da ARCH: un file di 500 righe, la rotellina che di norma lo scorre.

use eframe::egui::{
    Context, Event, FullOutput, Modifiers, MouseWheelUnit, Pos2, RawInput, Rect, Vec2,
};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::Cmd;
use std::time::Duration;

fn finestra() -> (Finestra, Context, std::path::PathBuf) {
    let p = std::env::temp_dir().join(format!("mrp_rotella_{}.txt", std::process::id()));
    let dati: String = (0..500).map(|i| format!("riga {i}\n")).collect();
    std::fs::write(&p, dati).unwrap();
    let cfg = || AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
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
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 800.0))),
        events: eventi,
        ..Default::default()
    };
    ctx.run(raw, |ctx| f.giro(ctx))
}

fn rotella() -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: Vec2::new(0.0, -5.0),
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn the_wheel_scrolls_the_document_but_not_under_the_settings_window() {
    let (mut f, ctx, p) = finestra();
    // senza finestre la rotellina scorre il documento
    assert_eq!(f.app().top(), 0);
    giro(&mut f, &ctx, vec![rotella()]);
    giro(&mut f, &ctx, vec![]);
    let scorso = f.app().top();
    assert!(scorso > 0, "la rotellina scorre il documento");
    // con le impostazioni aperte (anche sulla scheda dei tasti) il documento sotto sta fermo
    f.azione(Cmd::Settings);
    giro(&mut f, &ctx, vec![]);
    f.scheda_impostazioni(1);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![rotella()]);
    giro(&mut f, &ctx, vec![rotella()]);
    giro(&mut f, &ctx, vec![]);
    assert_eq!(f.app().top(), scorso, "il documento sotto non si e' mosso");
    // chiuse le impostazioni (Esc), la rotellina torna al documento
    f.azione(Cmd::Settings);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![rotella()]);
    giro(&mut f, &ctx, vec![]);
    assert!(f.app().top() > scorso);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_wheel_does_not_scroll_the_document_under_a_dialog_either() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::Goto);
    giro(&mut f, &ctx, vec![]);
    assert!(f.app().dialog_kind().is_some());
    giro(&mut f, &ctx, vec![rotella()]);
    giro(&mut f, &ctx, vec![]);
    assert_eq!(f.app().top(), 0);
    let _ = std::fs::remove_file(p);
}
