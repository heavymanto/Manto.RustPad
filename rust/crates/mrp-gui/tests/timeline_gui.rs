//! La barra della timeline nella finestra (F2): spenta di fabbrica, si accende dal menu Log, sta a sinistra della scrollbar ed e' larga,
//! disegna una riga per giorno con le date, un clic salta al punto, il menu del passo cambia i bucket. Scritti da ARCH: tre giorni di
//! righe una ogni mezz'ora (144 righe), gli offset si calcolano a mano.

use eframe::egui::{self, Context, Event, FullOutput, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_gui::menu::{self, MENU};
use mrp_tui::app::timeline::{TimelinePref, TimelineStatus};
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::Cmd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cfg() -> AppConfig {
    AppConfig {
        chrome: false,
        timeline: TimelinePref {
            no_autostart: true,
            ..TimelinePref::default()
        },
        ..AppConfig::default()
    }
}

/// Come `cfg`, ma con la partenza da sola di fabbrica.
fn cfg_di_fabbrica() -> AppConfig {
    AppConfig {
        chrome: false,
        ..AppConfig::default()
    }
}

fn finestra() -> (Finestra, Context, PathBuf, Vec<u64>) {
    finestra_con(cfg())
}

fn finestra_con(configurazione: AppConfig) -> (Finestra, Context, PathBuf, Vec<u64>) {
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
    let mut app = App::open(&p, configurazione.clone()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(move || configurazione.clone()));
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
fn the_bar_is_wide_and_sits_on_the_left_of_the_text_by_default() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    pronta(&mut f, &ctx);
    let barra = f.timeline_disegnata().rect.expect("la barra");
    let scroll = f.scrollbar_rect().expect("la scrollbar");
    assert!(
        barra.width() >= 90.0,
        "larga abbastanza da leggere le date: {barra:?}"
    );
    // a sinistra: attaccata al bordo della finestra, e il testo parte dopo di lei
    assert!(barra.min.x < 1.0, "{barra:?}");
    assert!(barra.max.x < scroll.min.x - 200.0, "{barra:?} {scroll:?}");
    // la barra e la scrollbar sono alte uguali (sotto il menu, sopra la barra di stato)
    assert!((barra.min.y - scroll.min.y).abs() < 1.0 && (barra.max.y - scroll.max.y).abs() < 1.0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn with_the_right_flag_the_bar_sits_next_to_the_scrollbar() {
    let (mut f, ctx, p, _) = finestra();
    f.app().set_timeline_right(true);
    f.azione(Cmd::Timeline);
    pronta(&mut f, &ctx);
    let barra = f.timeline_disegnata().rect.expect("la barra");
    let scroll = f.scrollbar_rect().expect("la scrollbar");
    assert!(
        barra.max.x <= scroll.min.x + 0.5 && barra.min.x < scroll.min.x,
        "{barra:?} {scroll:?}"
    );
    assert!(barra.min.x > 500.0, "a destra, non a sinistra: {barra:?}");
    // e si cambia di nuovo: torna a sinistra
    f.app().set_timeline_right(false);
    disegna(&mut f, &ctx);
    assert!(f.timeline_disegnata().rect.expect("la barra").min.x < 1.0);
    let _ = std::fs::remove_file(p);
}

/// Il centro del primo testo disegnato che contiene `cercato` (dove cliccare su una voce di menu).
fn centro_del_testo(o: &FullOutput, cercato: &str) -> Option<Pos2> {
    fn visita(s: &egui::Shape, cercato: &str) -> Option<Pos2> {
        match s {
            egui::Shape::Text(t) if t.galley.text().contains(cercato) => {
                Some(t.pos + t.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(tutte) => tutte.iter().find_map(|x| visita(x, cercato)),
            _ => None,
        }
    }
    o.shapes.iter().find_map(|c| visita(&c.shape, cercato))
}

/// Un clic con il tasto `tasto` nel punto `pos`.
fn clic_con(f: &mut Finestra, ctx: &Context, pos: Pos2, tasto: egui::PointerButton) {
    for premuto in [true, false] {
        giro(
            f,
            ctx,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: tasto,
                    pressed: premuto,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}

/// Apre il menu del tasto destro sulla barra e clicca la voce che contiene `voce`.
fn scegli_dal_menu_della_barra(f: &mut Finestra, ctx: &Context, voce: &str) {
    let corpo = f.timeline_disegnata().corpo.expect("il corpo");
    clic_con(f, ctx, corpo.center(), egui::PointerButton::Secondary);
    let o = giro(f, ctx, vec![]);
    let dove = centro_del_testo(&o, voce)
        .unwrap_or_else(|| panic!("la voce {voce:?} non c'e' nel menu: {:?}", testi(&o)));
    clic_con(f, ctx, dove, egui::PointerButton::Primary);
    disegna(f, ctx);
}

#[test]
fn the_right_click_menu_of_the_bar_moves_it_to_the_right_and_back() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    pronta(&mut f, &ctx);
    assert!(!f.app().timeline_right());
    scegli_dal_menu_della_barra(&mut f, &ctx, "Timeline on the right");
    assert!(f.app().timeline_right(), "la voce ha spostato la barra");
    assert!(f.timeline_disegnata().rect.expect("la barra").min.x > 500.0);
    // la stessa voce la riporta a sinistra
    scegli_dal_menu_della_barra(&mut f, &ctx, "Timeline on the right");
    assert!(!f.app().timeline_right());
    assert!(f.timeline_disegnata().rect.expect("la barra").min.x < 1.0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_right_click_menu_of_the_bar_switches_the_start_by_itself_on_and_off() {
    let (mut f, ctx, p, _) = finestra_con(cfg_di_fabbrica());
    pronta(&mut f, &ctx);
    assert!(f.app().timeline_autostart());
    scegli_dal_menu_della_barra(&mut f, &ctx, "Start by itself");
    assert!(!f.app().timeline_autostart(), "la voce l'ha spenta");
    assert!(f.app().timeline_pref().no_autostart);
    scegli_dal_menu_della_barra(&mut f, &ctx, "Start by itself");
    assert!(f.app().timeline_autostart(), "e la riaccende");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_file_with_dates_starts_the_timeline_by_itself() {
    let (mut f, ctx, p, _) = finestra_con(cfg_di_fabbrica());
    pronta(&mut f, &ctx);
    assert!(f.timeline_disegnata().rect.is_some());
    assert!(f.app().timeline_on());
    // e' l'apertura che l'ha accesa, non una scelta: la preferenza non cambia
    assert!(!f.app().timeline_pref().on);
    // l'utente la spegne: resta spenta
    f.azione(Cmd::Timeline);
    disegna(&mut f, &ctx);
    disegna(&mut f, &ctx);
    assert!(f.timeline_disegnata().rect.is_none());
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_file_without_dates_does_not_start_it() {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlgui_nd_{}_{}.log", std::process::id(), n));
    std::fs::write(
        &p,
        "niente di niente
solo parole
nessuna data qui
",
    )
    .unwrap();
    let mut app = App::open(&p, cfg_di_fabbrica()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg_di_fabbrica));
    let ctx = Context::default();
    disegna(&mut f, &ctx);
    disegna(&mut f, &ctx);
    assert!(f.timeline_disegnata().rect.is_none());
    assert!(!f.app().timeline_on());
    let _ = std::fs::remove_file(p);
}

#[test]
fn with_the_switch_off_a_file_with_dates_does_not_start_it() {
    let (mut f, ctx, p, _) = finestra_con(cfg());
    assert!(!f.app().timeline_autostart());
    disegna(&mut f, &ctx);
    disegna(&mut f, &ctx);
    assert!(f.timeline_disegnata().rect.is_none());
    assert!(!f.app().timeline_on());
    // si riaccende: vale per i file che si apriranno, e si ricorda
    f.app().set_timeline_autostart(true);
    assert!(f.app().timeline_autostart());
    assert!(!f.app().timeline_pref().no_autostart);
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

#[test]
fn choosing_a_level_opens_the_filtered_view_by_itself_after_a_moment() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    f.workspace()
        .active_mut()
        .set_timeline_step(mrp_core::timeline::Step::Day);
    pronta(&mut f, &ctx);
    assert_eq!(f.workspace().len(), 1);
    // un clic su I (INFO): non si apre subito (si potrebbe voler sceglierne un altro)...
    let i = f
        .timeline_disegnata()
        .chip
        .iter()
        .find(|(_, t)| t == "INFO")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, i);
    assert_eq!(
        f.app().timeline_filter_levels(),
        [false, false, true, false]
    );
    assert_eq!(f.workspace().len(), 1);
    // ...ma dopo un attimo la scheda filtrata c'e', col nome dei livelli
    std::thread::sleep(mrp_gui::timeline::ATTESA_LIVELLI + Duration::from_millis(100));
    let fine = Instant::now() + Duration::from_secs(20);
    while f.workspace().len() == 1 && Instant::now() < fine {
        disegna(&mut f, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(f.workspace().len(), 2, "la vista filtrata");
    assert_eq!(f.app().file_name(), "[filter] levels: INFO");
    let _ = std::fs::remove_file(p);
}

#[test]
fn clicking_the_same_level_again_in_the_filtered_view_goes_back_to_the_file() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    f.workspace()
        .active_mut()
        .set_timeline_step(mrp_core::timeline::Step::Day);
    pronta(&mut f, &ctx);
    let info = |f: &Finestra| {
        f.timeline_disegnata()
            .chip
            .iter()
            .find(|(_, t)| t == "INFO")
            .unwrap()
            .0
            .center()
    };
    let i = info(&f);
    clic(&mut f, &ctx, i);
    std::thread::sleep(mrp_gui::timeline::ATTESA_LIVELLI + Duration::from_millis(100));
    let fine = Instant::now() + Duration::from_secs(20);
    while f.workspace().len() == 1 && Instant::now() < fine {
        disegna(&mut f, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(f.workspace().len(), 2);
    assert!(f.app().is_filter_view());
    // la scheda di partenza non resta con INFO acceso: mostra tutto
    assert_eq!(f.workspace().tabs()[0].timeline_filter_levels(), [false; 4]);
    pronta(&mut f, &ctx);
    // secondo clic su I nella vista filtrata: toglie il livello e torna al file vero
    let i = info(&f);
    clic(&mut f, &ctx, i);
    disegna(&mut f, &ctx);
    assert_eq!(
        f.workspace().active_index(),
        0,
        "torna alla scheda del file"
    );
    assert!(!f.app().is_filter_view());
    assert_eq!(f.workspace().len(), 2, "la vista resta aperta");
    let _ = std::fs::remove_file(p);
}

#[test]
fn choosing_two_levels_in_a_row_opens_one_view_and_the_funnel_does_not_wait() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    pronta(&mut f, &ctx);
    let chip = |f: &Finestra, nome: &str| {
        f.timeline_disegnata()
            .chip
            .iter()
            .find(|(_, t)| t == nome)
            .unwrap()
            .0
            .center()
    };
    // INFO e DEBUG uno dopo l'altro (meno di un attimo): una vista sola, coi due livelli
    let i = chip(&f, "INFO");
    clic(&mut f, &ctx, i);
    let d = chip(&f, "DEBUG");
    clic(&mut f, &ctx, d);
    assert_eq!(f.workspace().len(), 1);
    std::thread::sleep(mrp_gui::timeline::ATTESA_LIVELLI + Duration::from_millis(100));
    let fine = Instant::now() + Duration::from_secs(20);
    while f.workspace().len() == 1 && Instant::now() < fine {
        disegna(&mut f, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(f.workspace().len(), 2);
    assert_eq!(f.app().file_name(), "[filter] levels: INFO, DEBUG");
    // l'imbuto apre subito (qui senza livelli scelti non apre niente e lo dice, come prima)
    f.workspace().select(0);
    disegna(&mut f, &ctx);
    f.workspace()
        .active_mut()
        .set_timeline_filter_level(2, false);
    f.workspace()
        .active_mut()
        .set_timeline_filter_level(3, false);
    disegna(&mut f, &ctx);
    let filtra = chip(&f, "filtra");
    clic(&mut f, &ctx, filtra);
    assert_eq!(
        f.app().flash(),
        Some("Choose at least one level first (E, W, I, D)")
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_filtered_view_keeps_the_timeline_on_with_the_chosen_levels() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::Timeline);
    f.workspace()
        .active_mut()
        .set_timeline_step(mrp_core::timeline::Step::Day);
    pronta(&mut f, &ctx);
    // un livello scelto: apre la vista filtrata da sola
    let i = f
        .timeline_disegnata()
        .chip
        .iter()
        .find(|(_, t)| t == "INFO")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, i);
    std::thread::sleep(mrp_gui::timeline::ATTESA_LIVELLI + Duration::from_millis(100));
    let fine = Instant::now() + Duration::from_secs(20);
    while f.workspace().len() == 1 && Instant::now() < fine {
        disegna(&mut f, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(f.workspace().len(), 2);
    // la scheda filtrata e' quella attiva e la barra c'e' ancora (non si chiude), con il livello gia' scelto
    pronta(&mut f, &ctx);
    assert!(f.app().timeline_on(), "la timeline resta accesa");
    assert!(f.timeline_disegnata().rect.is_some());
    assert_eq!(
        f.app().timeline_filter_levels(),
        [false, false, true, false]
    );
    // ha le sue tacche (con l'orario ereditato) e non e' vuota
    assert!(
        f.timeline_disegnata().righe > 0,
        "{:?}",
        f.timeline_disegnata()
    );
    // la scheda di partenza ha ancora la sua timeline
    f.workspace().select(0);
    disegna(&mut f, &ctx);
    assert!(f.app().timeline_on());
    let _ = std::fs::remove_file(p);
}
