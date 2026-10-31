//! I test del JUN per la barra degli strumenti e la barra dei salti (B3): i casi che il file di ARCH (`strumenti.rs`)
//! non copre, con gli attesi calcolati a mano dal PLAN. Come li': le funzioni pure si provano da sole, la finestra con
//! un contesto di egui senza schermo, un giro alla volta con eventi finti.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_gui::strumenti::{glifo, salti, SaltoAzione};
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::Cmd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

// ---- le funzioni pure ----

#[test]
fn le_dieci_voci_dei_salti_restano_dieci_anche_con_i_moltiplicatori_uguali() {
    let v = salti([3, 3, 3]);
    let etichette: Vec<&str> = v.iter().map(|s| s.etichetta.as_str()).collect();
    assert_eq!(
        etichette,
        vec![
            glifo("home"),
            "-3",
            "-3",
            "-3",
            "-1",
            "+1",
            "+3",
            "+3",
            "+3",
            glifo("end")
        ]
    );
    // inizio e fine restano ai lati, qualunque siano i moltiplicatori
    assert_eq!(v.first().map(|s| s.azione), Some(SaltoAzione::Inizio));
    assert_eq!(v.last().map(|s| s.azione), Some(SaltoAzione::Fine));
}

// ---- la finestra ----

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!(
        "mrp_strumenti_jun_test_{}_{}.txt",
        std::process::id(),
        n
    ));
    std::fs::write(&p, dati).unwrap();
    p
}

fn cfg(chrome: bool) -> AppConfig {
    AppConfig {
        chrome,
        ..AppConfig::default()
    }
}

/// Una finestra su un file con `dati`; `chrome` dice se l'`App` ha la cornice del terminale.
fn finestra(dati: &[u8], chrome: bool) -> (Finestra, Context, PathBuf) {
    let p = file(dati);
    let mut app = App::open(&p, cfg(chrome)).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let f = Finestra::da_app(app, Box::new(move || cfg(chrome)));
    (f, Context::default(), p)
}

fn giro(f: &mut Finestra, ctx: &Context, eventi: Vec<Event>) -> FullOutput {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))),
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

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
}

fn pigia(pos: Pos2, premuto: bool) -> Event {
    Event::PointerButton {
        pos,
        button: eframe::egui::PointerButton::Primary,
        pressed: premuto,
        modifiers: Modifiers::NONE,
    }
}

fn clic(f: &mut Finestra, ctx: &Context, pos: Pos2) {
    giro(f, ctx, vec![Event::PointerMoved(pos), pigia(pos, true)]);
    giro(f, ctx, vec![pigia(pos, false)]);
    giro(f, ctx, vec![]);
}

/// Due giri: il primo ricorda la geometria, il secondo la usa.
fn disegna(f: &mut Finestra, ctx: &Context) {
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
}

/// Il centro del pulsante di `cmd` nella barra degli strumenti.
fn centro(f: &Finestra, cmd: Cmd) -> Pos2 {
    f.pulsanti()
        .iter()
        .find(|p| p.cmd == cmd)
        .unwrap_or_else(|| panic!("manca il pulsante di {cmd:?}"))
        .rect
        .center()
}

#[test]
fn il_pulsante_dei_salti_la_accende_e_la_spegne() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\n", false);
    disegna(&mut f, &ctx);
    assert!(f.salti_rect().is_none(), "la barra dei salti parte spenta");
    assert!(
        !f.pulsanti()
            .iter()
            .find(|p| p.cmd == Cmd::NavBar)
            .unwrap()
            .acceso,
        "il pulsante parte non premuto"
    );
    let pos = centro(&f, Cmd::NavBar);
    clic(&mut f, &ctx, pos);
    disegna(&mut f, &ctx);
    assert!(
        f.salti_rect().is_some(),
        "il clic sul pulsante accende la barra"
    );
    assert!(!f.pulsanti_salti().is_empty());
    assert!(
        f.pulsanti()
            .iter()
            .find(|p| p.cmd == Cmd::NavBar)
            .unwrap()
            .acceso,
        "il pulsante adesso e' premuto"
    );
    let pos = centro(&f, Cmd::NavBar);
    clic(&mut f, &ctx, pos);
    disegna(&mut f, &ctx);
    assert!(f.salti_rect().is_none(), "un altro clic la spegne");
    assert!(f.pulsanti_salti().is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn con_la_cornice_la_barra_a_celle_resta_anche_con_i_salti_accesi() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\n", true);
    disegna(&mut f, &ctx);
    // Ctrl+B accende i salti: ma con la cornice li disegna l'App nella sua griglia, a celle, non questa finestra
    giro(&mut f, &ctx, vec![tasto(Key::B, ctrl())]);
    disegna(&mut f, &ctx);
    assert!(f.strumenti_rect().is_none() && f.pulsanti().is_empty());
    assert!(f.salti_rect().is_none() && f.pulsanti_salti().is_empty());
    let righe = f.workspace().render_lines(80, 24);
    assert!(
        righe[0].starts_with("[Home]"),
        "la barra a celle c'e' ancora: {:?}",
        righe[0]
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn la_barra_dei_salti_sta_sotto_gli_strumenti_e_sopra_il_testo() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\n", false);
    disegna(&mut f, &ctx);
    giro(&mut f, &ctx, vec![tasto(Key::B, ctrl())]);
    disegna(&mut f, &ctx);
    let strumenti = f.strumenti_rect().unwrap();
    let strisce = f.salti_rect().unwrap();
    let stato = f.barra_stato_rect().unwrap();
    assert!(
        strisce.min.y >= strumenti.max.y - 0.5,
        "{strisce:?} sotto {strumenti:?}"
    );
    // il testo comincia sotto la striscia dei salti...
    let (o, _, _, _) = f.geometria().unwrap();
    assert!(o.y >= strisce.max.y - 0.5, "{o:?} sotto {strisce:?}");
    // ...che sta sopra la barra di stato
    assert!(
        strisce.max.y <= stato.min.y + 0.5,
        "{strisce:?} sopra {stato:?}"
    );
    let _ = std::fs::remove_file(p);
}
