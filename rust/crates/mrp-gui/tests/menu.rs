//! La barra dei menu e l'apertura dei file nella GUI (E33). Scritti da ARCH: gli attesi sono calcolati a mano. La
//! struttura dei menu si controlla come dato (chiavi del catalogo, scorciatoie scritte sulle voci); le azioni si
//! provano sulla finestra senza schermo, con un selettore di file finto al posto del dialogo nativo.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_core::lang::Lang;
use mrp_gui::fonts::{scrittura_della_lingua, Scrittura};
use mrp_gui::gui::Finestra;
use mrp_gui::menu::{acceso, voce_di, Voce, MENU};
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::commands::{shortcut_text, Cmd};
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
        .flat_map(|(_, v)| v.iter().flatten().map(|c| voce_di(*c)))
        .collect()
}

fn voce(chiave: &str) -> Voce {
    voci()
        .into_iter()
        .find(|v| v.chiave == chiave)
        .unwrap_or_else(|| panic!("manca la voce {chiave}"))
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
            "gui.menu.log",
            "gui.menu.help"
        ]
    );
    let en = Lang::english();
    for t in &titoli {
        assert!(en.get(t).is_some(), "{t} non e' nel catalogo");
    }
    let tutte = voci();
    assert_eq!(tutte.len(), 6 + 10 + 12 + 7 + 1);
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
        if let Some(k) = v.azione.info().key {
            // Trova, Sostituisci e Lettura / modifica hanno nella finestra i tasti standard (F3, F4, F12)
            if matches!(v.azione, Cmd::Find | Cmd::Replace | Cmd::EditMode) {
                continue;
            }
            assert_eq!(v.scorciatoia, shortcut_text(k), "{}", v.chiave);
        }
    }
    assert_eq!(voce("gui.find").scorciatoia, "F3");
    assert_eq!(voce("gui.replace").scorciatoia, "F4");
    assert_eq!(voce("gui.edit_mode").scorciatoia, "F12");
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
    giro(&mut f, &ctx, vec![tasto(Key::F12, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    assert!(f.app().is_modified());
    f.azione(Cmd::Open);
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
    f.azione(Cmd::Open);
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
    f.workspace()
        .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    for (chiave, dialogo) in [
        ("gui.find", DialogKind::Find),
        ("gui.goto", DialogKind::Goto),
        ("gui.encoding", DialogKind::Encoding),
    ] {
        esegui(&mut f, chiave);
        assert_eq!(f.app().dialog_kind(), Some(dialogo), "{chiave}");
        f.workspace()
            .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
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
    giro(&mut f, &ctx, vec![tasto(Key::F12, Modifiers::NONE)]);
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
        f.azione(Cmd::ZoomIn);
    }
    assert_eq!(f.corpo(), 48.0);
    for _ in 0..100 {
        f.azione(Cmd::ZoomOut);
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
        f.azione(Cmd::ZoomOut);
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
    // la prima voce del menu File e' Nuovo: apre una scheda senza nome
    assert_eq!(
        f.workspace().len(),
        2,
        "la prima voce (Nuovo) e' stata scelta"
    );
    assert!(titoli.borrow().is_empty());
    assert_eq!(
        f.workspace().tabs()[0].cursor(),
        0,
        "il clic non e' arrivato al testo"
    );
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
    f.azione(Cmd::Open);
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

#[test]
fn the_jump_bar_entry_shows_and_hides_the_bar() {
    let (mut f, ctx, p, _t) = finestra(b"uno\n", vec![]);
    giro(&mut f, &ctx, vec![]);
    assert!(!f.workspace().nav_bar());
    f.azione(voce("gui.nav_bar").azione);
    assert!(f.workspace().nav_bar());
    f.azione(voce("gui.nav_bar").azione);
    assert!(!f.workspace().nav_bar());
    assert_eq!(voce("gui.nav_bar").scorciatoia, "Ctrl+B");
    let _ = std::fs::remove_file(p);
}

// ---- documento nuovo e salva con nome (E42b) ----

type Chiamate = Rc<RefCell<Vec<(String, String)>>>;

/// Una finestra su un documento senza nome, il cui selettore di salvataggio restituisce uno dopo l'altro i percorsi di
/// `scelte` (poi `None`) e annota (titolo, nome suggerito).
fn finestra_nuova(scelte: Vec<PathBuf>) -> (Finestra, Context, PathBuf, Chiamate) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("mrp_menu_nuovo_{}_{}", std::process::id(), n));
    let app = App::new_untitled(&dir.join("untitled"), 1, AppConfig::default()).unwrap();
    let chiamate: Chiamate = Rc::default();
    let c = chiamate.clone();
    let mut coda = scelte.into_iter();
    let f = Finestra::da_app(app, Box::new(AppConfig::default)).con_salvataggio(Box::new(
        move |titolo, nome| {
            c.borrow_mut().push((titolo.to_string(), nome.to_string()));
            coda.next()
        },
    ));
    (f, Context::default(), dir, chiamate)
}

#[test]
fn the_file_menu_has_new_and_save_as_with_their_shortcuts() {
    assert_eq!(voce("gui.new").scorciatoia, "Ctrl+N");
    assert_eq!(voce("gui.save_as").scorciatoia, "Ctrl+Shift+S");
    assert_eq!(voce("gui.save_as").azione, Cmd::SaveAs);
    // sono nel menu File, dopo Apri
    let file_menu: Vec<&str> = MENU[0].1.iter().flatten().map(|c| c.info().label).collect();
    assert_eq!(file_menu[0], "gui.new");
    assert_eq!(file_menu[1], "gui.open");
    assert!(file_menu.contains(&"gui.save_as"));
}

#[test]
fn the_new_entry_opens_an_untitled_tab() {
    let (mut f, ctx, p, _t) = finestra(b"uno\n", vec![]);
    giro(&mut f, &ctx, vec![]);
    f.azione(voce("gui.new").azione);
    assert_eq!(f.workspace().len(), 2);
    assert!(f.app().is_untitled());
    assert_eq!(f.app().file_name(), "Untitled 1");
    // Ctrl+N dalla tastiera fa lo stesso
    giro(&mut f, &ctx, vec![tasto(Key::N, ctrl())]);
    assert_eq!(f.workspace().len(), 3);
    assert_eq!(f.app().file_name(), "Untitled 2");
    let _ = std::fs::remove_file(p);
}

#[test]
fn ctrl_s_on_an_untitled_document_asks_the_system_dialog_for_a_name_and_saves_there() {
    let dir0 = std::env::temp_dir().join(format!("mrp_menu_dest_{}", std::process::id()));
    std::fs::create_dir_all(&dir0).unwrap();
    let meta = dir0.join("nota.txt");
    let (mut f, ctx, dir, chiamate) = finestra_nuova(vec![meta.clone()]);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![Event::Text("ciao".into())]);
    giro(&mut f, &ctx, vec![tasto(Key::S, ctrl())]);
    // il dialogo e' quello del sistema, non il prompt a testo dell'App
    assert_ne!(f.app().dialog_kind(), Some(DialogKind::SaveAs));
    assert_eq!(
        chiamate.borrow().as_slice(),
        [("Save as".to_string(), "Untitled 1.txt".to_string())]
    );
    // scelto il nome, il dialogo della codifica: la stessa del documento, si conferma
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::SaveEncoding));
    f.workspace().active_mut().dialog_submit();
    assert!(f.app().wait_idle(std::time::Duration::from_secs(20)));
    assert_eq!(std::fs::read(&meta).unwrap(), b"ciao");
    assert!(!f.app().is_untitled());
    assert_eq!(f.app().file_name(), "nota.txt");
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(dir0);
}

