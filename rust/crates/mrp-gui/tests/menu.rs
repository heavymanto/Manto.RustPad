//! La barra dei menu e l'apertura dei file nella GUI (E33). Scritti da ARCH: gli attesi sono calcolati a mano. La
//! struttura dei menu si controlla come dato (chiavi del catalogo, scorciatoie scritte sulle voci); le azioni si
//! provano sulla finestra senza schermo, con un selettore di file finto al posto del dialogo nativo.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_core::lang::Lang;
use mrp_gui::fonts::{scrittura_della_lingua, Scrittura};
use mrp_gui::gui::Finestra;
use mrp_gui::menu::{Azione, Voce, MENU};
use mrp_tui::app::{App, AppConfig, DialogKind};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(data: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_menu_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, data).unwrap();
    p
}

fn voci() -> Vec<Voce> {
    MENU.iter()
        .flat_map(|(_, v)| v.iter().flatten().copied())
        .collect()
}

fn voce(chiave: &str) -> Voce {
    voci()
        .into_iter()
        .find(|v| v.chiave == chiave)
        .unwrap_or_else(|| panic!("manca la voce {chiave}"))
}

/// Cio' che i titoli e le voci dichiarano di essere: come si chiama la scorciatoia di un tasto.
fn nome_tasto(k: KeyEvent) -> String {
    let mut s = String::new();
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        s.push_str("Ctrl+");
    }
    match k.code {
        KeyCode::F(n) => s.push_str(&format!("F{n}")),
        KeyCode::PageUp => s.push_str("PgUp"),
        KeyCode::PageDown => s.push_str("PgDn"),
        KeyCode::Char(c) => s.push(c.to_ascii_uppercase()),
        altro => panic!("tasto non previsto: {altro:?}"),
    }
    s
}

#[test]
fn the_menus_are_file_edit_view_help_with_every_text_in_the_catalog() {
    let titoli: Vec<&str> = MENU.iter().map(|(t, _)| *t).collect();
    assert_eq!(
        titoli,
        [
            "gui.menu.file",
            "gui.menu.edit",
            "gui.menu.view",
            "gui.menu.help"
        ]
    );
    let en = Lang::english();
    for t in &titoli {
        assert!(en.get(t).is_some(), "{t} non e' nel catalogo");
    }
    let tutte = voci();
    assert_eq!(tutte.len(), 4 + 10 + 9 + 1);
    for v in &tutte {
        assert!(
            en.get(v.chiave).is_some(),
            "{} non e' nel catalogo",
            v.chiave
        );
    }
    // una chiave per voce
    let mut chiavi: Vec<&str> = tutte.iter().map(|v| v.chiave).collect();
    chiavi.sort_unstable();
    chiavi.dedup();
    assert_eq!(chiavi.len(), tutte.len());
}

#[test]
fn the_shortcut_written_on_a_menu_entry_is_the_key_it_sends() {
    for v in voci() {
        if let Azione::Tasto(k) = v.azione {
            assert_eq!(v.scorciatoia, nome_tasto(k), "{}", v.chiave);
        }
    }
    assert_eq!(voce("gui.open").scorciatoia, "Ctrl+O");
    assert_eq!(voce("gui.zoom_in").scorciatoia, "Ctrl++");
    assert_eq!(voce("gui.zoom_out").scorciatoia, "Ctrl+-");
    assert_eq!(voce("gui.zoom_reset").scorciatoia, "Ctrl+0");
}

#[test]
fn separators_never_start_end_or_double_up_a_menu() {
    for (titolo, v) in MENU {
        assert!(v.first().unwrap().is_some(), "{titolo}");
        assert!(v.last().unwrap().is_some(), "{titolo}");
        assert!(
            v.windows(2).all(|w| w[0].is_some() || w[1].is_some()),
            "{titolo}"
        );
    }
}

#[test]
fn the_menu_languages_need_the_right_font_script() {
    assert_eq!(scrittura_della_lingua("ja"), Scrittura::Giapponese);
    assert_eq!(
        scrittura_della_lingua("zh-CN"),
        Scrittura::CineseSemplificato
    );
    for c in ["en", "it", "de", "es", "pt-BR"] {
        assert_eq!(scrittura_della_lingua(c), Scrittura::Nessuna, "{c}");
    }
}

// ---- le azioni sulla finestra ----

type Titoli = Rc<RefCell<Vec<String>>>;

