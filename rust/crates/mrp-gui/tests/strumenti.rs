//! La barra degli strumenti con le icone e la barra dei salti della finestra (B3). Si vedono solo quando l'`App` e' senza la
//! cornice del terminale (come le barre di B2b). Scritti da ARCH: gli attesi sono calcolati a mano. Le icone sono caratteri
//! dei font incorporati in egui (nessuna dipendenza nuova); le funzioni pure si provano da sole, la finestra con un contesto
//! di egui senza schermo.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_core::lang::Catalog;
use mrp_gui::gui::Finestra;
use mrp_gui::strumenti::{glifo, gruppi, salti, tooltip, SaltoAzione};
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::commands::{Cmd, COMMANDS};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
const RIGA: u64 = 11;

// ---- le funzioni pure ----

#[test]
fn every_icon_of_the_registry_has_its_glyph_and_they_are_all_different() {
    let attesi = [
        ("new", "📄"),
        ("open", "📂"),
        ("save", "💾"),
        ("save_as", "📥"),
        ("close", "✖"),
        ("quit", "⏻"),
        ("edit", "✏"),
        ("undo", "↩"),
        ("redo", "↪"),
        ("cut", "✂"),
        ("copy", "📑"),
        ("paste", "📋"),
        ("select_all", "🔲"),
        ("find", "🔍"),
        ("goto", "➡"),
        ("replace", "🔁"),
        ("columns", "📊"),
        ("next_tab", "▶"),
        ("prev_tab", "◀"),
        ("jump_bar", "⏩"),
        ("theme", "🎨"),
        ("language", "🌐"),
        ("encoding", "🔤"),
        ("help", "❓"),
        ("home", "⏮"),
        ("end", "⏭"),
        ("zoom_in", "➕"),
        ("zoom_out", "➖"),
        ("zoom_reset", "◎"),
    ];
    assert_eq!(attesi.len(), COMMANDS.len(), "un'icona per comando");
    let mut visti: Vec<&str> = Vec::new();
    for (icona, g) in attesi {
        assert_eq!(glifo(icona), g, "{icona}");
        assert!(!visti.contains(&g), "{g} serve a due icone");
        visti.push(g);
    }
    for c in COMMANDS {
        assert_ne!(
            glifo(c.icon),
            "?",
            "l'icona {} del comando {:?} non ha il carattere",
            c.icon,
            c.cmd
        );
    }
    assert_eq!(glifo("non_esiste"), "?");
}

#[test]
fn the_toolbar_groups() {
    assert_eq!(
        gruppi(),
        vec![
            vec![Cmd::New, Cmd::Open, Cmd::Save],
            vec![Cmd::Undo, Cmd::Redo],
            vec![Cmd::Cut, Cmd::Copy, Cmd::Paste],
            vec![Cmd::Find, Cmd::Replace, Cmd::Goto],
            vec![Cmd::EditMode, Cmd::ColumnMode, Cmd::NavBar],
        ]
    );
}

