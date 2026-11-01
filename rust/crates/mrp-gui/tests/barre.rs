//! La barra di stato e la scrollbar della finestra (B2b). Si vedono solo quando l'`App` e' senza la cornice del terminale
//! (`AppConfig::chrome = false`: e' cosi' che la apre `mrp-gui`). Scritti da ARCH: gli attesi sono calcolati a mano.
//! Le funzioni pure (`testi_stato`, `pollice`, `frazione_da_posizione`) si provano da sole; la finestra si prova con un
//! contesto di egui senza schermo, un `giro` alla volta con eventi finti, come negli altri test della GUI.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_core::lang::Catalog;
use mrp_gui::barre::{frazione_da_posizione, pollice, testi_stato, Pezzo, POLLICE_MINIMO};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig, Busy, DialogKind, ScrollInfo, StatusInfo};
use mrp_tui::commands::Cmd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

// ---- le funzioni pure ----

fn info() -> StatusInfo {
    StatusInfo {
        file_name: "a.txt".to_string(),
        size: 2048,
        edit: false,
        modified: false,
        column_mode: false,
        untitled: false,
        follow: false,
        line: Some(3),
        column: 4,
        offset: 23,
        percent: 2,
        encoding: "UTF-8".to_string(),
        line_ending: "LF",
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
fn the_status_text_in_english() {
    let t = testi_stato(&info(), &Catalog::english());
    assert_eq!(t.sinistra, "Ln 3  Col 4  byte 23  2%");
    assert_eq!(
        t.destra,
        vec![
            pezzo("VIEW", Some(Cmd::EditMode)),
            pezzo("UTF-8", Some(Cmd::Encoding)),
            pezzo("LF", None),
            pezzo("2.00 KiB", None),
        ]
    );
}

#[test]
fn edit_mode_column_mode_and_line_ending_show_up() {
    let mut s = info();
    s.edit = true;
    s.column_mode = true;
    s.line_ending = "CRLF";
    s.encoding = "windows-1252".to_string();
    s.size = 0;
    let t = testi_stato(&s, &Catalog::english());
    assert_eq!(
        t.destra,
        vec![
            pezzo("EDIT", Some(Cmd::EditMode)),
            pezzo("COL", Some(Cmd::ColumnMode)),
            pezzo("windows-1252", Some(Cmd::Encoding)),
            pezzo("CRLF", None),
            pezzo("0 B", None),
        ]
    );
}

#[test]
fn an_unknown_line_is_a_question_mark() {
    let mut s = info();
    s.line = None;
    assert_eq!(
        testi_stato(&s, &Catalog::english()).sinistra,
        "Ln ?  Col 4  byte 23  2%"
    );
}

#[test]
fn the_flash_replaces_the_position_but_not_the_right_side() {
    let mut s = info();
    s.flash = Some("saved: a.txt".to_string());
    let t = testi_stato(&s, &Catalog::english());
    assert_eq!(t.sinistra, "saved: a.txt");
    assert_eq!(t.destra.len(), 4);
}

#[test]
fn background_work_comes_first_on_the_right() {
    for (busy, atteso) in [
        ((Busy::Saving, 42), "saving 42%"),
        ((Busy::Searching, 7), "searching 7%"),
        ((Busy::Indexing, 100), "indexing 100%"),
    ] {
        let mut s = info();
        s.busy = Some(busy);
        let t = testi_stato(&s, &Catalog::english());
        assert_eq!(t.destra.len(), 5, "{atteso}");
        assert_eq!(t.destra[0], pezzo(atteso, None));
        assert_eq!(t.destra[1], pezzo("VIEW", Some(Cmd::EditMode)));
    }
}

#[test]
fn sizes_use_the_same_units_as_the_terminal_title() {
    for (byte, atteso) in [
        (0u64, "0 B"),
        (1000, "1000 B"),
        (1536, "1.50 KiB"),
        (5 * 1024 * 1024, "5.00 MiB"),
    ] {
        let mut s = info();
        s.size = byte;
        let t = testi_stato(&s, &Catalog::english());
        assert_eq!(t.destra.last().unwrap().testo, atteso);
    }
}

#[test]
fn the_status_speaks_the_chosen_language() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang");
    let (lingue, _) = mrp_core::lang::registry_in(Some(dir.as_path()));
    let it = lingue
        .into_iter()
        .find(|l| l.code == "it")
        .expect("c'e' l'italiano");
    let t = testi_stato(&info(), &Catalog::with(it));
    assert_eq!(t.sinistra, "Rg 3  Col 4  byte 23  2%");
    assert_eq!(t.destra[0], pezzo("LETTURA", Some(Cmd::EditMode)));
}

#[test]
fn the_thumb_follows_the_window() {
    let i = |start, size| ScrollInfo { start, size };
    // dall'inizio, un ventesimo: 35 px su 700
    assert_eq!(pollice(i(0.0, 0.05), 700.0, 16.0), (0.0, 35.0));
    // a meta': 70 px a 350
    assert_eq!(pollice(i(0.5, 0.1), 700.0, 16.0), (350.0, 70.0));
    // il documento intero: tutta la barra
    assert_eq!(pollice(i(0.0, 1.0), 700.0, 16.0), (0.0, 700.0));
    // troppo piccolo: il minimo, e sta dove deve (0.3 * 700 = 210)
    assert_eq!(pollice(i(0.3, 0.0001), 700.0, 16.0), (210.0, 16.0));
    // in fondo non esce dalla barra: 0.99 * 700 = 693 ma 700 - 35 = 665
    assert_eq!(pollice(i(0.99, 0.05), 700.0, 16.0), (665.0, 35.0));
    // una barra piu' bassa del minimo
    assert_eq!(pollice(i(0.2, 0.01), 10.0, 16.0), (0.0, 10.0));
    assert_eq!(POLLICE_MINIMO, 16.0);
}

#[test]
fn a_position_on_the_bar_is_a_fraction_of_the_document() {
    assert_eq!(frazione_da_posizione(350.0, 0.0, 700.0), 0.5);
    // la presa: dove dentro al pollice si e' afferrato (con 35 px dentro, il pollice sta 35 px sopra il puntatore)
    assert_eq!(frazione_da_posizione(385.0, 35.0, 700.0), 0.5);
    assert_eq!(frazione_da_posizione(-20.0, 0.0, 700.0), 0.0);
    assert_eq!(frazione_da_posizione(900.0, 0.0, 700.0), 1.0);
    assert_eq!(
        frazione_da_posizione(100.0, 0.0, 0.0),
        0.0,
        "barra di altezza zero"
    );
}

// ---- la finestra ----

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_barre_test_{}_{}.txt", std::process::id(), n));
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

fn tasto(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
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

/// Un clic completo: muovi e premi, rilascia, un giro di riposo.
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

fn molte_righe() -> Vec<u8> {
    (0..2000)
        .flat_map(|i| format!("l{i:08}\n").into_bytes())
        .collect()
}

#[test]
fn the_bars_exist_only_when_the_app_has_no_terminal_chrome() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\n", true);
    disegna(&mut f, &ctx);
    assert!(f.barra_stato_rect().is_none() && f.scrollbar_rect().is_none());
    assert!(f.stato_disegnato().is_none());
    let (mut g, ctx2, q) = finestra(b"uno\ndue\n", false);
    disegna(&mut g, &ctx2);
    assert!(g.barra_stato_rect().is_some() && g.scrollbar_rect().is_some());
    assert!(g.stato_disegnato().is_some());
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn the_status_bar_is_at_the_bottom_and_the_scrollbar_at_the_right() {
    let (mut f, ctx, p) = finestra(&molte_righe(), false);
    disegna(&mut f, &ctx);
    let stato = f.barra_stato_rect().unwrap();
    let barra = f.scrollbar_rect().unwrap();
    assert_eq!(
        (stato.min.x, stato.max.x),
        (0.0, 1000.0),
        "la barra di stato e' larga quanto la finestra"
    );
    assert!(stato.max.y >= 699.0 && stato.min.y > 650.0, "{stato:?}");
    assert_eq!(barra.max.x, 1000.0, "la scrollbar sta sul bordo destro");
    assert!(barra.width() >= 8.0 && barra.width() <= 24.0, "{barra:?}");
    assert!(
        barra.min.y >= 15.0 && barra.height() > 400.0,
        "sotto il menu: {barra:?}"
    );
    assert!(
        barra.max.y <= stato.min.y + 0.5,
        "sopra la barra di stato: {barra:?} {stato:?}"
    );
    // la griglia del testo sta fra le due
    let (o, c, colonne, righe) = f.geometria().unwrap();
    assert!(o.x + f32::from(colonne) * c.x <= barra.min.x + 0.5);
    assert!(o.y + f32::from(righe) * c.y <= stato.min.y + 0.5);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_status_text_follows_the_cursor() {
    let (mut f, ctx, p) = finestra(b"abc\ndef\nghi\n", false);
    disegna(&mut f, &ctx);
    assert_eq!(
        f.stato_disegnato().unwrap().sinistra,
        "Ln 1  Col 1  byte 0  0%"
    );
    giro(&mut f, &ctx, vec![tasto(Key::ArrowDown)]);
    giro(&mut f, &ctx, vec![]);
    assert_eq!(
        f.stato_disegnato().unwrap().sinistra,
        "Ln 2  Col 1  byte 4  33%"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_right_side_pieces_have_a_place_on_the_bar() {
    let (mut f, ctx, p) = finestra(b"abc\n", false);
    disegna(&mut f, &ctx);
    let stato = f.barra_stato_rect().unwrap();
    let pezzi = f.pezzi_stato();
    // VIEW, UTF-8, LF, 4 B: ognuno ha un rettangolo dentro la barra, da sinistra a destra senza sovrapporsi
    let t = f.stato_disegnato().unwrap();
    assert_eq!(pezzi.len(), t.destra.len());
    for (r, _) in pezzi {
        assert!(stato.contains_rect(*r), "{r:?} fuori da {stato:?}");
    }
    for w in pezzi.windows(2) {
        assert!(w[0].0.max.x <= w[1].0.min.x + 0.5, "{:?}", w);
    }
    let _ = std::fs::remove_file(p);
}

fn centro_del_pezzo(f: &Finestra, cmd: Cmd) -> Pos2 {
    f.pezzi_stato()
        .iter()
        .find(|(_, c)| *c == Some(cmd))
        .unwrap_or_else(|| panic!("manca il pezzo di {cmd:?}"))
        .0
        .center()
}

#[test]
fn clicking_the_encoding_piece_opens_the_encoding_menu() {
    let (mut f, ctx, p) = finestra(b"abc\n", false);
    disegna(&mut f, &ctx);
    let pos = centro_del_pezzo(&f, Cmd::Encoding);
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Encoding));
    let _ = std::fs::remove_file(p);
}

#[test]
fn clicking_the_mode_piece_switches_between_view_and_edit() {
    let (mut f, ctx, p) = finestra(b"abc\n", false);
    disegna(&mut f, &ctx);
    assert!(!f.app().is_edit());
    let pos = centro_del_pezzo(&f, Cmd::EditMode);
    clic(&mut f, &ctx, pos);
    assert!(f.app().is_edit());
    disegna(&mut f, &ctx);
    let pos = centro_del_pezzo(&f, Cmd::EditMode);
    clic(&mut f, &ctx, pos);
    assert!(!f.app().is_edit());
    let _ = std::fs::remove_file(p);
}

#[test]
fn clicking_the_track_puts_the_thumb_there() {
    let (mut f, ctx, p) = finestra(&molte_righe(), false);
    disegna(&mut f, &ctx);
    let barra = f.scrollbar_rect().unwrap();
    // l'altezza del pollice (almeno il minimo): il clic lo centra, quindi la sua cima va mezza altezza sopra il puntatore
    let (_, alto) = pollice(f.app().scroll_info(), barra.height(), POLLICE_MINIMO);
    clic(&mut f, &ctx, Pos2::new(barra.center().x, barra.center().y));
    disegna(&mut f, &ctx);
    let atteso = f64::from(barra.height() / 2.0 - alto / 2.0) / f64::from(barra.height());
    let s = f.app().scroll_info();
    assert!(
        (s.start - atteso).abs() < 0.001,
        "start {} atteso {atteso}",
        s.start
    );
    // un clic in cima porta all'inizio
    clic(&mut f, &ctx, Pos2::new(barra.center().x, barra.min.y + 1.0));
    disegna(&mut f, &ctx);
    assert!(f.app().scroll_info().start < 0.01);
    let _ = std::fs::remove_file(p);
}

#[test]
fn dragging_the_thumb_moves_the_window_by_the_same_distance() {
    let (mut f, ctx, p) = finestra(&molte_righe(), false);
    disegna(&mut f, &ctx);
    let barra = f.scrollbar_rect().unwrap();
    let (cima, alto) = pollice(f.app().scroll_info(), barra.height(), POLLICE_MINIMO);
    let presa = Pos2::new(barra.center().x, barra.min.y + cima + alto / 2.0);
    let arrivo = Pos2::new(presa.x, presa.y + 140.0);
    giro(
        &mut f,
        &ctx,
        vec![Event::PointerMoved(presa), pigia(presa, true)],
    );
    giro(
        &mut f,
        &ctx,
        vec![Event::PointerMoved(Pos2::new(presa.x, presa.y + 70.0))],
    );
    giro(&mut f, &ctx, vec![Event::PointerMoved(arrivo)]);
    giro(&mut f, &ctx, vec![pigia(arrivo, false)]);
    disegna(&mut f, &ctx);
    let atteso = f64::from(cima + 140.0) / f64::from(barra.height());
    let s = f.app().scroll_info();
    assert!(
        (s.start - atteso).abs() < 0.01,
        "start {} atteso {atteso}",
        s.start
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_wheel_moves_the_thumb_too() {
    let (mut f, ctx, p) = finestra(&molte_righe(), false);
    disegna(&mut f, &ctx);
    assert!(f.app().scroll_info().start < 0.001);
    f.app().scroll_to_fraction(0.4);
    disegna(&mut f, &ctx);
    assert!((f.app().scroll_info().start - 0.4).abs() < 0.01);
    let _ = std::fs::remove_file(p);
}