/// Una finestra su un file con `data`, il cui selettore restituisce uno dopo l'altro i percorsi di `scelte` (poi `None`)
/// e annota i titoli del dialogo che gli sono arrivati.
fn finestra(data: &[u8], scelte: Vec<PathBuf>) -> (Finestra, Context, PathBuf, Titoli) {
    let p = file(data);
    let app = App::open(&p, AppConfig::default()).unwrap();
    let titoli: Titoli = Rc::default();
    let t = titoli.clone();
    let mut coda = scelte.into_iter();
    let f = Finestra::da_app(app, Box::new(AppConfig::default)).con_selettore(Box::new(
        move |titolo| {
            t.borrow_mut().push(titolo.to_string());
            coda.next()
        },
    ));
    (f, Context::default(), p, titoli)
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

fn nome(p: &std::path::Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

#[test]
fn ctrl_o_asks_for_a_file_and_opens_it() {
    let q = file(b"due\n");
    let (mut f, ctx, p, titoli) = finestra(b"uno\n", vec![q.clone()]);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![tasto(Key::O, ctrl())]);
    assert_eq!(f.app().file_name(), nome(&q));
    assert_eq!(*titoli.borrow(), ["Open file"]);
    // Ctrl+O non arriva all'App come tasto: il documento resta intatto e in lettura
    assert!(!f.app().is_edit());
    for x in [p, q] {
        let _ = std::fs::remove_file(x);
    }
}

#[test]
fn cancelling_the_open_dialog_leaves_the_document_alone() {
    let (mut f, ctx, p, titoli) = finestra(b"uno\n", vec![]);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![tasto(Key::O, ctrl())]);
    assert_eq!(f.app().file_name(), nome(&p));
    assert_eq!(titoli.borrow().len(), 1);
    let _ = std::fs::remove_file(p);
}

#[test]
fn opening_with_unsaved_changes_opens_a_new_tab_and_keeps_the_edits() {
    let q = file(b"due\n");
    let (mut f, ctx, p, titoli) = finestra(b"uno\n", vec![q.clone()]);
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    assert!(f.app().is_modified());
    f.azione(Azione::Apri);
    assert_eq!(titoli.borrow().len(), 1);
    assert_eq!(f.workspace().len(), 2);
    assert_eq!(f.app().file_name(), nome(&q));
    f.workspace().select(0);
    assert_eq!(f.app().file_name(), nome(&p));
    assert!(f.app().is_modified());
    assert_eq!(f.app().document_text(), b"xuno\n");
    for x in [p, q] {
        let _ = std::fs::remove_file(x);
    }
}

#[test]
fn a_file_that_cannot_be_opened_is_reported_and_the_old_one_stays() {
    let mancante = std::env::temp_dir().join("mrp_menu_test_non_esiste.txt");
    let _ = std::fs::remove_file(&mancante);
    let (mut f, ctx, p, _t) = finestra(b"uno\n", vec![mancante.clone()]);
    giro(&mut f, &ctx, vec![]);
    f.azione(Azione::Apri);
    assert_eq!(f.app().file_name(), nome(&p));
    let avviso = f.app().flash().expect("un avviso").to_string();
    assert!(avviso.contains("mrp_menu_test_non_esiste.txt"), "{avviso}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_menu_entries_do_what_their_shortcuts_do() {
    let (mut f, ctx, p, _t) = finestra(b"abc\ndef\n", vec![]);
    giro(&mut f, &ctx, vec![]);
    let esegui = |f: &mut Finestra, chiave: &str| f.azione(voce(chiave).azione);
    esegui(&mut f, "gui.edit_mode");
    assert!(f.app().is_edit());
    esegui(&mut f, "gui.edit_mode");
    assert!(!f.app().is_edit());
    esegui(&mut f, "gui.select_all");
    // in lettura l'ultimo a-capo non si seleziona
    assert_eq!(f.app().selection(), Some((0, 7)));
    esegui(&mut f, "gui.help");
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Help));
    f.azione(Azione::Tasto(KeyEvent::new(
        KeyCode::Esc,
        KeyModifiers::NONE,
    )));
    for (chiave, dialogo) in [
        ("gui.find", DialogKind::Find),
        ("gui.goto", DialogKind::Goto),
        ("gui.theme", DialogKind::Theme),
        ("gui.language", DialogKind::Lang),
        ("gui.encoding", DialogKind::Encoding),
    ] {
        esegui(&mut f, chiave);
        assert_eq!(f.app().dialog_kind(), Some(dialogo), "{chiave}");
        f.azione(Azione::Tasto(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert_eq!(f.app().dialog_kind(), None, "{chiave}");
    }
    esegui(&mut f, "gui.column_mode");
    assert!(f.app().column_mode());
    esegui(&mut f, "gui.column_mode");
    assert!(!f.app().column_mode());
    esegui(&mut f, "gui.quit");
    assert!(f.workspace().should_quit());
    let _ = std::fs::remove_file(p);
}

#[test]
fn undo_and_redo_from_the_menu_walk_the_history() {
    let (mut f, ctx, p, _t) = finestra(b"abc\n", vec![]);
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Text("X".into())]);
    assert_eq!(f.app().document_text(), b"Xabc\n");
    f.azione(voce("gui.undo").azione);
    assert_eq!(f.app().document_text(), b"abc\n");
    f.azione(voce("gui.redo").azione);
    assert_eq!(f.app().document_text(), b"Xabc\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_zoom_entries_change_the_text_size_within_limits() {
    let (mut f, _ctx, p, _t) = finestra(b"x\n", vec![]);
    let base = f.corpo();
    f.azione(voce("gui.zoom_in").azione);
    assert_eq!(f.corpo(), base + 1.0);
    f.azione(voce("gui.zoom_out").azione);
    f.azione(voce("gui.zoom_out").azione);
    assert_eq!(f.corpo(), base - 1.0);
    f.azione(voce("gui.zoom_reset").azione);
    assert_eq!(f.corpo(), base);
    for _ in 0..100 {
        f.azione(Azione::ZoomPiu);
    }
    assert_eq!(f.corpo(), 48.0);
    for _ in 0..100 {
        f.azione(Azione::ZoomMeno);
    }
    assert_eq!(f.corpo(), 8.0);
    let _ = std::fs::remove_file(p);
}

