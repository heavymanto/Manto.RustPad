//! La data dentro la riga nella finestra "Tempo": il suggerimento "in queste righe c'e' anche un'altra data" con il suo pulsante e la
//! scelta di quale data della riga vale. Scritti da ARCH: trenta righe di funding con la data dei dati dentro la riga.

use eframe::egui::{self, Context, Event, FullOutput, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::Cmd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cfg() -> AppConfig {
    AppConfig {
        chrome: false,
        ..AppConfig::default()
    }
}

fn finestra() -> (Finestra, Context, PathBuf) {
    let mut dati = String::new();
    for i in 0..30u32 {
        let ore = 8 + 8 * i;
        dati.push_str(&format!(
            "2026-11-03 11:19:{i:02}.350 +01:00 [INF] XC4 funding @ 2019-09-{:02} {:02}:00 rate=1
",
            10 + ore / 24,
            ore % 24
        ));
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tskipg_{}_{}.log", std::process::id(), n));
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

fn opzione(f: &Finestra, e: &str) -> Pos2 {
    f.dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == e)
        .unwrap_or_else(|| panic!("manca {e:?} in {:?}", f.dialogo_opzioni()))
        .0
        .center()
}

#[test]
fn the_suggestion_has_a_button_that_uses_it_and_the_custom_format_has_the_date_number() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    // il formato riconosciuto e' l'ora di scrittura; nelle righe c'e' anche la data dei dati: il suggerimento e il suo pulsante
    let usa = opzione(&f, "suggerimento");
    // il numero della data non si vede finche' il formato personalizzato non e' aperto
    assert!(!f.dialogo_opzioni().iter().any(|(_, t)| t == "occorrenza"));
    clic(&mut f, &ctx, usa);
    // usato: il pattern con il testo prima della data, "entro" 200, e ora i campi personalizzati con il numero della data
    assert_eq!(f.app().time_field_text(0), "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(f.app().time_field_text(1), "200");
    opzione(&f, "occorrenza");
    assert_eq!(f.dialogo_campi().len(), 4, "pattern, entro, dal, al");
    // il suggerimento sparisce da solo quando il formato e' stato scelto
    // (con un numero di data diverso dal primo non ha piu' senso proporlo)
    f.app().dialog_time_set_skip(1);
    disegna(&mut f, &ctx);
    assert!(!f.dialogo_opzioni().iter().any(|(_, t)| t == "suggerimento"));
    let _ = std::fs::remove_file(p);
}
