//! Il mouse nella griglia della finestra senza cornice: il trascinamento arriva alla riga sotto il puntatore (non a quella dopo) e
//! Alt+trascinamento disegna un blocco a colonna (fase 2, 1.3). Il testo ha 4 righe di 7 byte ("abcdef\n", "ghijkl\n",
//! "mnopqr\n", "stuvwx\n"): il gutter e' di 3 celle (le cifre della dimensione del file piu' uno) e la prima riga di testo e' la riga 0.
//! Scritti da ARCH: gli attesi sono calcolati a mano.

use eframe::egui::{self, Context, Event, FullOutput, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_core::block::ColumnBlock;
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const G: u16 = 3;

fn finestra() -> (Finestra, Context, PathBuf) {
    let cfg = || AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_blocco_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, b"abcdef\nghijkl\nmnopqr\nstuvwx\n").unwrap();
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
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0))),
        events: eventi,
        ..Default::default()
    };
    ctx.run(raw, |ctx| f.giro(ctx))
}

/// Il centro della cella (colonna di testo, riga di testo): il gutter e la prima riga sono quelli di questo file.
fn centro(f: &Finestra, colonna: u16, riga: u16) -> Pos2 {
    let (o, c, _, _) = f.geometria().expect("un giro e' stato fatto");
    Pos2::new(
        o.x + c.x * (f32::from(G + colonna) + 0.5),
        o.y + c.y * (f32::from(riga) + 0.5),
    )
}

fn premi(pos: Pos2, premuto: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: premuto,
        modifiers,
    }
}

fn blocco(first_row: u64, last_row: u64, col_a: u64, col_b: u64) -> Option<ColumnBlock> {
    Some(ColumnBlock {
        first_row,
        last_row,
        col_a,
        col_b,
    })
}

#[test]
fn a_drag_selects_up_to_the_row_under_the_pointer() {
    let (mut f, ctx, p) = finestra();
    // dalla riga 0, colonna 1 (byte 1) alla riga 1, colonna 3 (byte 7 + 3)
    let (a, b) = (centro(&f, 1, 0), centro(&f, 3, 1));
    let ev = vec![
        premi(a, true, Modifiers::NONE),
        Event::PointerMoved(b),
        premi(b, false, Modifiers::NONE),
    ];
    giro(&mut f, &ctx, ev);
    assert_eq!(f.app().selection(), Some((1, 10)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn alt_drag_draws_a_column_block() {
    let (mut f, ctx, p) = finestra();
    let (a, b) = (centro(&f, 1, 0), centro(&f, 3, 2));
    let ev = vec![
        premi(a, true, Modifiers::ALT),
        Event::PointerMoved(b),
        premi(b, false, Modifiers::ALT),
    ];
    giro(&mut f, &ctx, ev);
    assert!(f.app().column_mode());
    assert_eq!(f.app().column_block(), blocco(0, 14, 1, 3));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_block_follows_the_pointer_in_separate_frames() {
    let (mut f, ctx, p) = finestra();
    let (a, b, c) = (centro(&f, 1, 1), centro(&f, 2, 3), centro(&f, 4, 0));
    giro(&mut f, &ctx, vec![premi(a, true, Modifiers::ALT)]);
    giro(&mut f, &ctx, vec![Event::PointerMoved(b)]);
    assert_eq!(f.app().column_block(), blocco(7, 21, 1, 2));
    giro(&mut f, &ctx, vec![Event::PointerMoved(c)]);
    assert_eq!(f.app().column_block(), blocco(0, 7, 1, 4));
    giro(&mut f, &ctx, vec![premi(c, false, Modifiers::NONE)]);
    // dopo il rilascio il puntatore non muove piu' il blocco
    giro(&mut f, &ctx, vec![Event::PointerMoved(b)]);
    assert_eq!(f.app().column_block(), blocco(0, 7, 1, 4));
    let _ = std::fs::remove_file(p);
}