#[test]
fn f2_and_the_save_menu_entry_do_the_same_on_an_untitled_document() {
    let dir0 = std::env::temp_dir().join(format!("mrp_menu_dest2_{}", std::process::id()));
    std::fs::create_dir_all(&dir0).unwrap();
    let (mut f, ctx, dir, chiamate) = finestra_nuova(vec![dir0.join("a.txt"), dir0.join("b.txt")]);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    giro(&mut f, &ctx, vec![tasto(Key::F2, Modifiers::NONE)]);
    assert_eq!(chiamate.borrow().len(), 1);
    // scelto il nome, il dialogo della codifica: la stessa del documento, si conferma
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::SaveEncoding));
    f.workspace().active_mut().dialog_submit();
    assert!(f.app().wait_idle(std::time::Duration::from_secs(20)));
    assert_eq!(std::fs::read(dir0.join("a.txt")).unwrap(), b"x");
    // una scheda nuova: la voce Salva del menu (che manda F2) passa dallo stesso dialogo
    f.azione(voce("gui.new").azione);
    giro(&mut f, &ctx, vec![Event::Text("y".into())]);
    f.azione(voce("gui.save").azione);
    assert_eq!(chiamate.borrow().len(), 2);
    // scelto il nome, il dialogo della codifica: la stessa del documento, si conferma
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::SaveEncoding));
    f.workspace().active_mut().dialog_submit();
    assert!(f.app().wait_idle(std::time::Duration::from_secs(20)));
    assert_eq!(std::fs::read(dir0.join("b.txt")).unwrap(), b"y");
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(dir0);
}