// ---- il clic su un menu aperto non e' un clic sul testo ----

fn pigia(pos: Pos2, premuto: bool) -> Event {
    Event::PointerButton {
        pos,
        button: eframe::egui::PointerButton::Primary,
        pressed: premuto,
        modifiers: Modifiers::NONE,
    }
}

/// Un clic completo (muovi, premi) in un giro e il rilascio nel successivo.
fn clic(f: &mut Finestra, ctx: &Context, pos: Pos2) {
    giro(f, ctx, vec![Event::PointerMoved(pos), pigia(pos, true)]);
    giro(f, ctx, vec![pigia(pos, false)]);
    giro(f, ctx, vec![]);
}

#[test]
fn a_click_on_an_open_menu_entry_does_not_reach_the_text_underneath() {
    // il selettore annulla: l'App resta quella, e il cursore dice se il clic e' arrivato al testo
    let (mut f, ctx, p, titoli) = finestra(b"abc\ndef\nghi\n", vec![]);
    // il corpo piccolo: le righe sono basse, quindi la voce del menu sta sopra una riga di testo e non sul titolo
    for _ in 0..10 {
        f.azione(Azione::ZoomMeno);
    }
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    // il menu File si apre cliccando sul suo titolo, in alto a sinistra
    clic(&mut f, &ctx, Pos2::new(14.0, 10.0));
    // la prima voce: il primo punto dove sta qualcosa sopra lo sfondo, a 14 pixel dentro
    let primo = (0..300)
        .map(|y| Pos2::new(20.0, y as f32))
        .find(|&p| {
            ctx.layer_id_at(p)
                .is_some_and(|l| l.order == eframe::egui::Order::Foreground)
        })
        .expect("il menu e' aperto");
    let voce = Pos2::new(60.0, primo.y + 14.0);
    let (o, c, colonne, righe) = f.geometria().unwrap();
    // senza il menu quel punto sarebbe un clic vero su una riga di testo, ben dentro la riga
    let (col, riga) =
        mrp_gui::input::cella_sotto(voce, o, c, colonne, righe).expect("sulla griglia");
    assert!(
        riga >= 1 && col >= 6,
        "{voce:?} cade sulla cella {col},{riga}"
    );
    clic(&mut f, &ctx, voce);
    assert_eq!(titoli.borrow().len(), 1, "la voce Apri e' stata scelta");
    assert_eq!(f.app().cursor(), 0, "il clic non e' arrivato al testo");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_tab_entries_switch_and_close_tabs() {
    let q = file(
        b"due
",
    );
    let (mut f, ctx, p, _t) = finestra(
        b"uno
",
        vec![q.clone()],
    );
    giro(&mut f, &ctx, vec![]);
    f.azione(Azione::Apri);
    assert_eq!(f.workspace().active_index(), 1);
    f.azione(voce("gui.next_tab").azione);
    assert_eq!(f.workspace().active_index(), 0);
    f.azione(voce("gui.prev_tab").azione);
    assert_eq!(f.workspace().active_index(), 1);
    f.azione(voce("gui.close_tab").azione);
    assert_eq!(f.workspace().len(), 1);
    assert_eq!(f.app().file_name(), nome(&p));
    for x in [p, q] {
        let _ = std::fs::remove_file(x);
    }
}