#[test]
fn a_tooltip_is_the_name_and_the_shortcut() {
    let en = Catalog::english();
    for (cmd, atteso) in [
        (Cmd::Save, "Save (F2)"),
        (Cmd::SaveAs, "Save as… (Ctrl+Shift+S)"),
        (Cmd::Undo, "Undo (Ctrl+Z)"),
        (Cmd::Find, "Find… (Ctrl+F)"),
        (Cmd::NavBar, "Page-jump bar (Ctrl+B)"),
        (Cmd::ZoomIn, "Zoom in (Ctrl++)"),
        (Cmd::Home, "Go to start (Ctrl+Home)"),
    ] {
        assert_eq!(tooltip(cmd, &en), atteso, "{cmd:?}");
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang");
    let (lingue, _) = mrp_core::lang::registry_in(Some(dir.as_path()));
    let it = lingue.into_iter().find(|l| l.code == "it").unwrap();
    let it = Catalog::with(it);
    assert_eq!(tooltip(Cmd::Save, &it), "Salva (F2)");
    assert_eq!(tooltip(Cmd::NavBar, &it), "Barra dei salti (Ctrl+B)");
}

#[test]
fn the_jump_buttons_follow_the_configured_multipliers() {
    let v = salti([10, 100, 1000]);
    let etichette: Vec<&str> = v.iter().map(|s| s.etichetta.as_str()).collect();
    assert_eq!(
        etichette,
        vec![
            glifo("home"),
            "-1000",
            "-100",
            "-10",
            "-1",
            "+1",
            "+10",
            "+100",
            "+1000",
            glifo("end")
        ]
    );
    let azioni: Vec<SaltoAzione> = v.iter().map(|s| s.azione).collect();
    assert_eq!(
        azioni,
        vec![
            SaltoAzione::Inizio,
            SaltoAzione::Pagine(false, 1000),
            SaltoAzione::Pagine(false, 100),
            SaltoAzione::Pagine(false, 10),
            SaltoAzione::Pagine(false, 1),
            SaltoAzione::Pagine(true, 1),
            SaltoAzione::Pagine(true, 10),
            SaltoAzione::Pagine(true, 100),
            SaltoAzione::Pagine(true, 1000),
            SaltoAzione::Fine,
        ]
    );
    let v = salti([2, 3, 4]);
    let etichette: Vec<&str> = v.iter().map(|s| s.etichetta.as_str()).collect();
    assert_eq!(
        etichette,
        vec![
            glifo("home"),
            "-4",
            "-3",
            "-2",
            "-1",
            "+1",
            "+2",
            "+3",
            "+4",
            glifo("end")
        ]
    );
}

// ---- la finestra ----

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!(
        "mrp_strumenti_test_{}_{}.txt",
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

fn disegna(f: &mut Finestra, ctx: &Context) {
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
}

fn molte_righe() -> Vec<u8> {
    (0..30_000)
        .flat_map(|i| format!("riga {i:05}\n").into_bytes())
        .collect()
}

fn centro(f: &Finestra, cmd: Cmd) -> Pos2 {
    f.pulsanti()
        .iter()
        .find(|p| p.cmd == cmd)
        .unwrap_or_else(|| panic!("manca il pulsante di {cmd:?}"))
        .rect
        .center()
}

fn pulsante(f: &Finestra, cmd: Cmd) -> (bool, bool) {
    let p = f.pulsanti().iter().find(|p| p.cmd == cmd).unwrap();
    (p.attivo, p.acceso)
}

#[test]
fn the_bars_exist_only_when_the_app_has_no_terminal_chrome() {
    let (mut f, ctx, p) = finestra(b"uno\n", true);
    disegna(&mut f, &ctx);
    assert!(f.strumenti_rect().is_none() && f.pulsanti().is_empty());
    assert!(f.salti_rect().is_none() && f.pulsanti_salti().is_empty());
    let (mut g, ctx2, q) = finestra(b"uno\n", false);
    disegna(&mut g, &ctx2);
    assert!(g.strumenti_rect().is_some());
    assert!(
        g.salti_rect().is_none(),
        "la barra dei salti sta spenta finche' non si accende"
    );
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn the_toolbar_sits_under_the_menu_with_one_button_per_command_in_order() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    disegna(&mut f, &ctx);
    let barra = f.strumenti_rect().unwrap();
    assert_eq!((barra.min.x, barra.max.x), (0.0, 1000.0));
    assert!(
        barra.min.y >= 15.0 && barra.height() >= 20.0 && barra.height() <= 60.0,
        "{barra:?}"
    );
    let attesi: Vec<Cmd> = gruppi().into_iter().flatten().collect();
    let visti: Vec<Cmd> = f.pulsanti().iter().map(|p| p.cmd).collect();
    assert_eq!(visti, attesi);
    for p in f.pulsanti() {
        assert!(
            barra.contains_rect(p.rect),
            "{:?} fuori da {barra:?}",
            p.rect
        );
    }
    for w in f.pulsanti().windows(2) {
        assert!(
            w[0].rect.max.x <= w[1].rect.min.x + 0.5,
            "sovrapposti: {:?}",
            w
        );
    }
    // la griglia del testo sta sotto la barra
    let (o, _, _, _) = f.geometria().unwrap();
    assert!(o.y >= barra.max.y - 0.5, "{o:?} {barra:?}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn buttons_are_grey_when_the_command_makes_no_sense_and_toggles_light_up() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\n", false);
    disegna(&mut f, &ctx);
    for c in [
        Cmd::Save,
        Cmd::Undo,
        Cmd::Redo,
        Cmd::Cut,
        Cmd::Copy,
        Cmd::Paste,
    ] {
        assert!(!pulsante(&f, c).0, "{c:?} dovrebbe essere spento");
    }
    for c in [
        Cmd::New,
        Cmd::Open,
        Cmd::Find,
        Cmd::Replace,
        Cmd::Goto,
        Cmd::EditMode,
        Cmd::ColumnMode,
        Cmd::NavBar,
    ] {
        assert!(pulsante(&f, c).0, "{c:?} dovrebbe essere acceso");
    }
    for c in [Cmd::EditMode, Cmd::ColumnMode, Cmd::NavBar] {
        assert!(!pulsante(&f, c).1, "{c:?} parte non premuto");
    }
    // F4: modifica; un carattere: salva e annulla si accendono; F8: colonna; Ctrl+B: salti
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    giro(&mut f, &ctx, vec![tasto(Key::F8, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![tasto(Key::B, ctrl())]);
    disegna(&mut f, &ctx);
    assert!(pulsante(&f, Cmd::EditMode).1);
    assert!(pulsante(&f, Cmd::ColumnMode).1);
    assert!(pulsante(&f, Cmd::NavBar).1);
    assert!(pulsante(&f, Cmd::Save).0 && pulsante(&f, Cmd::Undo).0);
    assert!(pulsante(&f, Cmd::Paste).0, "in modifica si incolla");
    assert!(!pulsante(&f, Cmd::Redo).0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn clicking_a_button_runs_its_command() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\n", false);
    disegna(&mut f, &ctx);
    // Nuovo: una scheda senza nome
    let pos = centro(&f, Cmd::New);
    clic(&mut f, &ctx, pos);
    assert_eq!(f.workspace().len(), 2);
    assert!(f.app().is_untitled());
    disegna(&mut f, &ctx);
    // Cerca: il dialogo
    let pos = centro(&f, Cmd::Find);
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Find));
    f.workspace()
        .handle_key(ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Esc,
            ratatui::crossterm::event::KeyModifiers::NONE,
        ));
    disegna(&mut f, &ctx);
    // Modifica: passa a lettura (il documento senza nome era in modifica)
    assert!(f.app().is_edit());
    let pos = centro(&f, Cmd::EditMode);
    clic(&mut f, &ctx, pos);
    assert!(!f.app().is_edit());
    let _ = std::fs::remove_file(p);
}

#[test]
fn clicking_a_grey_button_does_nothing() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\n", false);
    disegna(&mut f, &ctx);
    let pos = centro(&f, Cmd::Undo);
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.workspace().len(), 1);
    assert!(f.app().flash().is_none());
    let pos = centro(&f, Cmd::Save);
    clic(&mut f, &ctx, pos);
    assert!(
        f.app().flash().is_none(),
        "spento: niente 'niente da salvare'"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_jump_strip_appears_with_ctrl_b_in_place_of_the_cell_bar() {
    let (mut f, ctx, p) = finestra(&molte_righe(), false);
    disegna(&mut f, &ctx);
    assert!(f.salti_rect().is_none());
    giro(&mut f, &ctx, vec![tasto(Key::B, ctrl())]);
    disegna(&mut f, &ctx);
    let strisce = f.salti_rect().unwrap();
    let toolbar = f.strumenti_rect().unwrap();
    assert!(
        strisce.min.y >= toolbar.max.y - 0.5,
        "i salti stanno sotto gli strumenti"
    );
    assert_eq!((strisce.min.x, strisce.max.x), (0.0, 1000.0));
    let attese = salti(f.workspace().jump_pages());
    let viste: Vec<String> = f
        .pulsanti_salti()
        .iter()
        .map(|(_, s)| s.etichetta.clone())
        .collect();
    let etichette: Vec<String> = attese.iter().map(|s| s.etichetta.clone()).collect();
    assert_eq!(viste, etichette);
    for (r, _) in f.pulsanti_salti() {
        assert!(strisce.contains_rect(*r));
    }
    // la barra a celle del terminale non si disegna: la prima riga del testo e' la prima della griglia
    let righe = f.workspace().render_lines(80, 24);
    assert!(!righe[0].starts_with("[Home]"), "{:?}", righe[0]);
    // e Ctrl+B la spegne
    giro(&mut f, &ctx, vec![tasto(Key::B, ctrl())]);
    disegna(&mut f, &ctx);
    assert!(f.salti_rect().is_none());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_jump_buttons_jump_by_pages_of_the_grid() {
    let (mut f, ctx, p) = finestra(&molte_righe(), false);
    giro(&mut f, &ctx, vec![tasto(Key::B, ctrl())]);
    disegna(&mut f, &ctx);
    let (_, _, _, righe) = f.geometria().unwrap();
    let pagina = u64::from(righe) - 1;
    let cerca = |f: &Finestra, a: SaltoAzione| -> Pos2 {
        f.pulsanti_salti()
            .iter()
            .find(|(_, s)| s.azione == a)
            .unwrap()
            .0
            .center()
    };
    let pos = cerca(&f, SaltoAzione::Pagine(true, 10));
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().top(), 10 * pagina * RIGA, "+10 pagine");
    disegna(&mut f, &ctx);
    let pos = cerca(&f, SaltoAzione::Pagine(false, 1));
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().top(), 9 * pagina * RIGA, "-1 pagina");
    disegna(&mut f, &ctx);
    let pos = cerca(&f, SaltoAzione::Fine);
    clic(&mut f, &ctx, pos);
    assert!(f.app().top() > 29_000 * RIGA, "in fondo: {}", f.app().top());
    disegna(&mut f, &ctx);
    let pos = cerca(&f, SaltoAzione::Inizio);
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().top(), 0);
    let _ = std::fs::remove_file(p);
}
