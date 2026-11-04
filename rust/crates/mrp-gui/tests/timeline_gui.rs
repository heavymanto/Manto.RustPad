//! La barra della timeline nella finestra (F2): spenta di fabbrica, si accende dal menu Log, sta a sinistra della scrollbar ed e' larga,
//! disegna una riga per giorno con le date, un clic salta al punto, il menu del passo cambia i bucket. Scritti da ARCH: tre giorni di
//! righe una ogni mezz'ora (144 righe), gli offset si calcolano a mano.

use eframe::egui::{self, Context, Event, FullOutput, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_gui::menu::{self, MENU};
use mrp_tui::app::timeline::TimelineStatus;
use mrp_tui::app::{App, AppConfig};
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

fn finestra() -> (Finestra, Context, PathBuf, Vec<u64>) {
    let mut dati = String::new();
    let mut offset = Vec::new();
    for i in 0..144u32 {
        offset.push(dati.len() as u64);
        dati.push_str(&format!(
            "2026-11-{:02} {:02}:{:02}:00 [INFO ] riga {i}\n",
            1 + i / 48,
            (i % 48) / 2,
            (i % 2) * 30
        ));
    }
    offset.push(dati.len() as u64);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlgui_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
    ctx.style_mut(|s| s.animation_time = 0.0);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    (f, ctx, p, offset)
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

/// Disegna finche' la timeline e' pronta (le tacche le cerca un thread).
fn pronta(f: &mut Finestra, ctx: &Context) {
    let fine = Instant::now() + Duration::from_secs(20);
    loop {
        disegna(f, ctx);
        match f.app().timeline_status() {
            TimelineStatus::Building(_) if Instant::now() < fine => {
                std::thread::sleep(Duration::from_millis(5))
            }
            _ => return,
        }
    }
}

#[test]
fn it_is_off_by_default_and_the_log_menu_switches_it_on() {
    let (mut f, ctx, p, _) = finestra();
    assert!(f.timeline_disegnata().rect.is_none());
    // la voce e' nel menu Log e non e' accesa
    let log: Vec<Cmd> = MENU[3].1.iter().flatten().copied().collect();
    assert!(log.contains(&Cmd::Timeline));
    assert_eq!(menu::acceso(f.workspace(), Cmd::Timeline), Some(false));
    f.azione(Cmd::Timeline);
    pronta(&mut f, &ctx);
    assert!(f.timeline_disegnata().rect.is_some());
    assert_eq!(menu::acceso(f.workspace(), Cmd::Timeline), Some(true));
    // di nuovo: sparisce
    f.azione(Cmd::Timeline);
    disegna(&mut f, &ctx);
    assert!(f.timeline_disegnata().rect.is_none());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_bar_is_wide_and_sits_left_of_the_scrollbar() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    pronta(&mut f, &ctx);
    let barra = f.timeline_disegnata().rect.expect("la barra");
    let scroll = f.scrollbar_rect().expect("la scrollbar");
    assert!(
        barra.width() >= 90.0,
        "larga abbastanza da leggere le date: {barra:?}"
    );
    assert!(
        barra.max.x <= scroll.min.x + 0.5 && barra.min.x < scroll.min.x,
        "{barra:?} {scroll:?}"
    );
    // la barra e la scrollbar sono alte uguali (sotto il menu, sopra la barra di stato)
    assert!((barra.min.y - scroll.min.y).abs() < 1.0 && (barra.max.y - scroll.max.y).abs() < 1.0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn it_draws_a_row_per_day_with_dates_and_a_click_jumps_there() {
    let (mut f, ctx, p, offset) = finestra();
    f.azione(Cmd::Timeline);
    f.workspace()
        .active_mut()
        .set_timeline_step(mrp_core::timeline::Step::Day);
    pronta(&mut f, &ctx);
    let st = f.timeline_disegnata().clone();
    assert_eq!(st.righe, 3, "tre giorni, tre righe");
    assert!(st.scritte >= 3, "le tre date");
    let corpo = st.corpo.expect("il corpo");
    // un clic nel secondo giorno (a meta' del suo tratto): il cursore va nei suoi byte
    let y = corpo.min.y + corpo.height() * 0.5;
    clic(&mut f, &ctx, Pos2::new(corpo.center().x, y));
    let cursore = f.app().cursor();
    assert!(cursore > offset[48] && cursore < offset[96], "{cursore}");
    // un clic all'inizio del corpo porta all'inizio del file
    clic(&mut f, &ctx, Pos2::new(corpo.center().x, corpo.min.y + 0.5));
    assert_eq!(f.app().cursor(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_step_menu_is_in_the_header_and_a_shorter_step_gives_more_rows() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    pronta(&mut f, &ctx);
    let passo = f.timeline_disegnata().passo.expect("il menu del passo");
    assert!(passo.width() > 40.0);
    // dall'API: l'ora fa 72 bucket (alti 3 punti l'uno entrano nell'altezza della finestra)
    f.workspace()
        .active_mut()
        .set_timeline_step(mrp_core::timeline::Step::Hour);
    pronta(&mut f, &ctx);
    assert_eq!(f.timeline_disegnata().righe, 72);
    let _ = std::fs::remove_file(p);
}

/// I testi disegnati in un giro (per sapere cosa c'e' scritto sullo schermo).
fn testi(o: &FullOutput) -> Vec<String> {
    fn visita(s: &egui::Shape, v: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => v.push(t.galley.text().to_string()),
            egui::Shape::Vec(tutte) => tutte.iter().for_each(|x| visita(x, v)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &o.shapes {
        visita(&c.shape, &mut v);
    }
    v
}

#[test]
fn the_step_is_automatic_and_the_header_says_which_one_was_chosen() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    pronta(&mut f, &ctx);
    // tre giorni di righe: da solo sceglie l'ora, e lo scrive nell'intestazione
    let o = giro(&mut f, &ctx, vec![]);
    let scritti = testi(&o);
    assert!(scritti.iter().any(|t| t == "Auto: 1 hour"), "{scritti:?}");
    assert_eq!(f.timeline_disegnata().righe, 72);
    // un passo fisso: il nome del passo e basta
    f.workspace()
        .active_mut()
        .set_timeline_step(mrp_core::timeline::Step::Day);
    pronta(&mut f, &ctx);
    let o = giro(&mut f, &ctx, vec![]);
    let scritti = testi(&o);
    assert!(scritti.iter().any(|t| t == "1 day"), "{scritti:?}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_level_buttons_toggle_the_filter_and_the_marks_show_the_counted_levels() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    f.workspace()
        .active_mut()
        .set_timeline_step(mrp_core::timeline::Step::Day);
    // aspetta anche i livelli (la passata e' un thread)
    let fine = Instant::now() + Duration::from_secs(20);
    loop {
        disegna(&mut f, &ctx);
        if f.app().timeline_levels_ready() || Instant::now() >= fine {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // i quattro pulsanti dei livelli e quello del filtro
    let nomi: Vec<String> = f
        .timeline_disegnata()
        .chip
        .iter()
        .map(|(_, t)| t.clone())
        .collect();
    assert_eq!(nomi, vec!["ERROR", "WARN", "INFO", "DEBUG", "filtra"]);
    // un clic sceglie il livello, un altro lo toglie
    let e = f.timeline_disegnata().chip[0].0.center();
    clic(&mut f, &ctx, e);
    assert_eq!(
        f.app().timeline_filter_levels(),
        [true, false, false, false]
    );
    clic(&mut f, &ctx, e);
    assert_eq!(f.app().timeline_filter_levels(), [false; 4]);
    // le righe del log di prova sono tutte INFO: un segno per giorno (tre giorni), nessuno per gli altri livelli
    assert_eq!(f.timeline_disegnata().strisce, 3);
    // il pulsante del filtro senza livelli scelti lo dice
    let filtra = f.timeline_disegnata().chip[4].0.center();
    clic(&mut f, &ctx, filtra);
    assert_eq!(
        f.app().flash(),
        Some("Choose at least one level first (E, W, I, D)")
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_clock_button_turns_the_timeline_on_and_off_and_the_funnel_is_the_date_filter() {
    let (mut f, ctx, p, _) = finestra();
    let bottone = |f: &Finestra, c: Cmd| {
        f.pulsanti()
            .iter()
            .find(|b| b.cmd == c)
            .map(|b| (b.rect, b.attivo, b.acceso))
            .unwrap_or_else(|| panic!("manca il pulsante di {c:?}"))
    };
    // l'orologio (timeline) e l'imbuto (filtro per data) sono due pulsanti, uno accanto all'altro
    let (orologio, _, acceso) = bottone(&f, Cmd::Timeline);
    let (imbuto, _, _) = bottone(&f, Cmd::TimeFilter);
    assert!(
        !acceso && orologio.max.x <= imbuto.min.x + 0.5,
        "{orologio:?} {imbuto:?}"
    );
    // un clic sull'orologio accende la timeline e il pulsante si accende; un altro la spegne
    clic(&mut f, &ctx, orologio.center());
    assert!(f.app().timeline_on());
    assert!(bottone(&f, Cmd::Timeline).2);
    clic(&mut f, &ctx, orologio.center());
    assert!(!f.app().timeline_on());
    assert!(!bottone(&f, Cmd::Timeline).2);
    // l'imbuto non tocca la timeline: senza date ricordate apre la finestra del filtro
    let (imbuto, _, _) = bottone(&f, Cmd::TimeFilter);
    clic(&mut f, &ctx, imbuto.center());
    assert!(!f.app().timeline_on());
    assert!(f.app().dialog_kind().is_some());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_time_window_has_a_timeline_box_next_to_the_filter_box() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    // la casella Timeline sta accanto a "Filtro attivo", spenta finche' la timeline e' spenta
    let filtro = f
        .dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == "filtro")
        .unwrap()
        .0;
    let barra = f
        .dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == "timeline")
        .unwrap()
        .0;
    assert!(barra.min.x > filtro.max.x && (barra.center().y - filtro.center().y).abs() < 3.0);
    assert!(!f.app().time_timeline_check());
    // spuntarla e dare OK accende la timeline (il filtro resta spento: sono due cose)
    clic(&mut f, &ctx, barra.center());
    assert!(f.app().time_timeline_check());
    assert!(!f.app().timeline_on(), "si applica con OK");
    let ok = f
        .dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == "OK")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, ok);
    assert!(f.app().timeline_on());
    assert!(!f.app().time_filter_on());
    // riaperta, la casella e' spuntata; toglierla e OK spegne la timeline
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    assert!(f.app().time_timeline_check());
    let barra = f
        .dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == "timeline")
        .unwrap()
        .0;
    clic(&mut f, &ctx, barra.center());
    let ok = f
        .dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == "OK")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, ok);
    assert!(!f.app().timeline_on());
    // Annulla non cambia niente
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    let barra = f
        .dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == "timeline")
        .unwrap()
        .0;
    clic(&mut f, &ctx, barra.center());
    let annulla = f
        .dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == "Cancel")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, annulla);
    assert!(!f.app().timeline_on());
    let _ = std::fs::remove_file(p);
}
