//! Due pulsanti nella barra degli strumenti: lo zoom (indietro e avanti) e il percorso del file (sopra si legge il percorso
//! completo, un clic lo copia negli appunti). Scritti da ARCH: il percorso atteso lo calcola il test con `canonicalize` di std.

use eframe::egui::{
    self, Context, Event, FullOutput, Modifiers, OutputCommand, Pos2, RawInput, Rect, Shape, Vec2,
};
use mrp_gui::gui::Finestra;
use mrp_gui::strumenti::percorso_leggibile;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::Cmd;
use std::path::{Path, PathBuf};
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
    let d = std::env::temp_dir().join(format!("mrp_zp_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("dati di prova.log");
    std::fs::write(&p, b"uno\ndue\n").unwrap();
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
    ctx.style_mut(|s| {
        s.animation_time = 0.0;
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
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

fn testi(o: &FullOutput) -> Vec<String> {
    fn visita(s: &Shape, v: &mut Vec<String>) {
        match s {
            Shape::Text(t) => v.push(t.galley.text().to_string()),
            Shape::Vec(tutte) => tutte.iter().for_each(|x| visita(x, v)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &o.shapes {
        visita(&c.shape, &mut v);
    }
    v
}

fn sopra(f: &mut Finestra, ctx: &Context, pos: Pos2) -> FullOutput {
    giro(f, ctx, vec![Event::PointerMoved(pos)]);
    giro(f, ctx, vec![Event::PointerMoved(pos)]);
    giro(f, ctx, vec![Event::PointerMoved(pos)])
}

/// Un clic; restituisce i comandi di piattaforma emessi (gli appunti) dai giri del clic.
fn clic(f: &mut Finestra, ctx: &Context, pos: Pos2) -> Vec<OutputCommand> {
    let mut comandi = Vec::new();
    for premuto in [true, false] {
        let o = giro(
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
        comandi.extend(o.platform_output.commands);
    }
    let o = giro(f, ctx, vec![]);
    comandi.extend(o.platform_output.commands);
    comandi
}

fn copiato(comandi: &[OutputCommand]) -> Vec<String> {
    comandi
        .iter()
        .filter_map(|c| match c {
            OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

/// Il percorso completo atteso, senza il prefisso `\?\` di Windows.
fn atteso(p: &Path) -> String {
    let s = std::fs::canonicalize(p).unwrap().display().to_string();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

#[test]
fn the_toolbar_has_zoom_out_and_zoom_in_that_work() {
    let (mut f, ctx, p) = finestra();
    let base = f.corpo();
    let pulsante = |f: &Finestra, cmd: Cmd| {
        f.pulsanti()
            .iter()
            .find(|b| b.cmd == cmd)
            .copied()
            .unwrap_or_else(|| panic!("manca il pulsante {cmd:?}"))
    };
    let (piu, meno) = (pulsante(&f, Cmd::ZoomIn), pulsante(&f, Cmd::ZoomOut));
    assert!(piu.attivo && meno.attivo);
    // lo zoom indietro sta a sinistra di quello avanti
    assert!(meno.rect.max.x <= piu.rect.min.x);
    // i suggerimenti dicono nome e scorciatoia (prima dei clic: dopo un clic il suggerimento aspetta)
    let o = sopra(&mut f, &ctx, piu.rect.center());
    assert!(
        testi(&o).iter().any(|t| t == "Zoom in (Ctrl++)"),
        "{:?}",
        testi(&o)
    );
    let o = sopra(&mut f, &ctx, meno.rect.center());
    assert!(
        testi(&o).iter().any(|t| t == "Zoom out (Ctrl+-)"),
        "{:?}",
        testi(&o)
    );
    clic(&mut f, &ctx, piu.rect.center());
    assert_eq!(f.corpo(), base + 1.0);
    clic(&mut f, &ctx, meno.rect.center());
    clic(&mut f, &ctx, meno.rect.center());
    assert_eq!(f.corpo(), base - 1.0);
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn hovering_the_path_button_shows_the_full_path_and_a_click_copies_it() {
    let (mut f, ctx, p) = finestra();
    let r = f.percorso_rect().expect("manca il pulsante del percorso");
    // sta a destra, nella barra degli strumenti
    assert!(r.min.x > 700.0, "{r:?}");
    let o = sopra(&mut f, &ctx, r.center());
    let voluto = atteso(&p);
    assert!(
        testi(&o)
            .iter()
            .any(|t| t.contains(&voluto) && t.contains("Click to copy")),
        "il percorso completo {voluto:?} non e' nel suggerimento: {:?}",
        testi(&o)
    );
    let comandi = clic(&mut f, &ctx, r.center());
    assert_eq!(copiato(&comandi), vec![voluto.clone()]);
    assert!(
        f.app().flash().is_some_and(|m| m.contains("copied")),
        "{:?}",
        f.app().flash()
    );
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn a_document_that_is_not_a_file_yet_has_nothing_to_copy_but_still_explains() {
    let (mut f, ctx, p) = finestra();
    f.azione(Cmd::New);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    assert!(f.app().is_untitled());
    let r = f.percorso_rect().unwrap();
    let o = sopra(&mut f, &ctx, r.center());
    assert!(
        testi(&o).iter().any(|t| t.contains("Not saved yet")),
        "{:?}",
        testi(&o)
    );
    let comandi = clic(&mut f, &ctx, r.center());
    assert!(
        copiato(&comandi).is_empty(),
        "niente da copiare: {comandi:?}"
    );
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn the_windows_long_path_prefix_is_removed() {
    assert_eq!(
        percorso_leggibile(Path::new(r"\\?\C:\dati\a.log")),
        r"C:\dati\a.log"
    );
    assert_eq!(
        percorso_leggibile(Path::new(r"\\?\UNC\server\cond\a.log")),
        r"\\server\cond\a.log"
    );
    assert_eq!(
        percorso_leggibile(Path::new(r"C:\dati\a.log")),
        r"C:\dati\a.log"
    );
    assert_eq!(
        percorso_leggibile(Path::new("/home/u/a.log")),
        "/home/u/a.log"
    );
}
