//! La finestra delle impostazioni (Visualizza > Impostazioni, Ctrl+,): tutte le personalizzazioni in un posto, ognuna col suo
//! tasto scritto accanto, e una scheda con tutti i tasti. Le scelte fanno quello che fanno i menu e i tasti (e si salvano). Scritti
//! da ARCH.

use eframe::egui::{self, Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::{Cmd, COMMANDS};
use mrp_tui::theme::ThemeId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn percorsi() -> (PathBuf, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_impost_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    (d.join("a.txt"), d.join("profilo").join("settings.txt"))
}

fn finestra() -> (Finestra, Context, PathBuf, PathBuf) {
    let (p, settings) = percorsi();
    std::fs::write(&p, b"uno\ndue\n").unwrap();
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
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    (f, ctx, p, settings)
}

fn giro(f: &mut Finestra, ctx: &Context, eventi: Vec<Event>) -> FullOutput {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 800.0))),
        events: eventi,
        ..Default::default()
    };
    ctx.run(raw, |ctx| f.giro(ctx))
}

fn ctrl_virgola() -> Event {
    Event::Key {
        key: Key::Comma,
        physical_key: Some(Key::Comma),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    }
}

fn apri(f: &mut Finestra, ctx: &Context) {
    giro(f, ctx, vec![ctrl_virgola()]);
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
    assert!(f.impostazioni_aperta());
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
    giro(f, ctx, vec![]);
}

fn controllo(f: &Finestra, testo: &str) -> Pos2 {
    f.impostazioni_controlli()
        .iter()
        .find(|(_, t)| t == testo)
        .unwrap_or_else(|| panic!("manca {testo:?} in {:?}", f.impostazioni_controlli()))
        .0
        .center()
}

#[test]
fn ctrl_comma_and_the_menu_command_open_and_close_it() {
    let (mut f, ctx, p, _) = finestra();
    assert!(!f.impostazioni_aperta() && f.impostazioni_rect().is_none());
    apri(&mut f, &ctx);
    assert!(f.impostazioni_rect().is_some());
    giro(&mut f, &ctx, vec![ctrl_virgola()]);
    giro(&mut f, &ctx, vec![]);
    assert!(!f.impostazioni_aperta());
    f.azione(Cmd::Settings);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    assert!(f.impostazioni_aperta());
    let _ = std::fs::remove_file(p);
}

