//! La finestra "Filtri" nella GUI: tre sezioni (Tempo, Livelli, Testo) nella stessa finestra, i livelli sono gli stessi bottoni della
//! timeline, il riassunto dice cosa applica OK, e OK con livelli e testo apre una vista filtrata sola. Scritti da ARCH.

use eframe::egui::{self, Context, Event, FullOutput, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::commands::Cmd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cfg() -> AppConfig {
    AppConfig {
        chrome: false,
        ..AppConfig::default()
    }
}

/// 144 righe, una ogni mezz'ora dal 2026-11-01, il livello gira ERROR, WARN, INFO, DEBUG.
fn finestra() -> (Finestra, Context, PathBuf) {
    let mut dati = String::new();
    for i in 0..144u32 {
        let livello = ["ERROR", "WARN", "INFO", "DEBUG"][(i % 4) as usize];
        dati.push_str(&format!(
            "2026-11-{:02} {:02}:{:02}:00 [{livello}] riga {i}\n",
            1 + i / 48,
            (i % 48) / 2,
            (i % 2) * 30
        ));
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_fgui_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
    ctx.style_mut(|s| s.animation_time = 0.0);
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

fn disegna(f: &mut Finestra, ctx: &Context) {
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
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
    disegna(f, ctx);
}

fn opzione(f: &Finestra, etichetta: &str) -> Pos2 {
    f.dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == etichetta)
        .unwrap_or_else(|| panic!("manca {etichetta:?} in {:?}", f.dialogo_opzioni()))
        .0
        .center()
}

fn ha_opzione(f: &Finestra, etichetta: &str) -> bool {
    f.dialogo_opzioni().iter().any(|(_, t)| t == etichetta)
}

fn pulsanti(f: &Finestra) -> Vec<String> {
    f.dialogo_pulsanti()
        .iter()
        .map(|(_, t)| t.clone())
        .collect()
}

fn pulsante(f: &Finestra, testo: &str) -> Pos2 {
    f.dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == testo)
        .unwrap_or_else(|| panic!("manca {testo:?} in {:?}", f.dialogo_pulsanti()))
        .0
        .center()
}

fn riassunto(f: &mut Finestra) -> String {
    match f.app().dialog_view() {
        Some(mrp_tui::app::dialogs::DialogView::Time { summary, .. }) => summary,
        altro => panic!("la finestra Filtri, non {altro:?}"),
    }
}

#[test]
fn the_window_has_three_sections_and_each_shows_its_own_controls() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Time));
    assert_eq!(f.dialogo_titolo(), Some("FILTERS"));
    for i in 0..3 {
        assert!(ha_opzione(&f, &format!("sezione {i}")), "scheda {i}");
    }
    // si apre sul Tempo: le due date, i tre pulsanti, niente livelli ne' testo
    assert_eq!(f.dialogo_campi().len(), 2);
    assert_eq!(pulsanti(&f), vec!["OK", "Recognise it again", "Cancel"]);
    assert!(!ha_opzione(&f, "livello 0") && !ha_opzione(&f, "modo 0"));
    // Livelli: le quattro caselle, nessun campo, OK e Annulla
    let s = opzione(&f, "sezione 1");
    clic(&mut f, &ctx, s);
    for i in 0..4 {
        assert!(ha_opzione(&f, &format!("livello {i}")), "livello {i}");
    }
    assert_eq!(f.dialogo_campi().len(), 0);
    assert_eq!(pulsanti(&f), vec!["OK", "Cancel"]);
    // Testo: un campo, i tre modi, le due caselle
    let s = opzione(&f, "sezione 2");
    clic(&mut f, &ctx, s);
    assert_eq!(f.dialogo_campi().len(), 1);
    for e in ["modo 0", "modo 1", "modo 2", "maiuscole", "nascondi"] {
        assert!(ha_opzione(&f, e), "{e}");
    }
    assert!(!ha_opzione(&f, "livello 0"));
    // si torna al Tempo senza perdere la finestra
    let s = opzione(&f, "sezione 0");
    clic(&mut f, &ctx, s);
    assert_eq!(f.dialogo_campi().len(), 2);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Time));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_levels_are_the_same_as_the_timeline_buttons_and_the_summary_follows() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    assert_eq!(riassunto(&mut f), "no filter chosen");
    let s = opzione(&f, "sezione 1");
    clic(&mut f, &ctx, s);
    let c = opzione(&f, "livello 1");
    clic(&mut f, &ctx, c);
    assert_eq!(
        f.app().timeline_filter_levels(),
        [false, true, false, false]
    );
    assert_eq!(riassunto(&mut f), "Levels: WARN");
    // un livello scelto fuori dalla finestra (i bottoni della timeline) si vede dentro
    f.app().set_timeline_filter_level(3, true);
    disegna(&mut f, &ctx);
    assert_eq!(riassunto(&mut f), "Levels: WARN, DEBUG");
    // e togliere la spunta qui lo toglie anche li'
    let c = opzione(&f, "livello 1");
    clic(&mut f, &ctx, c);
    assert_eq!(
        f.app().timeline_filter_levels(),
        [false, false, false, true]
    );
    // Annulla non applica niente: nessuna scheda nuova
    let a = pulsante(&f, "Cancel");
    clic(&mut f, &ctx, a);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.workspace().len(), 1);
    let _ = std::fs::remove_file(p);
}

#[test]
fn ok_with_levels_and_text_opens_one_filtered_tab() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    let s = opzione(&f, "sezione 1");
    clic(&mut f, &ctx, s);
    let c = opzione(&f, "livello 0");
    clic(&mut f, &ctx, c);
    let s = opzione(&f, "sezione 2");
    clic(&mut f, &ctx, s);
    f.app().dialog_filters_set_text("riga 1");
    disegna(&mut f, &ctx);
    assert_eq!(riassunto(&mut f), "Levels: ERROR \u{b7} Text: \"riga 1\"");
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().dialog_kind(), None);
    // la vista si apre in una scheda nuova, a lavoro finito
    let fine = Instant::now() + Duration::from_secs(20);
    while f.workspace().len() < 2 && Instant::now() < fine {
        disegna(&mut f, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(f.workspace().len(), 2);
    assert!(f.app().is_filter_view());
    assert!(
        f.app().file_name().contains("levels: ERROR"),
        "{}",
        f.app().file_name()
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_section_with_a_condition_is_marked_in_its_tab() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    // il segno e' nel titolo della scheda: cambia la larghezza della scheda "Livelli" quando ha una condizione
    let larghezza = |f: &Finestra| {
        f.dialogo_opzioni()
            .iter()
            .find(|(_, t)| t == "sezione 1")
            .unwrap()
            .0
            .width()
    };
    let prima = larghezza(&f);
    f.app().set_timeline_filter_level(0, true);
    disegna(&mut f, &ctx);
    assert!(larghezza(&f) > prima, "il punto allarga il titolo");
    let _ = std::fs::remove_file(p);
}
