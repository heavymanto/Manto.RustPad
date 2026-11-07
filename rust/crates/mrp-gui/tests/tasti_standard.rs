//! I tasti standard della finestra e due cose di disposizione: F3 = Trova, F4 = Sostituisci (nel
//! terminale F4 resta la modifica); la barra dei salti sta sopra le schede; le finestre dei dialoghi si spostano trascinando
//! il titolo e restano dove le si lascia. Scritti da ARCH.

use eframe::egui::{self, Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tasti_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, b"uno\ndue\ntre\n").unwrap();
    p
}

fn cfg() -> AppConfig {
    AppConfig {
        chrome: false,
        ..AppConfig::default()
    }
}

fn finestra() -> (Finestra, Context, PathBuf) {
    let p = file();
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
    let mut f = f;
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

fn tasto(key: Key) -> Event {
    tasto_con(key, Modifiers::NONE)
}

fn tasto_con(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn tasti(f: &mut Finestra, ctx: &Context, key: Key) {
    giro(f, ctx, vec![tasto(key)]);
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
}

#[test]
fn f3_finds_f4_replaces_and_alt_e_alt_v_switch_between_editing_and_reading() {
    let (mut f, ctx, p) = finestra();
    tasti(&mut f, &ctx, Key::F3);
    assert_eq!(f.dialogo_titolo(), Some("FIND"));
    tasti(&mut f, &ctx, Key::Escape);
    assert_eq!(f.dialogo_titolo(), None);
    tasti(&mut f, &ctx, Key::F4);
    assert_eq!(f.dialogo_titolo(), Some("REPLACE"));
    tasti(&mut f, &ctx, Key::Escape);
    assert!(!f.app().is_edit(), "F4 non e' piu' la modifica");
    giro(&mut f, &ctx, vec![tasto_con(Key::E, Modifiers::ALT)]);
    assert!(f.app().is_edit());
    giro(&mut f, &ctx, vec![tasto_con(Key::V, Modifiers::ALT)]);
    assert!(!f.app().is_edit());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_jump_bar_sits_right_under_the_toolbar_and_above_the_tabs() {
    let (mut f, ctx, p) = finestra();
    let q = file();
    f.workspace().open(&q).unwrap();
    giro(&mut f, &ctx, vec![]);
    giro(
        &mut f,
        &ctx,
        vec![Event::Key {
            key: Key::B,
            physical_key: Some(Key::B),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        }],
    );
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    let toolbar = f.strumenti_rect().unwrap();
    let salti = f.salti_rect().unwrap();
    // attaccata agli strumenti (senza le schede in mezzo)
    assert!(
        (salti.min.y - toolbar.max.y).abs() < 4.0,
        "{salti:?} sotto {toolbar:?}"
    );
    // il testo comincia piu' sotto, dopo la striscia delle schede
    let (o, _, _, _) = f.geometria().unwrap();
    assert!(o.y > salti.max.y + 10.0, "{o:?} sotto {salti:?}");
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn a_dialog_can_be_dragged_by_its_title_and_stays_there() {
    let (mut f, ctx, p) = finestra();
    tasti(&mut f, &ctx, Key::F3);
    let prima = f.dialogo_rect().unwrap();
    // si afferra il titolo (in alto, al centro) e si trascina di 250 px a sinistra e 120 in basso
    let presa = Pos2::new(prima.center().x, prima.min.y + 8.0);
    let arrivo = presa + Vec2::new(-250.0, 120.0);
    giro(
        &mut f,
        &ctx,
        vec![
            Event::PointerMoved(presa),
            Event::PointerButton {
                pos: presa,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    giro(&mut f, &ctx, vec![Event::PointerMoved(arrivo)]);
    giro(
        &mut f,
        &ctx,
        vec![Event::PointerButton {
            pos: arrivo,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    giro(&mut f, &ctx, vec![]);
    let dopo = f.dialogo_rect().unwrap();
    assert!(
        (dopo.min.x - (prima.min.x - 250.0)).abs() < 3.0
            && (dopo.min.y - (prima.min.y + 120.0)).abs() < 3.0,
        "{prima:?} -> {dopo:?}"
    );
    // altri giri senza toccare niente: resta dov'e'
    giro(&mut f, &ctx, vec![]);
    assert_eq!(f.dialogo_rect().unwrap(), dopo);
    let _ = std::fs::remove_file(p);
}
