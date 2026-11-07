//! Le finestrelle dei dialoghi: tutte hanno la X per chiudere (che vale Annulla), e i pulsanti OK / Annulla sono grandi e centrati
//! in orizzontale nella finestra. Scritti da ARCH: i numeri sono quelli del PLAN (X in alto a destra; pulsanti di almeno 100 x 30
//! punti, il gruppo centrato sulla finestra con tolleranza di 3 punti).

use eframe::egui::{self, Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
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
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_finestrelle_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, b"uno\ndue\n").unwrap();
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
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

fn tasto(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
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

/// Tutti i modi di aprire un dialogo con una finestra: (nome, come si apre).
type Apertura = Box<dyn Fn(&mut Finestra, &Context)>;

fn aperture() -> Vec<(&'static str, Apertura)> {
    vec![
        (
            "aiuto",
            Box::new(|f, ctx| {
                giro(f, ctx, vec![tasto(Key::F1, Modifiers::NONE)]);
            }),
        ),
        (
            "temi",
            Box::new(|f, ctx| {
                giro(f, ctx, vec![tasto(Key::F6, Modifiers::NONE)]);
            }),
        ),
        (
            "lingue",
            Box::new(|f, ctx| {
                giro(f, ctx, vec![tasto(Key::F7, Modifiers::NONE)]);
            }),
        ),
        (
            "codifiche",
            Box::new(|f, ctx| {
                giro(f, ctx, vec![tasto(Key::F9, Modifiers::NONE)]);
            }),
        ),
        (
            "vai a",
            Box::new(|f, ctx| {
                giro(f, ctx, vec![tasto(Key::G, Modifiers::CTRL)]);
            }),
        ),
        (
            "trova",
            Box::new(|f, ctx| {
                giro(f, ctx, vec![tasto(Key::F3, Modifiers::NONE)]);
            }),
        ),
        (
            "sostituisci",
            Box::new(|f, ctx| {
                giro(f, ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
            }),
        ),
        (
            "filtro",
            Box::new(|f, ctx| {
                giro(f, ctx, vec![tasto(Key::G, Modifiers::ALT)]);
            }),
        ),
        (
            "salti",
            Box::new(|f, ctx| {
                f.app().open_jump_dialog();
                giro(f, ctx, vec![]);
            }),
        ),
        (
            "intervallo di tempo",
            Box::new(|f, ctx| {
                f.azione(Cmd::TimeRange);
                giro(f, ctx, vec![]);
            }),
        ),
        (
            "formato dell'orario",
            Box::new(|f, ctx| {
                f.azione(Cmd::TimeMask);
                giro(f, ctx, vec![]);
            }),
        ),
        (
            "codifica del salvataggio",
            Box::new(|f, ctx| {
                f.app()
                    .save_as_choose(std::env::temp_dir().join("mrp_finestrelle_dest.txt"));
                giro(f, ctx, vec![]);
            }),
        ),
        (
            "conferma",
            Box::new(|f, ctx| {
                // una modifica non salvata e poi chiudere la scheda: chiede cosa fare
                giro(f, ctx, vec![tasto(Key::E, Modifiers::ALT)]);
                giro(f, ctx, vec![Event::Text("x".into())]);
                f.azione(Cmd::CloseTab);
                giro(f, ctx, vec![]);
            }),
        ),
    ]
}

#[test]
fn every_dialog_window_has_a_close_button_in_the_top_right_corner() {
    for (nome, apri) in aperture() {
        let (mut f, ctx, p) = finestra();
        apri(&mut f, &ctx);
        disegna(&mut f, &ctx);
        let finestra = f
            .dialogo_rect()
            .unwrap_or_else(|| panic!("{nome}: nessuna finestra"));
        let x = f
            .dialogo_chiudi()
            .unwrap_or_else(|| panic!("{nome}: manca la X"));
        assert!(
            finestra.contains_rect(x),
            "{nome}: la X {x:?} sta dentro {finestra:?}"
        );
        assert!(
            x.max.x >= finestra.max.x - 24.0 && x.min.y <= finestra.min.y + 44.0,
            "{nome}: la X {x:?} e' in alto a destra di {finestra:?}"
        );
        assert!(x.width() >= 20.0 && x.height() >= 18.0, "{nome}: {x:?}");
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn the_close_button_closes_the_dialog_like_cancel() {
    for (nome, apri) in aperture() {
        let (mut f, ctx, p) = finestra();
        apri(&mut f, &ctx);
        disegna(&mut f, &ctx);
        assert!(f.app().dialog_kind().is_some(), "{nome}");
        let x = f.dialogo_chiudi().unwrap().center();
        clic(&mut f, &ctx, x);
        assert_eq!(f.app().dialog_kind(), None, "{nome}: la X non ha chiuso");
        assert!(f.dialogo_rect().is_none(), "{nome}");
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn the_x_of_the_exit_question_cancels_it_and_keeps_the_work() {
    let (mut f, ctx, p) = finestra();
    let apri = aperture()
        .into_iter()
        .find(|(n, _)| *n == "conferma")
        .unwrap()
        .1;
    apri(&mut f, &ctx);
    disegna(&mut f, &ctx);
    assert!(f.app().is_modified());
    let x = f.dialogo_chiudi().unwrap().center();
    clic(&mut f, &ctx, x);
    assert_eq!(f.workspace().len(), 1, "la scheda e' ancora li");
    assert!(f.app().is_modified(), "e le modifiche pure");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_buttons_are_big_and_centred_in_every_dialog_that_has_them() {
    for (nome, apri) in aperture() {
        let (mut f, ctx, p) = finestra();
        apri(&mut f, &ctx);
        disegna(&mut f, &ctx);
        let finestra = f.dialogo_rect().unwrap();
        let pulsanti: Vec<(Rect, String)> = f.dialogo_pulsanti().to_vec();
        if pulsanti.is_empty() {
            // i menu (temi, lingue, codifiche) si scelgono dalla lista: niente pulsanti
            let _ = std::fs::remove_file(p);
            continue;
        }
        for (r, testo) in &pulsanti {
            assert!(
                r.width() >= 100.0 && r.height() >= 30.0,
                "{nome}: il pulsante {testo:?} e' {:?} x {:?}",
                r.width(),
                r.height()
            );
        }
        // tutti sulla stessa riga e il gruppo centrato sulla finestra
        let y0 = pulsanti[0].0.center().y;
        assert!(
            pulsanti
                .iter()
                .all(|(r, _)| (r.center().y - y0).abs() < 2.0),
            "{nome}"
        );
        let sinistra = pulsanti
            .iter()
            .map(|(r, _)| r.min.x)
            .fold(f32::MAX, f32::min);
        let destra = pulsanti
            .iter()
            .map(|(r, _)| r.max.x)
            .fold(f32::MIN, f32::max);
        let centro_gruppo = (sinistra + destra) / 2.0;
        assert!(
            (centro_gruppo - finestra.center().x).abs() < 3.0,
            "{nome}: gruppo {sinistra}..{destra} nella finestra {finestra:?}"
        );
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn the_buttons_keep_their_order_and_still_work() {
    let (mut f, ctx, p) = finestra();
    giro(&mut f, &ctx, vec![tasto(Key::F3, Modifiers::NONE)]);
    disegna(&mut f, &ctx);
    let testi: Vec<String> = f
        .dialogo_pulsanti()
        .iter()
        .map(|(_, t)| t.clone())
        .collect();
    assert_eq!(testi, vec!["Find", "Cancel"]);
    // il primo e' a sinistra del secondo
    let v = f.dialogo_pulsanti();
    assert!(v[0].0.max.x < v[1].0.min.x);
    let annulla = v[1].0.center();
    clic(&mut f, &ctx, annulla);
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_settings_window_has_the_same_close_button() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::Settings);
    // la finestra si adatta al contenuto in qualche giro: la X sta all'orlo destro quando si e' assestata
    for _ in 0..3 {
        disegna(&mut f, &ctx);
    }
    let finestra = f.impostazioni_rect().unwrap();
    let x = f.impostazioni_chiudi().expect("manca la X");
    assert!(finestra.contains_rect(x));
    assert!(x.max.x >= finestra.max.x - 24.0 && x.min.y <= finestra.min.y + 44.0);
    clic(&mut f, &ctx, x.center());
    assert!(!f.impostazioni_aperta());
    let _ = std::fs::remove_file(p);
}

// ---- la scelta della codifica di "salva con nome" ----

#[test]
fn the_save_encoding_window_lists_the_encodings_and_a_click_selects_without_saving() {
    let (mut f, ctx, p) = finestra();
    let dest = std::env::temp_dir().join(format!("mrp_finestrelle_se_{}.txt", std::process::id()));
    f.app().save_as_choose(dest.clone());
    disegna(&mut f, &ctx);
    let tutte = mrp_core::encoding::Encoding::all();
    assert_eq!(f.dialogo_voci().len(), tutte.len());
    // il clic sulla voce di windows-1252 la evidenzia: il dialogo resta aperto e non si e' scritto niente
    let i = tutte
        .iter()
        .position(|e| e.name() == "windows-1252")
        .unwrap();
    // la lista scorre: si porta in vista la voce con la tastiera (frecce) invece di cliccare fuori dallo schermo
    f.app().dialog_select(i);
    disegna(&mut f, &ctx);
    assert_eq!(f.app().menu_index(), Some(i));
    assert_eq!(
        f.app().dialog_kind(),
        Some(mrp_tui::app::DialogKind::SaveEncoding)
    );
    assert!(!dest.exists());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_save_encoding_window_shows_the_bom_box_only_for_encodings_that_have_it() {
    let (mut f, ctx, p) = finestra();
    f.app()
        .save_as_choose(std::env::temp_dir().join("mrp_finestrelle_bom.txt"));
    disegna(&mut f, &ctx);
    // UTF-8: la casella c'e' (spenta: il file non ha il BOM) e un clic la accende
    assert_eq!(f.dialogo_opzioni().len(), 1);
    assert!(!f.app().save_encoding_bom());
    let casella = f.dialogo_opzioni()[0].0.center();
    clic(&mut f, &ctx, casella);
    assert!(f.app().save_encoding_bom());
    // una code page: la casella non c'e'
    let i = mrp_core::encoding::Encoding::all()
        .iter()
        .position(|e| e.name() == "windows-1252")
        .unwrap();
    f.app().dialog_select(i);
    disegna(&mut f, &ctx);
    assert!(f.dialogo_opzioni().is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_save_encoding_window_ok_converts_and_cancel_gives_up() {
    let (mut f, ctx, p) = finestra();
    let d = std::env::temp_dir().join(format!("mrp_finestrelle_ok_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let dest = d.join("nuovo.txt");
    f.app().save_as_choose(dest.clone());
    disegna(&mut f, &ctx);
    let i = mrp_core::encoding::Encoding::all()
        .iter()
        .position(|e| e.name() == "utf-16le")
        .unwrap();
    f.app().dialog_select(i);
    disegna(&mut f, &ctx);
    let testi: Vec<String> = f
        .dialogo_pulsanti()
        .iter()
        .map(|(_, t)| t.clone())
        .collect();
    assert_eq!(testi, vec!["Save", "Cancel"]);
    let ok = f.dialogo_pulsanti()[0].0.center();
    clic(&mut f, &ctx, ok);
    assert!(f.app().wait_idle(Duration::from_secs(20)));
    // il testo in UTF-16LE senza BOM (il file di partenza non lo ha e la casella e' spenta)
    let atteso: Vec<u8> = "uno\ndue\n"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    assert_eq!(std::fs::read(&dest).unwrap(), atteso);
    assert_eq!(f.app().encoding(), mrp_core::encoding::Encoding::Utf16Le);
    // Annulla: niente
    let altro = d.join("altro.txt");
    f.app().save_as_choose(altro.clone());
    disegna(&mut f, &ctx);
    let annulla = f.dialogo_pulsanti()[1].0.center();
    clic(&mut f, &ctx, annulla);
    assert_eq!(f.app().dialog_kind(), None);
    assert!(!altro.exists());
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_file(p);
}
