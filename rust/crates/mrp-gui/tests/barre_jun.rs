//! I test del JUN per la barra di stato e la scrollbar (B2b): i casi che il file di ARCH (`barre.rs`) non copre,
//! con gli attesi calcolati a mano dal PLAN. Come li': le funzioni pure si provano da sole, la finestra con un
//! contesto di egui senza schermo, un giro alla volta con eventi finti.

use eframe::egui::{Context, Event, FullOutput, Pos2, RawInput, Rect, Vec2};
use mrp_core::lang::Catalog;
use mrp_gui::barre::{frazione_da_posizione, pollice, testi_stato, Pezzo};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig, Busy, ScrollInfo, StatusInfo};
use mrp_tui::commands::Cmd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

// ---- le funzioni pure ----

fn info() -> StatusInfo {
    StatusInfo {
        file_name: "b.txt".to_string(),
        size: 1024,
        edit: true,
        modified: true,
        column_mode: true,
        untitled: false,
        follow: false,
        line: Some(7),
        column: 2,
        offset: 512,
        percent: 50,
        encoding: "utf-16le".to_string(),
        line_ending: "CRLF",
        flash: None,
        busy: None,
    }
}

fn pezzo(testo: &str, comando: Option<Cmd>) -> Pezzo {
    Pezzo {
        testo: testo.to_string(),
        comando,
    }
}

#[test]
fn il_messaggio_e_il_lavoro_insieme_non_si_toccano() {
    let mut s = info();
    s.flash = Some("fatto".to_string());
    s.busy = Some((Busy::Searching, 50));
    let t = testi_stato(&s, &Catalog::english());
    // a sinistra solo il messaggio; a destra il lavoro in testa e tutto il resto al suo posto
    assert_eq!(t.sinistra, "fatto");
    assert_eq!(
        t.destra,
        vec![
            pezzo("searching 50%", None),
            pezzo("EDIT", Some(Cmd::EditMode)),
            pezzo("COL", Some(Cmd::ColumnMode)),
            pezzo("utf-16le", Some(Cmd::Encoding)),
            pezzo("CRLF", None),
            pezzo("1.00 KiB", None),
        ]
    );
}

#[test]
fn una_finestra_piu_grande_del_documento_riempie_la_pista() {
    // size 2.0 (difensivo): il pollice e' tutta la pista, attaccata in cima
    let i = ScrollInfo {
        start: 0.5,
        size: 2.0,
    };
    assert_eq!(pollice(i, 700.0, 16.0), (0.0, 700.0));
}

#[test]
fn la_presa_dice_dove_va_la_cima_della_finestra() {
    // presa zero: la cima del pollice va al puntatore; presa pari all'altezza della pista: resta in cima
    assert_eq!(frazione_da_posizione(100.0, 0.0, 100.0), 1.0);
    assert_eq!(frazione_da_posizione(100.0, 100.0, 100.0), 0.0);
    assert_eq!(frazione_da_posizione(0.0, 0.0, 100.0), 0.0);
}

// ---- la finestra ----

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!(
        "mrp_barre_jun_test_{}_{}.txt",
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

/// Due giri: il primo ricorda la geometria, il secondo la usa.
fn disegna(f: &mut Finestra, ctx: &Context) {
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
}

#[test]
fn le_barre_vanno_e_vengono_con_la_cornice() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\n", true);
    disegna(&mut f, &ctx);
    // con la cornice del terminale niente barre: le disegna l'App nella sua griglia
    assert!(f.barra_stato_rect().is_none() && f.scrollbar_rect().is_none());
    assert!(f.stato_disegnato().is_none());
    assert!(f.pezzi_stato().is_empty());
    // senza cornice ci sono, e la scrollbar sta sopra la barra di stato
    f.app().set_chrome(false);
    disegna(&mut f, &ctx);
    let stato = f.barra_stato_rect().unwrap();
    let barra = f.scrollbar_rect().unwrap();
    assert!(f.stato_disegnato().is_some() && !f.pezzi_stato().is_empty());
    assert!(barra.max.y <= stato.min.y + 0.5, "{barra:?} {stato:?}");
    // e con la cornice di nuovo, di sparizione non resta niente
    f.app().set_chrome(true);
    disegna(&mut f, &ctx);
    assert!(f.barra_stato_rect().is_none() && f.scrollbar_rect().is_none());
    assert!(f.stato_disegnato().is_none());
    assert!(f.pezzi_stato().is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn i_pezzi_di_destra_della_finestra_vengono_dallo_stato() {
    let (mut f, ctx, p) = finestra(b"abc\n", false);
    disegna(&mut f, &ctx);
    // il modo in lettura, la codifica col suo nome canonico, l'a-capo e la dimensione
    assert_eq!(
        f.stato_disegnato().unwrap().destra,
        vec![
            pezzo("VIEW", Some(Cmd::EditMode)),
            pezzo("utf-8", Some(Cmd::Encoding)),
            pezzo("LF", None),
            pezzo("4 B", None),
        ]
    );
    let _ = std::fs::remove_file(p);
}
