//! La navigazione temporale nella finestra: le voci del menu Log, il pulsante e l'orario nella barra, e la finestra "Tempo" (formato
//! e filtro per data insieme: casella, Dal / Al, calendario, X). Scritti da ARCH.

use eframe::egui::{self, Context, Event, FullOutput, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_gui::menu::MENU;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::commands::{self, Cmd};
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

fn finestra() -> (Finestra, Context, PathBuf, Vec<u64>) {
    let mut dati = String::new();
    let mut offset = Vec::new();
    for i in 0..100u32 {
        offset.push(dati.len() as u64);
        dati.push_str(&format!(
            "2026-11-01 {:02}:{:02}:00 [INFO ] riga {i}\n",
            2 + i / 60,
            i % 60
        ));
    }
    offset.push(dati.len() as u64);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tgui_{}_{}.log", std::process::id(), n));
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

/// Clicca la tendina dei formati del dialogo (registrata fra le opzioni) e ne ritorna il rettangolo.
fn apri_tendina(f: &mut Finestra, ctx: &Context) -> egui::Rect {
    let r = f
        .dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == "formato")
        .expect("manca la tendina dei formati")
        .0;
    clic(f, ctx, r.center());
    r
}

/// Il centro dell'opzione del dialogo con quell'etichetta (la tendina, la casella, il calendario, la X, un giorno).
fn opzione(f: &Finestra, etichetta: &str) -> Pos2 {
    f.dialogo_opzioni()
        .iter()
        .find(|(_, t)| t == etichetta)
        .unwrap_or_else(|| panic!("manca {etichetta:?} in {:?}", f.dialogo_opzioni()))
        .0
        .center()
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

#[test]
fn the_log_menu_has_the_time_filter_and_one_entry_for_the_format_and_filter_window() {
    let log: Vec<Cmd> = MENU[3].1.iter().flatten().copied().collect();
    for c in [Cmd::TimeFilter, Cmd::TimeRange] {
        assert!(log.contains(&c), "{c:?} nel menu Log");
    }
    // il formato e il filtro sono la stessa finestra: una voce sola (il formato resta su Alt+O e sul clic di FORMAT)
    assert!(!log.contains(&Cmd::TimeMask));
    // il filtro per data e' un interruttore: sempre disponibile, acceso quando c'e' una finestra di tempo
    let (mut f, ctx, p, _) = finestra();
    assert!(commands::enabled(f.workspace(), Cmd::TimeFilter));
    assert_eq!(
        mrp_gui::menu::acceso(f.workspace(), Cmd::TimeFilter),
        Some(false)
    );
    f.workspace()
        .active_mut()
        .apply_time_range("02:10", "02:20")
        .unwrap();
    assert_eq!(
        mrp_gui::menu::acceso(f.workspace(), Cmd::TimeFilter),
        Some(true)
    );
    // un clic lo spegne (le date restano), un altro lo riaccende con le stesse date
    f.azione(Cmd::TimeFilter);
    disegna(&mut f, &ctx);
    assert_eq!(f.app().time_window(), None);
    f.azione(Cmd::TimeFilter);
    disegna(&mut f, &ctx);
    assert!(f.app().time_window().is_some());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_status_bar_time_and_the_format_label_open_the_time_window() {
    let (mut f, ctx, p, _) = finestra();
    let testi = f.stato_disegnato().expect("barra di stato").clone();
    // l'orario della prima riga: un clic apre la finestra "Tempo"
    let pezzo = testi
        .destra
        .iter()
        .position(|z| z.testo == "2026-11-01 02:00:00")
        .expect("manca l'orario nella barra di stato");
    assert_eq!(testi.destra[pezzo].comando, Some(Cmd::TimeRange));
    let (r, _) = f.pezzi_stato()[pezzo];
    clic(&mut f, &ctx, r.center());
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Time));
    assert_eq!(f.dialogo_titolo(), Some("TIME"));
    f.app().dialog_cancel();
    disegna(&mut f, &ctx);
    // e anche il formato: la barra lo SCRIVE (non e' solo un menu), un clic apre la finestra
    let testi = f.stato_disegnato().expect("barra di stato").clone();
    let i = testi
        .destra
        .iter()
        .position(|z| z.comando == Some(Cmd::TimeMask))
        .expect("manca il pezzo del formato");
    assert_eq!(testi.destra[i].testo, "yyyy-MM-dd HH:mm:ss");
    let (r, _) = f.pezzi_stato()[i];
    clic(&mut f, &ctx, r.center());
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Time));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_toolbar_has_the_time_filter_button_lit_when_the_filter_is_on() {
    let (mut f, ctx, p, _) = finestra();
    let pulsante_filtro = |f: &Finestra| {
        f.pulsanti()
            .iter()
            .find(|b| b.cmd == Cmd::TimeFilter)
            .map(|b| (b.rect, b.attivo, b.acceso))
            .expect("manca il pulsante del filtro per data")
    };
    let (rect, attivo, acceso) = pulsante_filtro(&f);
    assert!(attivo && !acceso);
    // senza date ricordate il primo clic apre la finestra
    clic(&mut f, &ctx, rect.center());
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Time));
    f.app().dialog_cancel();
    disegna(&mut f, &ctx);
    // con un filtro acceso il pulsante e' acceso, e un clic lo spegne senza aprire niente
    f.workspace()
        .active_mut()
        .apply_time_range("02:10", "02:20")
        .unwrap();
    disegna(&mut f, &ctx);
    let (rect, _, acceso) = pulsante_filtro(&f);
    assert!(acceso);
    clic(&mut f, &ctx, rect.center());
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.app().time_window(), None);
    let (rect, _, acceso) = pulsante_filtro(&f);
    assert!(!acceso);
    // un altro clic lo riaccende con le stesse date
    clic(&mut f, &ctx, rect.center());
    assert!(f.app().time_window().is_some());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_time_window_applies_format_and_filter_with_ok() {
    let (mut f, ctx, p, offset) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("TIME"));
    assert_eq!(pulsanti(&f), vec!["OK", "Recognise it again", "Cancel"]);
    // due campi (Dal e Al), la casella spenta, niente filtro ancora
    assert_eq!(f.dialogo_campi().len(), 2);
    assert!(!f.app().time_check());
    // scrivere una data accende da sola la casella
    f.workspace().active_mut().dialog_set_field(2, "02:10");
    f.workspace().active_mut().dialog_set_field(3, "02:20");
    disegna(&mut f, &ctx);
    assert!(f.app().time_check());
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().dialog_kind(), None);
    // l'estremo finale e' compreso: "02:20" copre tutto il minuto 02:20, la finestra finisce alla riga delle 02:21
    assert_eq!(f.app().time_window(), Some((offset[10], offset[21])));
    // riaperta la finestra ha la casella accesa e le date
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    assert!(f.app().time_check());
    assert_eq!(f.app().time_field_text(2), "02:10");
    // togliere la spunta e OK: il file torna intero, le date restano scritte
    let casella = opzione(&f, "filtro");
    clic(&mut f, &ctx, casella);
    assert!(!f.app().time_check());
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().time_window(), None);
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    assert!(!f.app().time_check());
    assert_eq!(f.app().time_field_text(2), "02:10");
    assert_eq!(f.app().time_field_text(3), "02:20");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_bad_time_stays_in_the_window_with_the_error_and_cancel_closes_it() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    f.workspace().active_mut().dialog_set_field(2, "foo");
    disegna(&mut f, &ctx);
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Time));
    assert!(f.app().time_error_text().contains("foo"));
    let annulla = pulsante(&f, "Cancel");
    clic(&mut f, &ctx, annulla);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.app().time_window(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_x_empties_a_field_and_an_empty_field_means_no_limit() {
    let (mut f, ctx, p, offset) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    f.workspace().active_mut().dialog_set_field(2, "02:10");
    f.workspace().active_mut().dialog_set_field(3, "02:20");
    disegna(&mut f, &ctx);
    // la X del campo Dal lo svuota: il filtro parte dall'inizio del file
    let x = opzione(&f, "svuota 2");
    clic(&mut f, &ctx, x);
    assert_eq!(f.app().time_field_text(2), "");
    assert_eq!(f.app().time_field_text(3), "02:20");
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().time_window(), Some((0, offset[21])));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_calendar_picks_a_day_and_writes_it_in_the_format_of_the_file() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    // il calendario si apre sul mese del primo orario del file (novembre 2026)
    let cal = opzione(&f, "calendario 2");
    clic(&mut f, &ctx, cal);
    let giorno = opzione(&f, "giorno 5");
    clic(&mut f, &ctx, giorno);
    // Dal parte da mezzanotte, scritto come le righe del file; la casella si accende da sola
    assert_eq!(f.app().time_field_text(2), "2026-11-05 00:00:00");
    assert!(f.app().time_check());
    // Al arriva a fine giornata
    let cal = opzione(&f, "calendario 3");
    clic(&mut f, &ctx, cal);
    let giorno = opzione(&f, "giorno 7");
    clic(&mut f, &ctx, giorno);
    assert_eq!(f.app().time_field_text(3), "2026-11-07 23:59:59");
    // "<" e ">" cambiano mese
    let cal = opzione(&f, "calendario 2");
    clic(&mut f, &ctx, cal);
    let dopo = opzione(&f, "mese dopo");
    clic(&mut f, &ctx, dopo);
    let giorno = opzione(&f, "giorno 30");
    clic(&mut f, &ctx, giorno);
    assert_eq!(f.app().time_field_text(2), "2026-12-30 00:00:00");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_window_lists_the_times_of_the_file_and_other_opens_the_rule() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeMask);
    disegna(&mut f, &ctx);
    // l'elenco: il tempo in uso (l'orario all'inizio della riga) e "Altro"; con un tempo dell'elenco restano solo i campi Dal e Al
    opzione(&f, "fonte 0");
    opzione(&f, "altro");
    assert!(!f.dialogo_opzioni().iter().any(|(_, t)| t == "fonte 1"));
    assert!(!f.dialogo_opzioni().iter().any(|(_, t)| t == "formato"));
    assert_eq!(f.dialogo_campi().len(), 2);
    // "Altro" apre i formati noti in una tendina, il pattern, "entro quanti caratteri" e quale data della riga
    let altro = opzione(&f, "altro");
    clic(&mut f, &ctx, altro);
    assert_eq!(f.dialogo_campi().len(), 4);
    opzione(&f, "formato");
    opzione(&f, "occorrenza");
    // la tendina dei formati ha i 17 noti, quelli che leggono il file per primi
    assert!(f.dialogo_voci().is_empty());
    let tendina = apri_tendina(&mut f, &ctx);
    assert!(tendina.width() > 300.0, "{tendina:?}");
    assert_eq!(f.dialogo_voci().len(), 19);
    // un tempo dell'elenco toglie di nuovo i campi
    f.app().dialog_cancel();
    f.azione(Cmd::TimeMask);
    disegna(&mut f, &ctx);
    let altro = opzione(&f, "altro");
    clic(&mut f, &ctx, altro);
    let fonte = opzione(&f, "fonte 0");
    clic(&mut f, &ctx, fonte);
    assert_eq!(f.dialogo_campi().len(), 2);
    assert_eq!(f.app().time_field_text(0), "yyyy-MM-dd HH:mm:ss");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_known_format_chosen_under_other_becomes_the_standard_with_ok_and_a_written_rule_is_remembered()
{
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeMask);
    disegna(&mut f, &ctx);
    // aprire e dare OK senza toccare niente non fa diventare standard il formato
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().time_default(), None);
    assert!(f.app().time_rules().is_empty());
    f.azione(Cmd::TimeMask);
    disegna(&mut f, &ctx);
    // Altro, poi un formato noto: il terzo che legge il file e' la sola data; confermato, e' quello standard
    let altro = opzione(&f, "altro");
    clic(&mut f, &ctx, altro);
    apri_tendina(&mut f, &ctx);
    let data = f.dialogo_voci()[2].center();
    clic(&mut f, &ctx, data);
    assert_eq!(f.app().time_field_text(0), "yyyy-MM-dd");
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.app().time_default(), Some(("yyyy-MM-dd".to_string(), 0)));
    // un formato noto non e' una regola: niente da ricordare
    assert!(f.app().time_rules().is_empty());
    // una regola scritta a mano (qui un formato che legge le stesse righe, con un testo qualunque prima dell'ora)
    f.azione(Cmd::TimeMask);
    disegna(&mut f, &ctx);
    let altro = opzione(&f, "altro");
    clic(&mut f, &ctx, altro);
    f.app().dialog_set_field(0, "yyyy-MM-dd HH");
    disegna(&mut f, &ctx);
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(
        f.app().time_rules(),
        vec![("yyyy-MM-dd HH".to_string(), 0, 0)]
    );
    // la volta dopo e' una riga dell'elenco (il file la legge)
    f.azione(Cmd::TimeMask);
    disegna(&mut f, &ctx);
    let regola = f
        .app()
        .time_sources()
        .iter()
        .position(|s| s.pattern == "yyyy-MM-dd HH")
        .expect("la regola e' nell'elenco");
    opzione(&f, &format!("fonte {regola}"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn recognise_it_again_puts_the_detected_format_back() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeMask);
    disegna(&mut f, &ctx);
    f.workspace()
        .active_mut()
        .dialog_set_field(0, "dd/MM/yyyy HH:mm:ss");
    disegna(&mut f, &ctx);
    let auto = pulsante(&f, "Recognise it again");
    clic(&mut f, &ctx, auto);
    assert_eq!(f.app().time_field_text(0), "yyyy-MM-dd HH:mm:ss");
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Time));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_calendar_popup_has_month_and_year_combos_and_the_time_and_stays_open_until_done() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    // chiuso: niente giorni
    assert!(!f
        .dialogo_opzioni()
        .iter()
        .any(|(_, t)| t.starts_with("giorno")));
    let cal = opzione(&f, "calendario 2");
    clic(&mut f, &ctx, cal);
    // aperto: mese e anno in tendina, le frecce, i giorni, l'ora (secondi: la maschera del file scrive i secondi) e "Fatto"
    for e in [
        "mese combo",
        "anno combo",
        "mese prima",
        "mese dopo",
        "giorno 1",
        "giorno 30",
        "ora",
        "minuti",
        "secondi",
        "fatto",
    ] {
        opzione(&f, e);
    }
    assert!(
        !f.dialogo_opzioni().iter().any(|(_, t)| t == "millisecondi"),
        "la maschera del file non scrive i millisecondi"
    );
    // scegliere un giorno non chiude: l'ora si regola dopo
    let giorno = opzione(&f, "giorno 5");
    clic(&mut f, &ctx, giorno);
    opzione(&f, "giorno 5");
    // "Fatto" lo chiude
    let fatto = opzione(&f, "fatto");
    clic(&mut f, &ctx, fatto);
    assert!(!f
        .dialogo_opzioni()
        .iter()
        .any(|(_, t)| t.starts_with("giorno")));
    // il campo ha il giorno scelto
    assert_eq!(f.app().time_field_text(2), "2026-11-05 00:00:00");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_outside_the_calendar_closes_it() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    let cal = opzione(&f, "calendario 2");
    clic(&mut f, &ctx, cal);
    opzione(&f, "giorno 1");
    // un clic lontano dal popup e dal bottone (in un angolo della finestra)
    clic(&mut f, &ctx, Pos2::new(5.0, 790.0));
    assert!(!f
        .dialogo_opzioni()
        .iter()
        .any(|(_, t)| t.starts_with("giorno")));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_log_and_special_switches_say_how_they_are_and_what_a_click_does() {
    let (mut f, ctx, p, _) = finestra();
    ctx.style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    let testi = f.stato_disegnato().expect("barra di stato").clone();
    let i = testi
        .destra
        .iter()
        .position(|z| z.testo == "LOG")
        .expect("manca LOG");
    assert!(testi.destra[i].spento);
    let (r, _) = f.pezzi_stato()[i];
    for _ in 0..3 {
        giro(&mut f, &ctx, vec![Event::PointerMoved(r.center())]);
    }
    let o = giro(&mut f, &ctx, vec![Event::PointerMoved(r.center())]);
    fn visita(s: &egui::Shape, v: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => v.push(t.galley.text().to_string()),
            egui::Shape::Vec(tutte) => tutte.iter().for_each(|x| visita(x, v)),
            _ => {}
        }
    }
    let mut scritti = Vec::new();
    for c in &o.shapes {
        visita(&c.shape, &mut scritti);
    }
    assert!(
        scritti
            .iter()
            .any(|t| t == "Colour log lines: off (click to turn on)"),
        "{scritti:?}"
    );
    let _ = std::fs::remove_file(p);
}
