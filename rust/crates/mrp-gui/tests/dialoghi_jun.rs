//! I test del JUN per i dialoghi della finestra (B4b): i casi che il file di ARCH (`dialoghi.rs`) non copre, con gli
//! attesi calcolati a mano dal PLAN. Come li': un contesto di egui senza schermo, un giro alla volta con eventi finti.

use eframe::egui::{Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::dialogs::DialogView;
use mrp_tui::theme::ThemeId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!(
        "mrp_dialoghi_jun_test_{}_{}.txt",
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

/// Due giri: il primo apre la finestra, il secondo la mette a posto (il campo prende il fuoco).
fn disegna(f: &mut Finestra, ctx: &Context) {
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
}

fn premi(f: &mut Finestra, ctx: &Context, key: Key) {
    giro(f, ctx, vec![tasto(key, Modifiers::NONE)]);
    giro(f, ctx, vec![]);
}

/// Home e End portano l'evidenza alla prima e all'ultima voce del menu del tema (parte da Console, la terza di sei).
#[test]
fn home_and_end_go_to_the_first_and_last_menu_voice() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F6);
    disegna(&mut f, &ctx);
    let selezionata = |f: &mut Finestra| match f.app().dialog_view().unwrap() {
        DialogView::Menu { selected, .. } => selected,
        altro => panic!("{altro:?}"),
    };
    assert_eq!(selezionata(&mut f), 2, "parte dal tema corrente (Console)");
    premi(&mut f, &ctx, Key::Home);
    assert_eq!(selezionata(&mut f), 0, "Home: la prima voce");
    premi(&mut f, &ctx, Key::End);
    assert_eq!(selezionata(&mut f), 5, "End: l'ultima voce (sei temi)");
    let _ = std::fs::remove_file(p);
}

/// Un clic sulla parte destra di una riga del menu (il dettaglio, non il nome) sceglie lo stesso quella voce: la riga
/// intera e' della voce.
#[test]
fn a_click_on_the_detail_part_of_a_menu_row_picks_it() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F6);
    disegna(&mut f, &ctx);
    let riga = f.dialogo_voci()[0];
    let pos = Pos2::new(riga.right() - 2.0, riga.center().y);
    assert!(riga.contains(pos), "{riga:?} contiene {pos:?}");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().theme_id(), ThemeId::Night, "la prima voce e' Night");
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

/// Il menu delle lingue mostra tutte le lingue del registro (sette nel repo: l'inglese incorporato primo, le altre in
/// ordine di codice) e un clic ne applica una.
#[test]
fn the_language_menu_lists_the_registry_and_a_click_applies_one() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang");
    let (lingue, _) = mrp_core::lang::registry_in(Some(dir.as_path()));
    let p = file(b"uno\n");
    let c = AppConfig {
        chrome: false,
        langs: lingue,
        ..AppConfig::default()
    };
    let mut app = App::open(&p, c.clone()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(move || c.clone()));
    let ctx = Context::default();
    premi(&mut f, &ctx, Key::F7);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("LANGUAGE"));
    assert_eq!(
        f.dialogo_voci().len(),
        7,
        "de en es it ja pt-BR zh-CN, in ordine di codice"
    );
    let pos = f.dialogo_voci()[1].center();
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().lang_code(), "de", "la seconda voce e' il tedesco");
    assert_eq!(f.app().dialog_kind(), None);
    let _ = std::fs::remove_file(p);
}

/// Il campo del "vai a" riparte dal testo dell'App a ogni apertura (vuoto: non ricorda quello di prima) e Esc chiude
/// senza muovere il cursore.
#[test]
fn the_goto_field_starts_fresh_on_each_open_and_escape_cancels() {
    let (mut f, ctx, p) = finestra(b"uno\ndue\ntre\nquattro\n", false);
    giro(&mut f, &ctx, vec![tasto(Key::G, ctrl())]);
    disegna(&mut f, &ctx);
    giro(&mut f, &ctx, vec![Event::Text("2".into())]);
    giro(&mut f, &ctx, vec![]);
    premi(&mut f, &ctx, Key::Enter);
    assert_eq!(
        f.app().cursor(),
        4,
        "la riga 2 comincia al byte 4: \"uno\\n\" sono 4 byte"
    );
    // di nuovo: il campo riparte vuoto, non da "2"
    giro(&mut f, &ctx, vec![tasto(Key::G, ctrl())]);
    disegna(&mut f, &ctx);
    giro(&mut f, &ctx, vec![Event::Text("5".into())]);
    giro(&mut f, &ctx, vec![]);
    match f.app().dialog_view().unwrap() {
        DialogView::Prompt { text, .. } => {
            assert_eq!(
                text, "5",
                "il campo mostra il testo dell'App, non quello di prima"
            )
        }
        altro => panic!("{altro:?}"),
    }
    premi(&mut f, &ctx, Key::Escape);
    assert_eq!(f.app().dialog_kind(), None);
    assert_eq!(f.app().cursor(), 4, "annullato: il cursore non si e' mosso");
    let _ = std::fs::remove_file(p);
}

/// I pulsanti di un dialogo stanno da sinistra a destra nell'ordine del PLAN: la conferma di uscita tre (salva,
/// scarta, annulla), il prompt due (OK, annulla).
#[test]
fn the_dialog_buttons_lie_left_to_right() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    premi(&mut f, &ctx, Key::F12);
    giro(&mut f, &ctx, vec![Event::Text("x".into())]);
    f.workspace().active_mut().request_close();
    disegna(&mut f, &ctx);
    let pulsanti = f.dialogo_pulsanti();
    assert_eq!(
        pulsanti.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(),
        vec!["Save and quit", "Discard changes", "Cancel"]
    );
    assert!(
        pulsanti[0].0.max.x <= pulsanti[1].0.min.x && pulsanti[1].0.max.x <= pulsanti[2].0.min.x,
        "da sinistra a destra: {pulsanti:?}"
    );
    premi(&mut f, &ctx, Key::Escape);
    giro(&mut f, &ctx, vec![tasto(Key::G, ctrl())]);
    disegna(&mut f, &ctx);
    let pulsanti = f.dialogo_pulsanti();
    assert_eq!(
        pulsanti.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(),
        vec!["OK", "Cancel"]
    );
    assert!(
        pulsanti[0].0.max.x <= pulsanti[1].0.min.x,
        "OK sta a sinistra di Cancel: {pulsanti:?}"
    );
    let _ = std::fs::remove_file(p);
}

/// Chiuso il dialogo, tutto cio' che se ne ricordava il disegno si azzera: rettangolo, titolo, pulsanti, voci e campo.
#[test]
fn closing_the_dialog_clears_the_drawn_state() {
    let (mut f, ctx, p) = finestra(b"uno\n", false);
    giro(&mut f, &ctx, vec![tasto(Key::G, ctrl())]);
    disegna(&mut f, &ctx);
    assert_eq!(f.dialogo_titolo(), Some("GO TO"));
    assert!(f.dialogo_rect().is_some() && f.dialogo_campo().is_some());
    assert_eq!(f.dialogo_pulsanti().len(), 2);
    premi(&mut f, &ctx, Key::Escape);
    disegna(&mut f, &ctx);
    assert!(f.dialogo_rect().is_none() && f.dialogo_titolo().is_none());
    assert!(f.dialogo_campo().is_none());
    assert!(f.dialogo_pulsanti().is_empty() && f.dialogo_voci().is_empty());
    let _ = std::fs::remove_file(p);
}
