//! La navigazione temporale nella finestra (F1): le voci del menu Log, l'orario della prima riga nella barra di stato (un clic apre
//! l'intervallo) e i due dialoghi (intervallo di tempo, formato dell'orario) con i loro pulsanti. Scritti da ARCH.

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
fn the_log_menu_has_the_time_range_the_format_and_the_way_to_clear_it() {
    let log: Vec<Cmd> = MENU[3].1.iter().flatten().copied().collect();
    for c in [Cmd::TimeRange, Cmd::TimeMask, Cmd::TimeOff] {
        assert!(log.contains(&c), "{c:?} nel menu Log");
    }
    // Togli intervallo e' spento finche' non c'e' una finestra di tempo
    let (mut f, ctx, p, _) = finestra();
    assert!(!commands::enabled(f.workspace(), Cmd::TimeOff));
    f.workspace()
        .active_mut()
        .apply_time_range("02:10", "02:20")
        .unwrap();
    assert!(commands::enabled(f.workspace(), Cmd::TimeOff));
    f.azione(Cmd::TimeOff);
    disegna(&mut f, &ctx);
    assert_eq!(f.app().time_window(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_status_bar_shows_the_time_of_the_top_row_and_a_click_opens_the_range() {
    let (mut f, ctx, p, _) = finestra();
    let testi = f.stato_disegnato().expect("barra di stato").clone();
    let pezzo = testi
        .destra
        .iter()
        .position(|z| z.testo == "2026-11-01 02:00:00")
        .expect("manca l'orario nella barra di stato");
    assert_eq!(testi.destra[pezzo].comando, Some(Cmd::TimeRange));
    let (r, _) = f.pezzi_stato()[pezzo];
    clic(&mut f, &ctx, r.center());
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::TimeRange));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_range_dialog_applies_with_ok_and_offers_to_remove_the_range() {
    let (mut f, ctx, p, offset) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("TIME RANGE"));
    assert_eq!(f.dialogo_campi().len(), 2);
    assert_eq!(pulsanti(&f), vec!["OK", "Cancel"]);
    f.workspace().active_mut().dialog_set_field(0, "02:10");
    f.workspace().active_mut().dialog_set_field(1, "02:20");
    disegna(&mut f, &ctx);
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.app().time_window(), Some((offset[10], offset[20])));
    // con una finestra il dialogo ha il pulsante che la toglie, in mezzo
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    assert_eq!(pulsanti(&f), vec!["OK", "Remove range", "Cancel"]);
    let togli = pulsante(&f, "Remove range");
    clic(&mut f, &ctx, togli);
    assert_eq!(f.app().time_window(), None);
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_bad_time_in_the_window_dialog_stays_open_and_cancel_closes_it() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeRange);
    disegna(&mut f, &ctx);
    f.workspace().active_mut().dialog_set_field(0, "foo");
    disegna(&mut f, &ctx);
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::TimeRange));
    assert!(f.app().time_range_error().contains("foo"));
    let annulla = pulsante(&f, "Cancel");
    clic(&mut f, &ctx, annulla);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.app().time_window(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_format_dialog_has_a_recognise_again_button() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeMask);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("TIME FORMAT"));
    assert_eq!(pulsanti(&f), vec!["OK", "Recognise it again", "Cancel"]);
    f.workspace()
        .active_mut()
        .dialog_set_field(0, "dd/MM/yyyy HH:mm:ss");
    disegna(&mut f, &ctx);
    let auto = pulsante(&f, "Recognise it again");
    clic(&mut f, &ctx, auto);
    assert_eq!(f.app().time_mask_text(0), "yyyy-MM-dd HH:mm:ss");
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::TimeMask));
    // OK la applica e chiude
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(
        f.workspace().active_mut().time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss")
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_status_bar_has_the_format_label_that_opens_the_known_formats() {
    let (mut f, ctx, p, _) = finestra();
    let testi = f.stato_disegnato().expect("barra di stato").clone();
    let i = testi
        .destra
        .iter()
        .position(|z| z.testo == "FORMAT")
        .expect("manca la scritta FORMAT");
    assert_eq!(testi.destra[i].comando, Some(Cmd::TimeMask));
    let (r, _) = f.pezzi_stato()[i];
    // passandoci sopra si legge il formato che c'e' adesso
    ctx.style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    for _ in 0..3 {
        giro(&mut f, &ctx, vec![Event::PointerMoved(r.center())]);
    }
    let o = giro(&mut f, &ctx, vec![Event::PointerMoved(r.center())]);
    let testo_disegnato = {
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
    };
    assert!(
        testo_disegnato.iter().any(|t| t == "yyyy-MM-dd HH:mm:ss"),
        "{testo_disegnato:?}"
    );
    // un clic apre il dialogo, con l'elenco dei formati noti
    clic(&mut f, &ctx, r.center());
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::TimeMask));
    disegna(&mut f, &ctx);
    // le voci dell'elenco (17 formati) sono rettangoli registrati nel dialogo
    assert_eq!(f.dialogo_voci().len(), 17);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_on_a_known_format_fills_the_fields_and_ok_applies_it() {
    let (mut f, ctx, p, _) = finestra();
    f.azione(Cmd::TimeMask);
    disegna(&mut f, &ctx);
    // il formato "HH:mm:ss" (indice 13) e' piu' in basso nell'elenco che scorre: si sceglie con l'azione e si controlla la GUI
    f.workspace().active_mut().dialog_time_preset(0);
    disegna(&mut f, &ctx);
    // il primo formato e' in vista: un clic sul suo rettangolo lo sceglie
    let primo = f.dialogo_voci()[1].center();
    clic(&mut f, &ctx, primo);
    assert_eq!(f.app().time_mask_text(0), "yyyy-MM-dd HH:mm:ss.SSS");
    let ok = pulsante(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(
        f.app().time_default(),
        Some(("yyyy-MM-dd HH:mm:ss.SSS".to_string(), 0))
    );
    let _ = std::fs::remove_file(p);
}