#[test]
fn cancelling_the_save_dialog_keeps_the_document_as_it_was() {
    let (mut f, ctx, dir, chiamate) = finestra_nuova(vec![]);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![Event::Text("dati".into())]);
    giro(&mut f, &ctx, vec![tasto(Key::S, ctrl())]);
    assert_eq!(chiamate.borrow().len(), 1);
    assert!(f.app().is_untitled() && f.app().is_modified());
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.app().document_text(), b"dati");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_untitled_document_with_nothing_typed_has_nothing_to_save_and_asks_nothing() {
    let (mut f, ctx, dir, chiamate) = finestra_nuova(vec![]);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![tasto(Key::S, ctrl())]);
    assert!(chiamate.borrow().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn ctrl_s_on_a_normal_file_saves_without_the_dialog_and_save_as_copies_it() {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir0 = std::env::temp_dir().join(format!("mrp_menu_copia_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir0).unwrap();
    let copia = dir0.join("copia.txt");
    let p = file(b"uno\n");
    let app = App::open(&p, AppConfig::default()).unwrap();
    let chiamate: Chiamate = Rc::default();
    let c = chiamate.clone();
    let scelta = copia.clone();
    let mut f = Finestra::da_app(app, Box::new(AppConfig::default)).con_salvataggio(Box::new(
        move |t, n| {
            c.borrow_mut().push((t.to_string(), n.to_string()));
            Some(scelta.clone())
        },
    ));
    let ctx = Context::default();
    giro(&mut f, &ctx, vec![tasto(Key::F12, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Text("+".into())]);
    giro(&mut f, &ctx, vec![tasto(Key::S, ctrl())]);
    assert!(
        chiamate.borrow().is_empty(),
        "un file vero si salva e basta"
    );
    assert!(f.app().wait_idle(std::time::Duration::from_secs(20)));
    assert_eq!(std::fs::read(&p).unwrap(), b"+uno\n");
    // Salva con nome (dal menu, o Ctrl+Maiusc+S): il dialogo di sistema, il nome suggerito e' quello del file
    giro(&mut f, &ctx, vec![Event::Text("!".into())]);
    f.azione(Cmd::SaveAs);
    assert_eq!(chiamate.borrow().len(), 1);
    assert_eq!(chiamate.borrow()[0].1, nome(&p));
    // scelto il nome, il dialogo della codifica: la stessa del documento, si conferma
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::SaveEncoding));
    f.workspace().active_mut().dialog_submit();
    assert!(f.app().wait_idle(std::time::Duration::from_secs(20)));
    // il cursore era dopo il '+': il '!' sta li'
    assert_eq!(std::fs::read(&copia).unwrap(), b"+!uno\n");
    assert_eq!(f.app().file_name(), "copia.txt");
    assert_eq!(
        std::fs::read(&p).unwrap(),
        b"+uno\n",
        "il file di partenza resta com'era"
    );
    let maiusc = Modifiers {
        shift: true,
        ..ctrl()
    };
    giro(&mut f, &ctx, vec![tasto(Key::S, maiusc)]);
    assert_eq!(chiamate.borrow().len(), 2);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::SaveEncoding));
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_dir_all(dir0);
}

#[test]
fn the_view_menu_is_about_the_current_work_and_theme_and_language_live_in_the_settings() {
    let vista: Vec<Cmd> = MENU[2].1.iter().flatten().copied().collect();
    assert!(!vista.contains(&Cmd::Theme));
    assert!(!vista.contains(&Cmd::Language));
    for c in [Cmd::Settings, Cmd::Encoding, Cmd::ZoomIn, Cmd::LineNumbers] {
        assert!(vista.contains(&c), "{c:?} resta in Visualizza");
    }
}

#[test]
fn the_toggle_entries_know_whether_they_are_on() {
    let (mut f, ctx, p, _t) = finestra(b"abc\n", vec![]);
    giro(&mut f, &ctx, vec![]);
    // di fabbrica: numeri di riga accesi, il resto spento (la barra dei salti segue la scelta di chi apre)
    assert_eq!(acceso(f.workspace(), Cmd::LineNumbers), Some(true));
    let barra = f.workspace().nav_bar();
    assert_eq!(acceso(f.workspace(), Cmd::NavBar), Some(barra));
    f.azione(Cmd::NavBar);
    assert_eq!(acceso(f.workspace(), Cmd::NavBar), Some(!barra));
    assert_eq!(acceso(f.workspace(), Cmd::ColumnMode), Some(false));
    assert_eq!(acceso(f.workspace(), Cmd::Special), Some(false));
    f.azione(Cmd::LineNumbers);
    f.azione(Cmd::ColumnMode);
    f.azione(Cmd::Special);
    assert_eq!(acceso(f.workspace(), Cmd::LineNumbers), Some(false));
    assert_eq!(acceso(f.workspace(), Cmd::ColumnMode), Some(true));
    assert_eq!(acceso(f.workspace(), Cmd::Special), Some(true));
    // un comando qualunque non e' un interruttore
    assert_eq!(acceso(f.workspace(), Cmd::Find), None);
    let _ = std::fs::remove_file(p);
}
