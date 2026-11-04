//! La finestra Filtri riaperta nella GUI: il tempo scelto (la regola dell'utente) resta quello selezionato e il calendario si apre sulle
//! sue date. Scritti da ARCH sul log di `time_example.rs` (orario all'inizio riga 2026-11-03, data dentro la riga 2019-09-10..20).

use eframe::egui::{self, Context, Event, FullOutput, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::commands::Cmd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const REGOLA: &str = "'@ 'yyyy-MM-dd HH:mm";

fn cfg() -> AppConfig {
    AppConfig {
        chrome: false,
        time_rules: vec![(REGOLA.to_string(), 200, 0)],
        ..AppConfig::default()
    }
}

fn finestra() -> (Finestra, Context, PathBuf) {
    let mut dati = String::new();
    for i in 0..30u32 {
        let ore = 8 + 8 * i;
        let (giorno, ora) = (10 + ore / 24, ore % 24);
        dati.push_str(&format!(
            "2026-11-03 11:19:{i:02}.350 +01:00 [INF] XC4 funding @ 2019-09-{giorno:02} {ora:02}:00 rate=1\n"
        ));
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_trgui_{}_{}.log", std::process::id(), n));
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

/// Apre la tendina dei tempi del dialogo e ne sceglie la voce con quell'etichetta (`fonte N` o `altro`).
fn dalla_tendina(f: &mut Finestra, ctx: &Context, voce: &str) {
    let tendina = opzione(f, "fonti");
    clic(f, ctx, tendina);
    let v = opzione(f, voce);
    clic(f, ctx, v);
}

fn pulsante(f: &Finestra, testo: &str) -> Pos2 {
    f.dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == testo)
        .unwrap_or_else(|| panic!("manca {testo:?} in {:?}", f.dialogo_pulsanti()))
        .0
        .center()
}

/// Apre i Filtri e sceglie la regola dell'utente dall'elenco dei tempi.
fn scegli_la_regola(f: &mut Finestra, ctx: &Context) {
    f.azione(Cmd::TimeRange);
    disegna(f, ctx);
    let i = f
        .app()
        .time_sources()
        .iter()
        .position(|s| s.pattern == REGOLA)
        .expect("la regola e' fra i tempi offerti");
    dalla_tendina(f, ctx, &format!("fonte {i}"));
    assert_eq!(f.app().time_field_text(0), REGOLA);
    let ok = pulsante(f, "OK");
    clic(f, ctx, ok);
    assert_eq!(f.app().dialog_kind(), None, "{}", f.app().time_error_text());
}

#[test]
fn reopening_the_filters_keeps_my_rule_and_the_calendar_shows_2019() {
    let (mut f, ctx, p) = finestra();
    scegli_la_regola(&mut f, &ctx);
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Time));
    let scelta = f.app().time_source_selected().expect("una fonte scelta");
    assert_eq!(f.app().time_sources()[scelta].pattern, REGOLA);
    assert_eq!(f.app().time_field_text(0), REGOLA);
    let cal = f.app().time_calendar(2).expect("il calendario");
    assert_eq!(cal.first, Some((2019, 9, 10)), "{cal:?}");
    assert_eq!(cal.month, (2019, 9));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_rule_survives_a_filtered_view_in_both_tabs() {
    let (mut f, ctx, p) = finestra();
    f.app().set_timeline_filter_level(2, true);
    scegli_la_regola(&mut f, &ctx);
    // la vista filtrata si apre a lavoro finito
    let fine = std::time::Instant::now() + Duration::from_secs(20);
    while f.workspace().len() < 2 && std::time::Instant::now() < fine {
        disegna(&mut f, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(f.workspace().len(), 2);
    for scheda in [1usize, 0] {
        f.workspace().select(scheda);
        disegna(&mut f, &ctx);
        f.azione(Cmd::TimeRange);
        disegna(&mut f, &ctx);
        let scelta = f.app().time_source_selected();
        let campo = f.app().time_field_text(0).to_string();
        let fonti: Vec<String> = f
            .app()
            .time_sources()
            .iter()
            .map(|s| s.pattern.clone())
            .collect();
        assert_eq!(
            scelta.map(|i| fonti[i].clone()),
            Some(REGOLA.to_string()),
            "scheda {scheda}: campo {campo:?}, fonti {fonti:?}"
        );
        let cal = f.app().time_calendar(2).expect("il calendario");
        assert_eq!(
            cal.first.map(|d| d.0),
            Some(2019),
            "scheda {scheda}: {cal:?}"
        );
        let a = pulsante(&f, "Cancel");
        clic(&mut f, &ctx, a);
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_rule_survives_with_the_timeline_on() {
    let (mut f, ctx, p) = finestra();
    f.app().set_timeline(true);
    disegna(&mut f, &ctx);
    scegli_la_regola(&mut f, &ctx);
    for _ in 0..30 {
        disegna(&mut f, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    let scelta = f.app().time_source_selected();
    let fonti: Vec<String> = f
        .app()
        .time_sources()
        .iter()
        .map(|s| s.pattern.clone())
        .collect();
    assert_eq!(
        scelta.map(|i| fonti[i].clone()),
        Some(REGOLA.to_string()),
        "{fonti:?}"
    );
    let cal = f.app().time_calendar(2).expect("il calendario");
    assert_eq!(cal.first.map(|d| d.0), Some(2019), "{cal:?}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_rule_of_mine_can_be_taken_off_the_list_with_its_cross() {
    let (mut f, ctx, p) = finestra();
    // una regola dell'utente che legge righe del file ma non coincide con un tempo trovato da solo (quella di `cfg`, invece, e' la
    // stessa della data dopo "@ " e vale come quella)
    let mia = "yyyy-MM-dd HH:mm";
    f.app().set_time_rules(vec![(mia.to_string(), 0, 0)]);
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    let i = f
        .app()
        .time_sources()
        .iter()
        .position(|s| s.pattern == mia)
        .expect("la regola e' fra i tempi offerti");
    // chiusa la tendina non ci sono ne' voci ne' croci
    assert!(!f
        .dialogo_opzioni()
        .iter()
        .any(|(_, t)| t.starts_with("togli")));
    let tendina = opzione(&f, "fonti");
    clic(&mut f, &ctx, tendina);
    // la croce c'e' solo sulle regole dell'utente
    let croci = f
        .dialogo_opzioni()
        .iter()
        .filter(|(_, t)| t.starts_with("togli"))
        .count();
    assert_eq!(croci, 1);
    let x = opzione(&f, &format!("togli {i}"));
    clic(&mut f, &ctx, x);
    assert!(
        f.app().time_rules().is_empty(),
        "{:?}",
        f.app().time_rules()
    );
    assert!(f.app().time_sources().iter().all(|s| s.pattern != mia));
    // resta aperta la finestra Filtri, con gli altri tempi
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Time));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_filters_page_fits_a_normal_screen_with_everything_in_it() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    let r = f.dialogo_rect().expect("la finestra");
    println!("ALTEZZA chiusa {}", r.height());
    // con i tre riquadri la pagina sta in meno di 600 punti (uno schermo da 768 con la barra del sistema)
    assert!(r.height() < 600.0, "alta {}", r.height());
    // aperto "Altro" (la regola scritta a mano) cresce, ma resta sotto i 900
    dalla_tendina(&mut f, &ctx, "altro");
    disegna(&mut f, &ctx);
    let r = f.dialogo_rect().expect("la finestra");
    println!("ALTEZZA altro {}", r.height());
    assert!(r.height() < 900.0, "alta {}", r.height());
    let _ = std::fs::remove_file(p);
}

#[test]
fn on_a_low_screen_the_boxes_scroll_and_the_window_stays_on_screen() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    dalla_tendina(&mut f, &ctx, "altro");
    // uno schermo basso: 560 punti
    for _ in 0..3 {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 560.0))),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| f.giro(ctx));
    }
    let r = f.dialogo_rect().expect("la finestra");
    assert!(
        r.height() <= 560.0,
        "alta {} su uno schermo di 560",
        r.height()
    );
    // i pulsanti restano visibili
    let ok = pulsante(&f, "OK");
    assert!(ok.y < 560.0, "OK a {}", ok.y);
    let _ = std::fs::remove_file(p);
}