#[test]
fn every_setting_is_there() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let nomi: Vec<String> = f
        .impostazioni_controlli()
        .iter()
        .map(|(_, t)| t.clone())
        .collect();
    for atteso in [
        "General",
        "Keys",
        "Night (blue)",
        "Debian (blue)",
        "Console (default)",
        "Minimal",
        "Dark",
        "Light",
        "-",
        "+",
        "Default",
        "Show line numbers",
        "Show special characters",
        "Colour log lines",
        "Page-jump bar",
        "Change…",
    ] {
        assert!(
            nomi.iter().any(|n| n == atteso),
            "manca {atteso:?} in {nomi:?}"
        );
    }
    // nessun controllo ne copre un altro (una riga a capo nella griglia li sovrapponeva)
    let v = f.impostazioni_controlli();
    for (i, (a, na)) in v.iter().enumerate() {
        for (b, nb) in &v[i + 1..] {
            assert!(!a.intersects(*b), "{na:?} {a:?} copre {nb:?} {b:?}");
        }
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_switches_do_what_the_keys_do_and_are_saved() {
    let (mut f, ctx, p, settings) = finestra();
    apri(&mut f, &ctx);
    assert!(f.workspace().line_numbers() && !f.workspace().special_chars());
    let pos = controllo(&f, "Show line numbers");
    clic(&mut f, &ctx, pos);
    assert!(!f.workspace().line_numbers());
    let pos = controllo(&f, "Show special characters");
    clic(&mut f, &ctx, pos);
    assert!(f.workspace().special_chars());
    let pos = controllo(&f, "Colour log lines");
    clic(&mut f, &ctx, pos);
    assert!(f.workspace().log_colors());
    let pos = controllo(&f, "Page-jump bar");
    clic(&mut f, &ctx, pos);
    assert!(f.workspace().nav_bar());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    for voce in ["line_numbers=0", "special=1", "log_colors=1", "nav_bar=1"] {
        assert!(scritto.contains(voce), "{voce} in {scritto}");
    }
    // e si spengono di nuovo
    let pos = controllo(&f, "Show special characters");
    clic(&mut f, &ctx, pos);
    assert!(!f.workspace().special_chars());
    let _ = std::fs::remove_file(p);
}

#[test]
fn picking_a_theme_applies_and_saves_it() {
    let (mut f, ctx, p, settings) = finestra();
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Light");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().theme_id(), ThemeId::Light);
    assert!(std::fs::read_to_string(&settings)
        .unwrap()
        .contains("theme=light"));
    let pos = controllo(&f, "Night (blue)");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().theme_id(), ThemeId::Night);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_text_size_buttons_change_it_and_it_is_remembered() {
    let (mut f, ctx, p, settings) = finestra();
    apri(&mut f, &ctx);
    let prima = f.corpo();
    let pos = controllo(&f, "+");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.corpo(), prima + 1.0);
    clic(&mut f, &ctx, pos);
    assert_eq!(f.corpo(), prima + 2.0);
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(
        scritto.contains(&format!("font_size={}", prima as u16 + 2)),
        "{scritto}"
    );
    let pos = controllo(&f, "-");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.corpo(), prima + 1.0);
    let pos = controllo(&f, "Default");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.corpo(), 15.0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_remembered_size_is_used_at_the_start() {
    let (p, _) = percorsi();
    std::fs::write(&p, b"uno\n").unwrap();
    let cfg = move || AppConfig {
        chrome: false,
        font_size: Some(21),
        ..AppConfig::default()
    };
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let f = Finestra::da_app(app, Box::new(cfg));
    assert_eq!(f.corpo(), 21.0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_jump_change_button_opens_the_jump_window() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Change…");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.dialogo_titolo(), Some("JUMPS"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_keys_tab_lists_every_command_with_the_window_shortcuts_and_the_other_keys() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Keys");
    clic(&mut f, &ctx, pos);
    let righe = f.impostazioni_tasti().to_vec();
    // tutti i comandi del registro, coi tasti della finestra (F3, F4, F12) e Ctrl+, per le impostazioni
    for c in COMMANDS {
        let nome = f.app().text(c.label).trim_end_matches('…').to_string();
        assert!(righe.iter().any(|(n, _)| *n == nome), "manca {nome:?}");
    }
    let tasto = |nome: &str| {
        righe
            .iter()
            .find(|(n, _)| n == nome)
            .map(|(_, t)| t.as_str())
    };
    assert_eq!(tasto("Find"), Some("F3"));
    assert_eq!(tasto("Replace"), Some("F4"));
    assert_eq!(tasto("Edit mode"), Some("F12"));
    assert_eq!(tasto("Show line numbers"), Some("Alt+N"));
    assert_eq!(tasto("Settings"), Some("Ctrl+,"));
    // e gli altri tasti, dalla tabella dell'aiuto (descrizione, tasti)
    assert!(
        righe
            .iter()
            .any(|(n, t)| n.contains("select") && t.contains("Ctrl+A")),
        "{righe:?}"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_settings_window_can_be_dragged() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let r = f.impostazioni_rect().unwrap();
    let presa = Pos2::new(r.center().x, r.min.y + 8.0);
    let arrivo = presa + Vec2::new(-100.0, 60.0);
    giro(
        &mut f,
        &ctx,
        vec![
            Event::PointerMoved(presa),
            Event::PointerButton {
                pos: presa,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    giro(&mut f, &ctx, vec![Event::PointerMoved(arrivo)]);
    giro(
        &mut f,
        &ctx,
        vec![Event::PointerButton {
            pos: arrivo,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    giro(&mut f, &ctx, vec![]);
    let dopo = f.impostazioni_rect().unwrap();
    assert!(
        (dopo.min.x - (r.min.x - 100.0)).abs() < 3.0 && (dopo.min.y - (r.min.y + 60.0)).abs() < 3.0,
        "{r:?} -> {dopo:?}"
    );
    let _ = std::fs::remove_file(p);
}
