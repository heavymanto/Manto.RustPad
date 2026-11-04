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

/// Apre la tendina dei tempi del dialogo e ne sceglie la voce con quell'etichetta (`fonte N` o `altro`).
fn dalla_tendina(f: &mut Finestra, ctx: &Context, voce: &str) {
    let tendina = opzione(f, "fonti");
    clic(f, ctx, tendina);
    let v = opzione(f, voce);
    clic(f, ctx, v);
}

#[test]
fn the_dates_inside_the_lines_are_rows_of_the_list_and_other_has_the_date_number() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    // il tempo in uso (l'orario all'inizio della riga) e la data dei dati dentro la riga, dopo "@ ": due righe dell'elenco
    let tendina = opzione(&f, "fonti");
    clic(&mut f, &ctx, tendina);
    opzione(&f, "fonte 0");
    let dati = opzione(&f, "fonte 1");
    // il numero della data non si vede finche' "Altro" non e' aperto
    assert!(!f.dialogo_opzioni().iter().any(|(_, t)| t == "occorrenza"));
    clic(&mut f, &ctx, dati);
    // scelta: il pattern con il testo prima della data e "entro" 200; i campi del formato restano chiusi
    assert_eq!(f.app().time_field_text(0), "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(f.app().time_field_text(1), "200");
    assert_eq!(f.dialogo_campi().len(), 2, "solo Dal e Al");
    // "Altro" li apre, con il numero della data nella riga
    dalla_tendina(&mut f, &ctx, "altro");
    opzione(&f, "occorrenza");
    assert_eq!(f.dialogo_campi().len(), 4, "pattern, entro, dal, al");
    let _ = std::fs::remove_file(p);
}

#[test]
fn under_other_an_example_gives_the_rule_to_pick() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    // il campo dell'esempio e il pulsante stanno sotto "Altro"
    assert!(!f.dialogo_opzioni().iter().any(|(_, t)| t == "esempio"));
    dalla_tendina(&mut f, &ctx, "altro");
    opzione(&f, "esempio");
    // un esempio e il pulsante: appaiono le regole ricavate, con la loro prima e ultima data
    f.app()
        .dialog_time_set_example("funding @ 2019-09-10 08:00");
    disegna(&mut f, &ctx);
    let trova = opzione(&f, "trova");
    clic(&mut f, &ctx, trova);
    let regola = opzione(&f, "regola 0");
    assert_eq!(f.app().time_example_selected(), None);
    // sceglierla mette il pattern nei campi, e la riga risulta scelta
    clic(&mut f, &ctx, regola);
    assert_eq!(f.app().time_field_text(0), "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(f.app().time_example_selected(), Some(0));
    // un esempio senza date: il messaggio, nessuna regola
    f.app().dialog_time_set_example("niente");
    disegna(&mut f, &ctx);
    let trova = opzione(&f, "trova");
    clic(&mut f, &ctx, trova);
    assert!(!f
        .dialogo_opzioni()
        .iter()
        .any(|(_, t)| t.starts_with("regola ")));
    let _ = std::fs::remove_file(p);
}
