//! I test del JUN per le finestrelle dei dialoghi (G1): i casi che il file di ARCH (`finestrelle.rs`)
//! non copre, con gli attesi presi dal PLAN (pulsanti di almeno 120 x 34 distanti 12 punti, X di
//! almeno 28 x 24, la X non e' un pulsante del dialogo). Come gli altri test della finestra: un
//! contesto di egui senza schermo, un giro alla volta con eventi finti.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
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
    let p = std::env::temp_dir().join(format!(
        "mrp_finestrelle_jun_{}_{}.txt",
        std::process::id(),
        n
    ));
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

#[test]
fn la_x_della_testata_non_e_un_pulsante_del_dialogo() {
    let (mut f, ctx, p) = finestra();
    giro(&mut f, &ctx, vec![tasto(Key::F3, Modifiers::NONE)]);
    disegna(&mut f, &ctx);
    let x = f.dialogo_chiudi().unwrap();
    // il PLAN lo chiede esplicito: l'elenco dei pulsanti non deve contenere la X
    for (r, _) in f.dialogo_pulsanti() {
        assert!(!r.contains_rect(x), "la X {x:?} e' fra i pulsanti");
        assert!(!x.contains_rect(*r), "il pulsante {r:?} copre la X");
    }
    // e la X e' grande come dice il PLAN (almeno 28 x 24)
    assert!(x.width() >= 28.0 && x.height() >= 24.0, "{x:?}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn i_pulsanti_sono_almeno_120_per_34_e_distano_12() {
    let (mut f, ctx, p) = finestra();
    giro(&mut f, &ctx, vec![tasto(Key::F3, Modifiers::NONE)]);
    disegna(&mut f, &ctx);
    let pulsanti = f.dialogo_pulsanti().to_vec();
    assert_eq!(pulsanti.len(), 2);
    for (r, _) in &pulsanti {
        assert!(r.width() >= 120.0, "{r:?} stretto");
        assert!(r.height() >= 34.0, "{r:?} basso");
    }
    // la riga li fa tutti larghi uguali
    assert!(
        (pulsanti[0].0.width() - pulsanti[1].0.width()).abs() < 0.5,
        "{:?} {:?}",
        pulsanti[0].0,
        pulsanti[1].0
    );
    // e distano 12 punti, lo spazio fra gli oggetti della riga
    let spazio = pulsanti[1].0.min.x - pulsanti[0].0.max.x;
    assert!((spazio - 12.0).abs() < 1.0, "spazio {spazio}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn le_impostazioni_registrano_la_x_solo_da_aperte() {
    let (mut f, ctx, p) = finestra();
    assert!(f.impostazioni_chiudi().is_none());
    f.azione(Cmd::Settings);
    disegna(&mut f, &ctx);
    assert!(f.impostazioni_chiudi().is_some());
    // chiusa con Ctrl+, la X sparisce dallo stato del disegno
    giro(
        &mut f,
        &ctx,
        vec![tasto(
            Key::Comma,
            Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::NONE
            },
        )],
    );
    disegna(&mut f, &ctx);
    assert!(f.impostazioni_chiudi().is_none());
    let _ = std::fs::remove_file(p);
}
