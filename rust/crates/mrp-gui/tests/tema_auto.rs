//! Il tema della finestra: di fabbrica segue il sistema (chiaro o scuro) e lo segue anche se cambia mentre il programma e' aperto;
//! l'utente puo' scegliere Chiaro o Scuro e allora resta quello, e tornare ad "Automatico". Nella finestra i temi sono solo
//! questi (i quattro del terminale restano del terminale). Scritti da ARCH.

use eframe::egui::{
    self, Context, Event, FullOutput, Modifiers, Pos2, RawInput, Rect, Theme, Vec2,
};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::theme::ThemeId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn finestra() -> (Finestra, Context, PathBuf, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_temaauto_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("a.txt");
    std::fs::write(&p, b"uno\ndue\n").unwrap();
    let settings = d.join("profilo").join("settings.txt");
    let cfg = {
        let s = settings.clone();
        move || AppConfig {
            chrome: false,
            settings_path: Some(s.clone()),
            ..AppConfig::default()
        }
    };
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
    giro(&mut f, &ctx, None, vec![]);
    giro(&mut f, &ctx, None, vec![]);
    (f, ctx, p, settings)
}

fn giro(f: &mut Finestra, ctx: &Context, sistema: Option<Theme>, eventi: Vec<Event>) -> FullOutput {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 800.0))),
        system_theme: sistema,
        events: eventi,
        ..Default::default()
    };
    ctx.run(raw, |ctx| f.giro(ctx))
}

fn clic(f: &mut Finestra, ctx: &Context, sistema: Option<Theme>, pos: Pos2) {
    for premuto in [true, false] {
        giro(
            f,
            ctx,
            sistema,
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
    giro(f, ctx, sistema, vec![]);
    giro(f, ctx, sistema, vec![]);
}

fn controllo(f: &Finestra, testo: &str) -> Pos2 {
    f.impostazioni_controlli()
        .iter()
        .find(|(_, t)| t == testo)
        .unwrap_or_else(|| panic!("manca {testo:?} in {:?}", f.impostazioni_controlli()))
        .0
        .center()
}

fn apri_impostazioni(f: &mut Finestra, ctx: &Context, sistema: Option<Theme>) {
    f.azione(mrp_tui::commands::Cmd::Settings);
    giro(f, ctx, sistema, vec![]);
    giro(f, ctx, sistema, vec![]);
}

#[test]
fn without_a_choice_the_window_follows_the_system_even_when_it_changes() {
    let (mut f, ctx, p, _) = finestra();
    giro(&mut f, &ctx, Some(Theme::Light), vec![]);
    giro(&mut f, &ctx, Some(Theme::Light), vec![]);
    assert_eq!(f.app().theme_id(), ThemeId::Light);
    giro(&mut f, &ctx, Some(Theme::Dark), vec![]);
    assert_eq!(f.app().theme_id(), ThemeId::Dark);
    giro(&mut f, &ctx, Some(Theme::Light), vec![]);
    assert_eq!(f.app().theme_id(), ThemeId::Light);
    // e le schede nuove lo prendono
    let q = p.with_file_name("b.txt");
    std::fs::write(&q, b"x\n").unwrap();
    f.workspace().open(&q).unwrap();
    assert!(f
        .workspace()
        .tabs()
        .iter()
        .all(|a| a.theme_id() == ThemeId::Light));
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn changing_any_other_setting_does_not_pin_the_theme() {
    // prima il tema veniva scritto con la prima impostazione cambiata (e da li' in poi non seguiva piu' il sistema)
    let (mut f, ctx, p, settings) = finestra();
    giro(&mut f, &ctx, Some(Theme::Light), vec![]);
    f.workspace().toggle_line_numbers();
    giro(&mut f, &ctx, Some(Theme::Light), vec![]);
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("line_numbers=0"), "{scritto}");
    assert!(!scritto.contains("theme="), "{scritto}");
    giro(&mut f, &ctx, Some(Theme::Dark), vec![]);
    assert_eq!(f.app().theme_id(), ThemeId::Dark, "segue ancora il sistema");
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn the_settings_offer_automatic_light_and_dark_only() {
    let (mut f, ctx, p, _) = finestra();
    apri_impostazioni(&mut f, &ctx, Some(Theme::Light));
    let nomi: Vec<String> = f
        .impostazioni_controlli()
        .iter()
        .map(|(_, t)| t.clone())
        .collect();
    for atteso in [
        "Automatic (system)",
        "Blue (Total Commander)",
        "Debian (blue)",
        "Dark",
        "Light",
    ] {
        assert!(
            nomi.iter().any(|n| n == atteso),
            "manca {atteso:?} in {nomi:?}"
        );
    }
    for no in ["Console (default)", "Minimal"] {
        assert!(
            !nomi.iter().any(|n| n == no),
            "{no:?} non va nella finestra"
        );
    }
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn choosing_dark_or_light_pins_it_and_automatic_gives_the_system_back() {
    let (mut f, ctx, p, settings) = finestra();
    let sistema = Some(Theme::Light);
    giro(&mut f, &ctx, sistema, vec![]);
    apri_impostazioni(&mut f, &ctx, sistema);
    let pos = controllo(&f, "Dark");
    clic(&mut f, &ctx, sistema, pos);
    assert_eq!(f.app().theme_id(), ThemeId::Dark);
    assert!(std::fs::read_to_string(&settings)
        .unwrap()
        .contains("theme=dark"));
    // il sistema e' chiaro ma la scelta e' scuro: resta scuro
    giro(&mut f, &ctx, sistema, vec![]);
    assert_eq!(f.app().theme_id(), ThemeId::Dark);
    // Automatico: torna come il sistema e la scelta sparisce dal file
    let pos = controllo(&f, "Automatic (system)");
    clic(&mut f, &ctx, sistema, pos);
    assert_eq!(f.app().theme_id(), ThemeId::Light);
    assert!(!std::fs::read_to_string(&settings)
        .unwrap()
        .contains("theme="));
    giro(&mut f, &ctx, Some(Theme::Dark), vec![]);
    assert_eq!(f.app().theme_id(), ThemeId::Dark);
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn the_theme_menu_of_the_window_has_four_rows_blue_debian_dark_and_light() {
    let (mut f, ctx, p, _) = finestra();
    giro(
        &mut f,
        &ctx,
        None,
        vec![Event::Key {
            key: egui::Key::F6,
            physical_key: Some(egui::Key::F6),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
    );
    giro(&mut f, &ctx, None, vec![]);
    giro(&mut f, &ctx, None, vec![]);
    assert_eq!(f.dialogo_voci().len(), 4);
    // scegliere l'ultima (Chiaro) la fissa
    let pos = f.dialogo_voci()[3].center();
    clic(&mut f, &ctx, None, pos);
    assert_eq!(f.app().theme_id(), ThemeId::Light);
    assert!(f.app().theme_chosen());
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}
