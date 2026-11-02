//! I quattro temi della finestra (Blu stile Total Commander, Debian, Scuro, Chiaro): i popup hanno i colori del loro tema (nel
//! Blu lo stesso blu del testo, in Debian il grigio chiaro dei dialoghi debconf su fondo blu), la testata di ogni popup ha un
//! fondo di risalto, e TUTTE le finestrelle si spostano trascinando (la testata, ma anche il corpo). Scritti da ARCH: i colori
//! attesi sono scritti a mano (la tavolozza ANSI della console di Windows e i valori RGB dei temi), mai letti dal codice.

use eframe::egui::{
    self, Color32, Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Shape, Vec2,
};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::Cmd;
use mrp_tui::theme::ThemeId;
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
    let p = std::env::temp_dir().join(format!("mrp_stile_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, b"uno\ndue\n").unwrap();
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
    // niente dissolvenza all'apertura delle finestre: i colori dei riempimenti sono subito quelli veri (senza trasparenza)
    ctx.style_mut(|s| s.animation_time = 0.0);
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

/// Tutti i rettangoli pieni disegnati: (rettangolo, colore di riempimento).
fn riempimenti(o: &FullOutput) -> Vec<(Rect, Color32)> {
    fn visita(s: &Shape, v: &mut Vec<(Rect, Color32)>) {
        match s {
            Shape::Rect(r) => v.push((r.rect, r.fill)),
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

fn quasi(a: Rect, b: Rect, tol: f32) -> bool {
    (a.min - b.min).abs().max_elem() <= tol && (a.max - b.max).abs().max_elem() <= tol
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Imposta il tema, apre il dialogo "vai a" e restituisce l'ultimo disegno.
fn disegna_dialogo(f: &mut Finestra, ctx: &Context, tema: ThemeId) -> FullOutput {
    f.workspace().set_theme(tema);
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![tasto(Key::G, Modifiers::CTRL)]);
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![])
}

#[test]
fn popups_take_the_colours_of_their_theme_and_the_title_stands_out() {
    // (tema, fondo del popup, fondo della testata): la tavolozza della console di Windows e i valori RGB dei temi
    let attesi = [
        (ThemeId::Night, [0x00, 0x00, 0xaa], [193, 156, 0]),
        (ThemeId::Debian, [204, 204, 204], [197, 15, 31]),
        (ThemeId::Dark, [0x25, 0x25, 0x26], [0x09, 0x47, 0x71]),
        (ThemeId::Light, [0xf3, 0xf3, 0xf3], [0x00, 0x60, 0xc0]),
    ];
    for (tema, popup, testata) in attesi {
        let (mut f, ctx, p) = finestra();
        let o = disegna_dialogo(&mut f, &ctx, tema);
        let finestra = f
            .dialogo_rect()
            .unwrap_or_else(|| panic!("{tema:?}: nessun dialogo"));
        let pieni = riempimenti(&o);
        assert!(
            pieni
                .iter()
                .any(|(r, c)| quasi(*r, finestra, 3.0) && *c == rgb(popup)),
            "{tema:?}: il popup non ha il fondo {popup:?}"
        );
        // la testata: un rettangolo col risalto, in alto, largo come la finestra
        assert!(
            pieni.iter().any(|(r, c)| *c == rgb(testata)
                && (r.min.y - finestra.min.y).abs() <= 4.0
                && r.width() >= finestra.width() - 4.0
                && r.height() >= 24.0),
            "{tema:?}: la testata non ha il fondo di risalto {testata:?}"
        );
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn the_blue_theme_popup_is_the_same_blue_as_the_screen_and_debian_is_not() {
    let (mut f, ctx, p) = finestra();
    let blu = rgb([0x00, 0x00, 0xaa]);
    // il fondo dei pannelli (menu, barre, stato) e il fondo del testo sono il blu del tema; col Blu anche il popup
    let o = disegna_dialogo(&mut f, &ctx, ThemeId::Night);
    let d = f.dialogo_rect().unwrap();
    assert!(riempimenti(&o)
        .iter()
        .any(|(r, c)| quasi(*r, d, 3.0) && *c == blu));
    // con Debian lo schermo resta blu ma il popup e' grigio chiaro
    let o = disegna_dialogo(&mut f, &ctx, ThemeId::Debian);
    let d = f.dialogo_rect().unwrap();
    let pieni = riempimenti(&o);
    assert!(pieni
        .iter()
        .any(|(r, c)| quasi(*r, d, 3.0) && *c == rgb([204, 204, 204])));
    assert!(!pieni.iter().any(|(r, c)| quasi(*r, d, 3.0) && *c == blu));
    let _ = std::fs::remove_file(p);
}

/// Un modo di aprire un dialogo.
type Apertura = Box<dyn Fn(&mut Finestra, &Context)>;

/// Trascina il puntatore da `da` a `a` (premuto in `da`, qualche passo, rilasciato in `a`).
fn trascina(f: &mut Finestra, ctx: &Context, da: Pos2, a: Pos2) {
    giro(f, ctx, vec![Event::PointerMoved(da)]);
    giro(
        f,
        ctx,
        vec![Event::PointerButton {
            pos: da,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }],
    );
    for i in 1..=4 {
        let p = da + (a - da) * (i as f32 / 4.0);
        giro(f, ctx, vec![Event::PointerMoved(p)]);
    }
    giro(
        f,
        ctx,
        vec![Event::PointerButton {
            pos: a,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
}

#[test]
fn every_dialog_window_moves_when_you_drag_its_title_band() {
    let aperture: Vec<(&str, Apertura)> = vec![
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
            "codifica del salvataggio",
            Box::new(|f, ctx| {
                f.app()
                    .save_as_choose(std::env::temp_dir().join("mrp_stile_dest.txt"));
                giro(f, ctx, vec![]);
            }),
        ),
    ];
    for tema in [
        ThemeId::Night,
        ThemeId::Debian,
        ThemeId::Dark,
        ThemeId::Light,
    ] {
        for (nome, apri) in &aperture {
            let (mut f, ctx, p) = finestra();
            f.workspace().set_theme(tema);
            giro(&mut f, &ctx, vec![]);
            apri(&mut f, &ctx);
            giro(&mut f, &ctx, vec![]);
            giro(&mut f, &ctx, vec![]);
            let prima = f
                .dialogo_rect()
                .unwrap_or_else(|| panic!("{nome}: nessuna finestra"));
            let x = f.dialogo_chiudi().unwrap();
            // un punto della banda, fra il titolo e la X
            let da = Pos2::new((prima.center().x + x.min.x) / 2.0, prima.min.y + 16.0);
            let a = da + Vec2::new(90.0, 60.0);
            trascina(&mut f, &ctx, da, a);
            let dopo = f.dialogo_rect().unwrap();
            let spostamento = dopo.min - prima.min;
            assert!(
                (spostamento - Vec2::new(90.0, 60.0)).abs().max_elem() <= 3.0,
                "{tema:?}/{nome}: la finestra si e' spostata di {spostamento:?}, non di (90, 60)"
            );
            let _ = std::fs::remove_file(p);
        }
    }
}

#[test]
fn a_drag_on_the_body_of_a_window_moves_it_too_help_and_settings() {
    // l'aiuto: un clic-trascinamento su una riga della tabella (prima scorreva la lista, o selezionava il testo)
    let (mut f, ctx, p) = finestra();
    giro(&mut f, &ctx, vec![tasto(Key::F1, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    let prima = f.dialogo_rect().unwrap();
    let da = Pos2::new(prima.min.x + 40.0, prima.center().y);
    trascina(&mut f, &ctx, da, da + Vec2::new(70.0, 50.0));
    let dopo = f.dialogo_rect().unwrap();
    assert!(
        ((dopo.min - prima.min) - Vec2::new(70.0, 50.0))
            .abs()
            .max_elem()
            <= 3.0,
        "aiuto: {prima:?} -> {dopo:?}"
    );
    // la finestra delle impostazioni: dalla riga in fondo (il percorso del file) e dalla testata
    for da_testata in [true, false] {
        let (mut f, ctx, p2) = finestra();
        f.azione(Cmd::Settings);
        giro(&mut f, &ctx, vec![]);
        giro(&mut f, &ctx, vec![]);
        let prima = f.impostazioni_rect().unwrap();
        let da = if da_testata {
            let x = f.impostazioni_chiudi().unwrap();
            Pos2::new((prima.center().x + x.min.x) / 2.0, prima.min.y + 16.0)
        } else {
            Pos2::new(prima.min.x + 60.0, prima.max.y - 20.0)
        };
        trascina(&mut f, &ctx, da, da + Vec2::new(-80.0, 40.0));
        let dopo = f.impostazioni_rect().unwrap();
        assert!(
            ((dopo.min - prima.min) - Vec2::new(-80.0, 40.0))
                .abs()
                .max_elem()
                <= 3.0,
            "impostazioni (testata: {da_testata}): {prima:?} -> {dopo:?}"
        );
        let _ = std::fs::remove_file(p2);
    }
    let _ = std::fs::remove_file(p);
}

// ---- i difettini di UX: suggerimenti sempre, pulsanti dei salti grandi, acceso / spento ben distinti ----

/// I testi di tutto quello che e' stato disegnato (i suggerimenti compresi).
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

/// Il puntatore resta fermo su `pos` per qualche giro (il suggerimento senza attesa) e restituisce l'ultimo disegno.
fn passa_sopra(f: &mut Finestra, ctx: &Context, pos: Pos2) -> FullOutput {
    ctx.style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    giro(f, ctx, vec![Event::PointerMoved(pos)]);
    giro(f, ctx, vec![Event::PointerMoved(pos)]);
    giro(f, ctx, vec![Event::PointerMoved(pos)]);
    giro(f, ctx, vec![Event::PointerMoved(pos)])
}

#[test]
fn a_disabled_toolbar_button_still_shows_its_tooltip() {
    let (mut f, ctx, p) = finestra();
    // Annulla e' spento (niente da annullare, e in lettura): prima il suggerimento non compariva
    let undo = f
        .pulsanti()
        .iter()
        .find(|b| b.cmd == Cmd::Undo)
        .copied()
        .unwrap();
    assert!(!undo.attivo);
    let o = passa_sopra(&mut f, &ctx, undo.rect.center());
    assert!(
        testi(&o).iter().any(|t| t.contains("Undo")),
        "nessun suggerimento: {:?}",
        testi(&o)
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_jump_buttons_have_tooltips_and_are_as_tall_as_the_toolbar_buttons() {
    let (mut f, ctx, p) = finestra();
    // la barra dei salti si accende con Ctrl+B (di fabbrica e' spenta nei test)
    if f.salti_rect().is_none() {
        f.azione(Cmd::NavBar);
    }
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    let alto_strumenti = f
        .pulsanti()
        .iter()
        .map(|b| b.rect.height())
        .fold(f32::MAX, f32::min);
    let salti: Vec<(Rect, _)> = f.pulsanti_salti().to_vec();
    assert!(!salti.is_empty(), "la barra dei salti non c'e'");
    for (r, voce) in &salti {
        assert!(
            r.height() >= alto_strumenti - 0.5 && r.height() >= 24.0,
            "{:?}: alto {} contro {alto_strumenti}",
            voce.etichetta,
            r.height()
        );
    }
    // i suggerimenti: inizio, fine e un salto di pagine con i suoi tasti
    let primo = salti.first().unwrap().0.center();
    let ultimo = salti.last().unwrap().0.center();
    assert!(testi(&passa_sopra(&mut f, &ctx, primo))
        .iter()
        .any(|t| t.contains("Start of the file")));
    assert!(testi(&passa_sopra(&mut f, &ctx, ultimo))
        .iter()
        .any(|t| t.contains("End of the file")));
    let (r, _) = salti
        .iter()
        .find(|(_, v)| v.etichetta == "+10")
        .expect("manca +10");
    let o = passa_sopra(&mut f, &ctx, r.center());
    assert!(
        testi(&o)
            .iter()
            .any(|t| t.contains("Forward 10 page(s) (Alt+PgDn)")),
        "{:?}",
        testi(&o)
    );
    let _ = std::fs::remove_file(p);
}

fn luce(c: Color32) -> f32 {
    0.299 * f32::from(c.r()) + 0.587 * f32::from(c.g()) + 0.114 * f32::from(c.b())
}

#[test]
fn a_toggle_button_that_is_on_is_clearly_different_from_one_that_is_off_in_every_theme() {
    for tema in [
        ThemeId::Night,
        ThemeId::Debian,
        ThemeId::Dark,
        ThemeId::Light,
    ] {
        let (mut f, ctx, p) = finestra();
        f.workspace().set_theme(tema);
        giro(&mut f, &ctx, vec![]);
        let o = giro(&mut f, &ctx, vec![]);
        // Modifica e Colonna sono spenti: il riempimento del loro pulsante
        let pulsante =
            |f: &Finestra, cmd: Cmd| f.pulsanti().iter().find(|b| b.cmd == cmd).copied().unwrap();
        let fondo = |o: &FullOutput, r: Rect| {
            riempimenti(o)
                .into_iter()
                .filter(|(q, _)| quasi(*q, r, 0.6))
                .map(|(_, c)| c)
                .next_back()
        };
        let spento = fondo(&o, pulsante(&f, Cmd::ColumnMode).rect);
        // accendo la modalita' colonna e ridisegno
        f.azione(Cmd::ColumnMode);
        giro(&mut f, &ctx, vec![]);
        let o2 = giro(&mut f, &ctx, vec![]);
        let b = pulsante(&f, Cmd::ColumnMode);
        assert!(b.acceso, "{tema:?}");
        let acceso = fondo(&o2, b.rect).expect("manca il fondo del pulsante acceso");
        let spento = spento.unwrap_or(Color32::TRANSPARENT);
        // abbastanza diversi: la luminosita' dista almeno 55 su 255, oppure il colore dista molto (Debian: rosso su blu)
        let dist = (i32::from(acceso.r()) - i32::from(spento.r())).abs()
            + (i32::from(acceso.g()) - i32::from(spento.g())).abs()
            + (i32::from(acceso.b()) - i32::from(spento.b())).abs();
        assert!(
            (luce(acceso) - luce(spento)).abs() >= 55.0 || dist >= 200,
            "{tema:?}: acceso {acceso:?} troppo simile a spento {spento:?}"
        );
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn the_items_of_a_drop_down_menu_have_no_border_of_their_own() {
    for tema in [
        ThemeId::Night,
        ThemeId::Debian,
        ThemeId::Dark,
        ThemeId::Light,
    ] {
        let (mut f, ctx, p) = finestra();
        f.workspace().set_theme(tema);
        giro(&mut f, &ctx, vec![]);
        // un clic sul primo menu (File) lo apre
        let pos = Pos2::new(18.0, 10.0);
        for premuto in [true, false] {
            giro(
                &mut f,
                &ctx,
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
        giro(&mut f, &ctx, vec![]);
        let o = giro(&mut f, &ctx, vec![]);
        // i rettangoli con il bordo sotto la barra dei menu: ce n'e' uno solo, la cornice del menu a tendina
        fn con_bordo(s: &Shape, v: &mut Vec<Rect>) {
            match s {
                Shape::Rect(r) if r.stroke.width > 0.0 && r.rect.min.y > 24.0 => v.push(r.rect),
                Shape::Vec(tutte) => tutte.iter().for_each(|x| con_bordo(x, v)),
                _ => {}
            }
        }
        let mut bordati = Vec::new();
        for c in &o.shapes {
            con_bordo(&c.shape, &mut bordati);
        }
        // le voci sono a destra e sotto il "File": cerco i rettangoli bordati dentro la zona del menu (x < 300, y < 400)
        let nel_menu: Vec<&Rect> = bordati
            .iter()
            .filter(|r| r.min.x < 300.0 && r.max.y < 400.0 && r.min.y > 22.0 && r.width() > 40.0)
            .collect();
        assert!(
            nel_menu.len() <= 1,
            "{tema:?}: le voci hanno un bordo proprio: {nel_menu:?}"
        );
        let _ = std::fs::remove_file(p);
    }
}
