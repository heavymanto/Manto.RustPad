//! Prova a mano (ignorata): apre il file indicato da `MRP_BIG` e misura quanto dura ogni giro della finestra mentre si
//! trascina la scrollbar a meta' e mentre l'indice righe lavora ancora.

use eframe::egui::{Context, FullOutput, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use std::time::{Duration, Instant};

fn giro(f: &mut Finestra, ctx: &Context) -> (FullOutput, Duration) {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 800.0))),
        ..Default::default()
    };
    let t = Instant::now();
    let o = ctx.run(raw, |ctx| f.giro(ctx));
    (o, t.elapsed())
}

#[test]
#[ignore]
fn drag_on_a_big_file() {
    let Ok(p) = std::env::var("MRP_BIG") else {
        return;
    };
    let t = Instant::now();
    let app = App::open(
        std::path::Path::new(&p),
        AppConfig {
            chrome: false,
            ..AppConfig::default()
        },
    )
    .unwrap();
    println!("open: {:?}", t.elapsed());
    let mut f = Finestra::da_app(
        app,
        Box::new(|| AppConfig {
            chrome: false,
            ..AppConfig::default()
        }),
    );
    let ctx = Context::default();
    for i in 0..5 {
        let (_, d) = giro(&mut f, &ctx);
        println!("giro {i}: {d:?}");
    }
    for k in 0..40 {
        let frac = 0.3 + 0.01 * k as f64;
        let t = Instant::now();
        f.workspace().active_mut().scroll_to_fraction(frac);
        let a = t.elapsed();
        let (_, d) = giro(&mut f, &ctx);
        println!("frac {frac:.2}: scroll {a:?}, giro {d:?}");
    }
}

#[test]
#[ignore]
fn index_speed_on_a_big_file() {
    let Ok(p) = std::env::var("MRP_BIG") else {
        return;
    };
    let secondi: u64 = std::env::var("MRP_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(15);
    let cfg = || AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut app = App::open(std::path::Path::new(&p), cfg()).unwrap();
    let len = app.document_len();
    let t = Instant::now();
    while t.elapsed().as_secs() < secondi {
        std::thread::sleep(Duration::from_secs(5));
        let busy = app.status().busy;
        println!("{:>3}s: {:?}", t.elapsed().as_secs(), busy);
        if busy.is_none() {
            break;
        }
    }
    println!("len {} byte", len);
}
