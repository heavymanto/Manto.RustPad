//! La finestra in un contesto di egui senza finestra vera (E31): un `giro` con eventi finti. Si controlla che l'`App`
//! riceva tastiera, testo, incolla e rotella, che le copie vadano agli appunti, che il titolo e la chiusura funzionino
//! e che un file trascinato si apra. Il disegno non si guarda con gli occhi qui: basta che non vada in panico e che le
//! celle vengano da `render_grid` (provate in mrp-tui).

use eframe::egui::{
    self, Context, DroppedFile, Event, FullOutput, Key, Modifiers, MouseWheelUnit, OutputCommand,
    Pos2, RawInput, Rect, Vec2, ViewportCommand, ViewportEvent, ViewportId, ViewportInfo,
};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig, DialogKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(data: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_gui_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, data).unwrap();
    p
}

fn finestra(data: &[u8]) -> (Finestra, Context, PathBuf) {
    let p = file(data);
    let app = App::open(&p, AppConfig::default()).unwrap();
    let ctx = Context::default();
    let f = Finestra::da_app(app, Box::new(AppConfig::default));
    (f, ctx, p)
}

fn schermo() -> Option<Rect> {
    Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0)))
}

fn giro(f: &mut Finestra, ctx: &Context, eventi: Vec<Event>) -> FullOutput {
    let raw = RawInput {
        screen_rect: schermo(),
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

fn copie(out: &FullOutput) -> Vec<String> {
    out.platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

fn comandi(out: &FullOutput) -> Vec<ViewportCommand> {
    out.viewport_output
        .get(&ViewportId::ROOT)
        .map(|v| v.commands.clone())
        .unwrap_or_default()
}

/// Un giro in cui la finestra riceve la richiesta di chiusura.
fn giro_con_chiusura(f: &mut Finestra, ctx: &Context) -> FullOutput {
    let mut raw = RawInput {
        screen_rect: schermo(),
        ..Default::default()
    };
    raw.viewports.insert(
        ViewportId::ROOT,
        ViewportInfo {
            events: vec![ViewportEvent::Close],
            ..Default::default()
        },
    );
    ctx.run(raw, |ctx| f.giro(ctx))
}

#[test]
fn the_first_frame_sets_the_title_and_draws_without_panicking() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\n");
    let out = giro(&mut f, &ctx, vec![]);
    let nome = p.file_name().unwrap().to_string_lossy().to_string();
    let titolo = format!("{nome} - Manto.RustPad");
    assert!(
        comandi(&out)
            .iter()
            .any(|c| matches!(c, ViewportCommand::Title(t) if *t == titolo)),
        "{:?}",
        comandi(&out)
    );
    assert!(!out.shapes.is_empty(), "qualcosa e stato disegnato");
    // un secondo giro con lo stesso titolo non lo rimanda
    let out = giro(&mut f, &ctx, vec![]);
    assert!(!comandi(&out)
        .iter()
        .any(|c| matches!(c, ViewportCommand::Title(_))));
    let _ = std::fs::remove_file(p);
}

#[test]
fn typed_text_and_keys_reach_the_app() {
    let (mut f, ctx, p) = finestra(b"abc\n");
    giro(&mut f, &ctx, vec![]);
    // F4: modifica; poi il testo digitato
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    assert!(f.app().is_edit());
    let out = giro(&mut f, &ctx, vec![Event::Text("xy".into())]);
    assert_eq!(f.app().document_text(), b"xyabc\n");
    assert!(f.app().is_modified());
    // il titolo ha l'asterisco (nel giro in cui il documento diventa modificato)
    assert!(comandi(&out)
        .iter()
        .any(|c| matches!(c, ViewportCommand::Title(t) if t.contains(" * - "))));
    // le frecce e il testo nello stesso giro, in ordine
    giro(
        &mut f,
        &ctx,
        vec![
            tasto(Key::ArrowRight, Modifiers::NONE),
            Event::Text("Z".into()),
            tasto(Key::Backspace, Modifiers::NONE),
            tasto(Key::Delete, Modifiers::NONE),
        ],
    );
    assert_eq!(f.app().document_text(), b"xyac\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_letter_key_without_ctrl_does_not_type_twice() {
    // egui manda il tasto e il testo: solo il testo scrive
    let (mut f, ctx, p) = finestra(b"\n");
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    giro(
        &mut f,
        &ctx,
        vec![tasto(Key::Q, Modifiers::NONE), Event::Text("q".into())],
    );
    assert_eq!(f.app().document_text(), b"q\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn copy_goes_to_the_system_clipboard_and_paste_comes_from_it() {
    let (mut f, ctx, p) = finestra("caff\u{e8} \u{65e5}\u{672c}\n".as_bytes());
    giro(&mut f, &ctx, vec![]);
    let out = giro(&mut f, &ctx, vec![tasto(Key::A, ctrl()), Event::Copy]);
    // in lettura Ctrl+A non arriva oltre l'ultimo a-capo: l'a-capo finale non e nella copia
    assert_eq!(copie(&out), vec!["caff\u{e8} \u{65e5}\u{672c}".to_string()]);
    // incolla: in modifica, il testo degli appunti di sistema
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Paste("ZZ".into())]);
    assert!(f.app().document_text().windows(2).any(|w| w == b"ZZ"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_wheel_scrolls_the_text() {
    let testo: String = (0..200).map(|i| format!("riga {i}\n")).collect();
    let (mut f, ctx, p) = finestra(testo.as_bytes());
    giro(&mut f, &ctx, vec![]);
    assert!(f.app().wait_idle(Duration::from_secs(20)));
    let prima = f.app().top();
    giro(
        &mut f,
        &ctx,
        vec![Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: Vec2::new(0.0, -3.0),
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(f.app().top() > prima, "{} > {}", f.app().top(), prima);
    // su
    let giu = f.app().top();
    giro(
        &mut f,
        &ctx,
        vec![Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: Vec2::new(0.0, 3.0),
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(f.app().top() < giu);
    let _ = std::fs::remove_file(p);
}

#[test]
fn closing_a_modified_document_asks_first() {
    let (mut f, ctx, p) = finestra(b"abc\n");
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    let out = giro_con_chiusura(&mut f, &ctx);
    assert!(comandi(&out)
        .iter()
        .any(|c| matches!(c, ViewportCommand::CancelClose)));
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Confirm));
    let _ = std::fs::remove_file(p);
}

#[test]
fn closing_a_clean_document_lets_the_window_close() {
    let (mut f, ctx, p) = finestra(b"abc\n");
    giro(&mut f, &ctx, vec![]);
    let out = giro_con_chiusura(&mut f, &ctx);
    assert!(!comandi(&out)
        .iter()
        .any(|c| matches!(c, ViewportCommand::CancelClose)));
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn quitting_from_the_app_closes_the_window() {
    let (mut f, ctx, p) = finestra(b"abc\n");
    giro(&mut f, &ctx, vec![]);
    let out = giro(&mut f, &ctx, vec![tasto(Key::F10, Modifiers::NONE)]);
    assert!(comandi(&out)
        .iter()
        .any(|c| matches!(c, ViewportCommand::Close)));
    let _ = std::fs::remove_file(p);
}

fn giro_con_file_trascinato(f: &mut Finestra, ctx: &Context, percorso: &std::path::Path) {
    let raw = RawInput {
        screen_rect: schermo(),
        dropped_files: vec![DroppedFile {
            path: Some(percorso.to_path_buf()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| f.giro(ctx));
}

#[test]
fn a_dropped_file_opens_in_a_new_tab_even_with_unsaved_changes() {
    let (mut f, ctx, p) = finestra(b"uno\n");
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    let q = file(b"due\n");
    giro_con_file_trascinato(&mut f, &ctx, &q);
    assert_eq!(f.workspace().len(), 2);
    assert_eq!(
        f.app().file_name(),
        q.file_name().unwrap().to_string_lossy()
    );
    // un secondo file: un'altra scheda; lo stesso file ancora: nessuna scheda nuova, torna avanti quella
    let r = file(b"tre\n");
    giro_con_file_trascinato(&mut f, &ctx, &r);
    assert_eq!(f.workspace().len(), 3);
    giro_con_file_trascinato(&mut f, &ctx, &q);
    assert_eq!(f.workspace().len(), 3);
    assert_eq!(f.workspace().active_index(), 1);
    // la prima scheda ha ancora le sue modifiche
    f.workspace().select(0);
    assert!(f.app().is_modified());
    assert_eq!(f.app().document_text(), b"xuno\n");
    for x in [p, q, r] {
        let _ = std::fs::remove_file(x);
    }
}

#[test]
fn ctrl_plus_and_minus_change_the_text_size_without_reaching_the_app() {
    let (mut f, ctx, p) = finestra(b"\n");
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    for k in [Key::Plus, Key::Minus, Key::Num0, Key::Equals] {
        giro(&mut f, &ctx, vec![tasto(k, ctrl())]);
    }
    assert_eq!(f.app().document_text(), b"\n");
    let _ = std::fs::remove_file(p);
}

/// Il centro della cella (colonna, riga) della griglia dell'ultimo giro.
fn centro(f: &Finestra, colonna: u16, riga: u16) -> Pos2 {
    let (o, c, _, _) = f.geometria().expect("un giro e' stato fatto");
    Pos2::new(
        o.x + c.x * (f32::from(colonna) + 0.5),
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

#[test]
fn a_click_moves_the_cursor_and_dragging_selects() {
    let (mut f, ctx, p) = finestra(b"abc\ndef\n");
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    // il gutter e' di 2 celle ("abc\ndef\n" ha 8 byte: 1 cifra + il separatore)
    let g = 2;
    let clic = vec![
        premi(centro(&f, g + 2, 2), true, Modifiers::NONE),
        premi(centro(&f, g + 2, 2), false, Modifiers::NONE),
    ];
    giro(&mut f, &ctx, clic);
    assert_eq!(f.app().cursor(), 6);
    // trascinando: dalla prima riga alla seconda
    let trascina = vec![
        premi(centro(&f, g, 1), true, Modifiers::NONE),
        Event::PointerMoved(centro(&f, g + 2, 2)),
        premi(centro(&f, g + 2, 2), false, Modifiers::NONE),
    ];
    giro(&mut f, &ctx, trascina);
    assert_eq!(f.app().selection(), Some((0, 6)));
    // Maiusc+clic estende
    let estendi = vec![
        premi(centro(&f, g + 1, 1), true, Modifiers::NONE),
        premi(centro(&f, g + 1, 1), false, Modifiers::NONE),
        premi(centro(&f, g + 1, 2), true, Modifiers::SHIFT),
        premi(centro(&f, g + 1, 2), false, Modifiers::SHIFT),
    ];
    giro(&mut f, &ctx, estendi);
    assert_eq!(f.app().selection(), Some((1, 5)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn pointer_events_outside_the_grid_or_before_the_first_frame_do_nothing() {
    let (mut f, ctx, p) = finestra(b"abc\ndef\n");
    // prima del primo giro la geometria non c'e': il clic si ignora senza panico
    giro(
        &mut f,
        &ctx,
        vec![premi(Pos2::new(50.0, 50.0), true, Modifiers::NONE)],
    );
    giro(&mut f, &ctx, vec![]);
    giro(
        &mut f,
        &ctx,
        vec![
            premi(Pos2::new(-5.0, -5.0), true, Modifiers::NONE),
            premi(Pos2::new(-5.0, -5.0), false, Modifiers::NONE),
            Event::PointerMoved(Pos2::new(5.0, 5.0)),
        ],
    );
    assert_eq!(f.app().cursor(), 0);
    assert_eq!(f.app().selection(), None);
    let _ = std::fs::remove_file(p);
}

// ---- le schede nella finestra (E34) ----

fn finestra_con_schede() -> (Finestra, Context, Vec<PathBuf>) {
    let (mut f, ctx, p) = finestra(b"uno\n");
    let q = file(b"due\n");
    giro(&mut f, &ctx, vec![]);
    giro_con_file_trascinato(&mut f, &ctx, &q);
    giro(&mut f, &ctx, vec![]);
    (f, ctx, vec![p, q])
}

#[test]
fn ctrl_tab_and_ctrl_shift_tab_cycle_through_the_tabs_without_reaching_the_app() {
    let (mut f, ctx, file) = finestra_con_schede();
    assert_eq!(f.workspace().active_index(), 1);
    giro(&mut f, &ctx, vec![tasto(Key::Tab, ctrl())]);
    assert_eq!(
        f.workspace().active_index(),
        0,
        "dall'ultima si torna alla prima"
    );
    let mut m = ctrl();
    m.shift = true;
    giro(&mut f, &ctx, vec![tasto(Key::Tab, m)]);
    assert_eq!(f.workspace().active_index(), 1);
    // il tasto Tab non e' arrivato come tabulatore (siamo in lettura, ma il testo e' intatto)
    assert_eq!(f.app().document_text(), b"due\n");
    for x in file {
        let _ = std::fs::remove_file(x);
    }
}

#[test]
fn ctrl_w_closes_the_active_tab_and_asks_when_it_has_changes() {
    let (mut f, ctx, file) = finestra_con_schede();
    giro(&mut f, &ctx, vec![tasto(Key::W, ctrl())]);
    assert_eq!(f.workspace().len(), 1);
    assert_eq!(
        f.app().file_name(),
        file[0].file_name().unwrap().to_string_lossy()
    );
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    giro(&mut f, &ctx, vec![tasto(Key::W, ctrl())]);
    // l'ultima scheda con modifiche: chiede, non chiude
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Confirm));
    assert!(!f.workspace().should_quit());
    for x in file {
        let _ = std::fs::remove_file(x);
    }
}

#[test]
fn the_tab_strip_takes_space_only_with_more_than_one_tab() {
    let (mut f, ctx, p) = finestra(b"uno\n");
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    let una = f.geometria().unwrap().0.y;
    let q = file(b"due\n");
    giro_con_file_trascinato(&mut f, &ctx, &q);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    let due = f.geometria().unwrap().0.y;
    assert!(
        due > una + 10.0,
        "la barra delle schede sposta la griglia: {una} -> {due}"
    );
    for x in [p, q] {
        let _ = std::fs::remove_file(x);
    }
}

#[test]
fn clicking_a_tab_label_brings_that_tab_forward() {
    let (mut f, ctx, file) = finestra_con_schede();
    assert_eq!(f.workspace().active_index(), 1);
    let (o, _, _, _) = f.geometria().unwrap();
    // la barra delle schede e' la fascia appena sopra la griglia; la prima etichetta comincia a sinistra
    let pos = Pos2::new(24.0, o.y - 12.0);
    giro(
        &mut f,
        &ctx,
        vec![Event::PointerMoved(pos), premi(pos, true, Modifiers::NONE)],
    );
    giro(&mut f, &ctx, vec![premi(pos, false, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![]);
    assert_eq!(f.workspace().active_index(), 0);
    for x in file {
        let _ = std::fs::remove_file(x);
    }
}

#[test]
fn closing_the_window_with_changes_in_a_background_tab_asks_about_that_tab() {
    let (mut f, ctx, file) = finestra_con_schede();
    giro(&mut f, &ctx, vec![tasto(Key::F4, Modifiers::NONE)]);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    f.workspace().select(0);
    assert!(!f.app().is_modified());
    let out = giro_con_chiusura(&mut f, &ctx);
    assert!(comandi(&out)
        .iter()
        .any(|c| matches!(c, ViewportCommand::CancelClose)));
    // la scheda con modifiche e' avanti e chiede
    assert_eq!(f.workspace().active_index(), 1);
    assert_eq!(f.app().dialog_kind(), Some(DialogKind::Confirm));
    for x in file {
        let _ = std::fs::remove_file(x);
    }
}
