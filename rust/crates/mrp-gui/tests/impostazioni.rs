//! La finestra delle impostazioni (Visualizza > Impostazioni, Ctrl+,): tutte le personalizzazioni in un posto, ognuna col suo
//! tasto scritto accanto, e una scheda con tutti i tasti. Le scelte fanno quello che fanno i menu e i tasti (e si salvano). Scritti
//! da ARCH.

use eframe::egui::{self, Context, Event, FullOutput, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use mrp_gui::gui::Finestra;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::commands::{Cmd, COMMANDS};
use mrp_tui::theme::ThemeId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn percorsi() -> (PathBuf, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_impost_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    (d.join("a.txt"), d.join("profilo").join("settings.txt"))
}

fn finestra() -> (Finestra, Context, PathBuf, PathBuf) {
    let (p, settings) = percorsi();
    std::fs::write(&p, b"uno\ndue\n").unwrap();
    let cfg = {
        let s = settings.clone();
        move || AppConfig {
            chrome: false,
            settings_path: Some(s.clone()),
            // l'accensione dell'interruttore si prova partendo dallo spento
            log_colors: false,
            ..AppConfig::default()
        }
    };
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    (f, ctx, p, settings)
}

fn giro(f: &mut Finestra, ctx: &Context, eventi: Vec<Event>) -> FullOutput {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 800.0))),
        events: eventi,
        ..Default::default()
    };
    ctx.run(raw, |ctx| f.giro(ctx))
}

fn ctrl_virgola() -> Event {
    Event::Key {
        key: Key::Comma,
        physical_key: Some(Key::Comma),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    }
}

fn apri(f: &mut Finestra, ctx: &Context) {
    giro(f, ctx, vec![ctrl_virgola()]);
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
    assert!(f.impostazioni_aperta());
}

fn clic(f: &mut Finestra, ctx: &Context, pos: Pos2) {
    for premuto in [true, false] {
        giro(
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
    }
    giro(f, ctx, vec![]);
    giro(f, ctx, vec![]);
}

/// Sceglie un tema dalla tendina delle impostazioni: apre la tendina e clicca la voce.
fn scegli_tema(f: &mut Finestra, ctx: &Context, nome: &str) {
    let combo = controllo(f, "tema");
    clic(f, ctx, combo);
    let voce = controllo(f, nome);
    clic(f, ctx, voce);
}

fn controllo(f: &Finestra, testo: &str) -> Pos2 {
    f.impostazioni_controlli()
        .iter()
        .find(|(_, t)| t == testo)
        .unwrap_or_else(|| panic!("manca {testo:?} in {:?}", f.impostazioni_controlli()))
        .0
        .center()
}

#[test]
fn ctrl_comma_and_the_menu_command_open_and_close_it() {
    let (mut f, ctx, p, _) = finestra();
    assert!(!f.impostazioni_aperta() && f.impostazioni_rect().is_none());
    apri(&mut f, &ctx);
    assert!(f.impostazioni_rect().is_some());
    giro(&mut f, &ctx, vec![ctrl_virgola()]);
    giro(&mut f, &ctx, vec![]);
    assert!(!f.impostazioni_aperta());
    f.azione(Cmd::Settings);
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    assert!(f.impostazioni_aperta());
    let _ = std::fs::remove_file(p);
}

#[test]
fn every_setting_is_there() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let nomi: Vec<String> = f
        .impostazioni_controlli()
        .iter()
        .map(|(_, t)| t.clone())
        .collect();
    for atteso in [
        "General",
        "Keys",
        "tema",
        "-",
        "+",
        "Default",
        "Line index",
        "Show line numbers",
        "Show special characters",
        "Colour log lines",
        "Page-jump bar",
        "Change…",
    ] {
        assert!(
            nomi.iter().any(|n| n == atteso),
            "manca {atteso:?} in {nomi:?}"
        );
    }
    // nessun controllo ne copre un altro (una riga a capo nella griglia li sovrapponeva)
    let v = f.impostazioni_controlli();
    for (i, (a, na)) in v.iter().enumerate() {
        for (b, nb) in &v[i + 1..] {
            assert!(!a.intersects(*b), "{na:?} {a:?} copre {nb:?} {b:?}");
        }
    }
    // il tema e' una tendina: chiusa mostra solo il tema scelto, aperta ha Automatico e i quattro temi
    let combo = controllo(&f, "tema");
    clic(&mut f, &ctx, combo);
    let nomi: Vec<String> = f
        .impostazioni_controlli()
        .iter()
        .map(|(_, t)| t.clone())
        .collect();
    for atteso in [
        "Automatic (system)",
        "Blue (Total Commander)",
        "Debian (blue)",
        "Dark",
        "Light",
    ] {
        assert!(
            nomi.iter().any(|n| n == atteso),
            "manca {atteso:?} in {nomi:?}"
        );
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_switches_do_what_the_keys_do_and_are_saved() {
    let (mut f, ctx, p, settings) = finestra();
    apri(&mut f, &ctx);
    assert!(f.workspace().line_numbers() && !f.workspace().special_chars());
    let pos = controllo(&f, "Show line numbers");
    clic(&mut f, &ctx, pos);
    let pos = controllo(&f, "Show special characters");
    clic(&mut f, &ctx, pos);
    let pos = controllo(&f, "Colour log lines");
    clic(&mut f, &ctx, pos);
    let pos = controllo(&f, "Page-jump bar");
    clic(&mut f, &ctx, pos);
    // finche' non si preme OK non e' cambiato niente (e niente e' stato salvato)
    assert!(f.workspace().line_numbers() && !f.workspace().special_chars());
    assert!(!f.workspace().log_colors() && !f.workspace().nav_bar());
    assert!(
        !settings.exists()
            || !std::fs::read_to_string(&settings)
                .unwrap()
                .contains("special=1")
    );
    let pos = controllo(&f, "OK");
    clic(&mut f, &ctx, pos);
    assert!(!f.impostazioni_aperta(), "OK chiude la finestra");
    assert!(!f.workspace().line_numbers());
    assert!(f.workspace().special_chars());
    assert!(f.workspace().log_colors());
    assert!(f.workspace().nav_bar());
    let scritto = std::fs::read_to_string(&settings).unwrap();
    for voce in ["line_numbers=0", "special=1", "nav_bar=1"] {
        assert!(scritto.contains(voce), "{voce} in {scritto}");
    }
    // i colori dei log sono accesi di fabbrica: acceso non si scrive (si scrive solo `log_colors=0`)
    assert!(!scritto.contains("log_colors"), "{scritto}");
    // e si spengono di nuovo
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Show special characters");
    clic(&mut f, &ctx, pos);
    let pos = controllo(&f, "OK");
    clic(&mut f, &ctx, pos);
    assert!(!f.workspace().special_chars());
    let _ = std::fs::remove_file(p);
}

#[test]
fn picking_a_theme_applies_and_saves_it() {
    let (mut f, ctx, p, settings) = finestra();
    apri(&mut f, &ctx);
    let prima = f.app().theme_id();
    scegli_tema(&mut f, &ctx, "Light");
    assert_eq!(f.app().theme_id(), prima, "la scelta aspetta OK");
    let pos = controllo(&f, "OK");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().theme_id(), ThemeId::Light);
    assert!(std::fs::read_to_string(&settings)
        .unwrap()
        .contains("theme=light"));
    apri(&mut f, &ctx);
    scegli_tema(&mut f, &ctx, "Dark");
    let pos = controllo(&f, "OK");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.app().theme_id(), ThemeId::Dark);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_text_size_buttons_change_it_and_it_is_remembered() {
    let (mut f, ctx, p, settings) = finestra();
    apri(&mut f, &ctx);
    let prima = f.corpo();
    let pos = controllo(&f, "+");
    clic(&mut f, &ctx, pos);
    clic(&mut f, &ctx, pos);
    assert_eq!(f.corpo(), prima, "il corpo cambia con OK");
    let ok = controllo(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.corpo(), prima + 2.0);
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(
        scritto.contains(&format!("font_size={}", prima as u16 + 2)),
        "{scritto}"
    );
    apri(&mut f, &ctx);
    let pos = controllo(&f, "-");
    clic(&mut f, &ctx, pos);
    let ok = controllo(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.corpo(), prima + 1.0);
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Default");
    clic(&mut f, &ctx, pos);
    let ok = controllo(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.corpo(), 15.0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_remembered_size_is_used_at_the_start() {
    let (p, _) = percorsi();
    std::fs::write(&p, b"uno\n").unwrap();
    let cfg = move || AppConfig {
        chrome: false,
        font_size: Some(21),
        ..AppConfig::default()
    };
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let f = Finestra::da_app(app, Box::new(cfg));
    assert_eq!(f.corpo(), 21.0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_jump_change_button_opens_the_jump_window() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Change…");
    clic(&mut f, &ctx, pos);
    assert_eq!(f.dialogo_titolo(), Some("JUMPS"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_keys_tab_lists_every_command_with_the_window_shortcuts_and_the_other_keys() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Keys");
    clic(&mut f, &ctx, pos);
    let righe = f.impostazioni_tasti().to_vec();
    // tutti i comandi del registro, coi tasti della finestra (F3, F4, Alt+E) e Ctrl+, per le impostazioni
    for c in COMMANDS {
        let nome = f.app().text(c.label).trim_end_matches('…').to_string();
        assert!(righe.iter().any(|(n, _)| *n == nome), "manca {nome:?}");
    }
    let tasto = |nome: &str| {
        righe
            .iter()
            .find(|(n, _)| n == nome)
            .map(|(_, t)| t.as_str())
    };
    assert_eq!(tasto("Find"), Some("F3"));
    assert_eq!(tasto("Replace"), Some("F4"));
    assert_eq!(tasto("Edit mode"), Some("Alt+E"));
    assert_eq!(tasto("Show line numbers"), Some("Alt+N"));
    assert_eq!(tasto("Settings"), Some("Ctrl+,"));
    // e gli altri tasti, dalla tabella dell'aiuto (descrizione, tasti)
    assert!(
        righe
            .iter()
            .any(|(n, t)| n.contains("select") && t.contains("Ctrl+A")),
        "{righe:?}"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_settings_window_can_be_dragged() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let r = f.impostazioni_rect().unwrap();
    let presa = Pos2::new(r.center().x, r.min.y + 8.0);
    let arrivo = presa + Vec2::new(-100.0, 60.0);
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
    let dopo = f.impostazioni_rect().unwrap();
    assert!(
        (dopo.min.x - (r.min.x - 100.0)).abs() < 3.0 && (dopo.min.y - (r.min.y + 60.0)).abs() < 3.0,
        "{r:?} -> {dopo:?}"
    );
    let _ = std::fs::remove_file(p);
}

// ---- OK e Annulla: le scelte non cambiano niente finche' non si conferma ----

fn tasto_giu(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// Cambia un po' di cose senza confermare: un interruttore, il tema e il corpo.
fn cambia_tutto(f: &mut Finestra, ctx: &Context) {
    for nome in ["Show special characters", "+"] {
        let pos = controllo(f, nome);
        clic(f, ctx, pos);
    }
    scegli_tema(f, ctx, "Light");
}

fn niente_e_cambiato(f: &mut Finestra, corpo: f32, tema: ThemeId, settings: &PathBuf) {
    assert!(!f.workspace().special_chars());
    assert_eq!(f.app().theme_id(), tema);
    assert_eq!(f.corpo(), corpo);
    assert!(
        !settings.exists()
            || !std::fs::read_to_string(settings)
                .unwrap()
                .contains("special=1"),
        "niente si e' salvato"
    );
}

#[test]
fn cancel_the_x_and_escape_throw_the_choices_away() {
    for come in ["Cancel", "X", "Esc"] {
        let (mut f, ctx, p, settings) = finestra();
        let (corpo, tema) = (f.corpo(), f.app().theme_id());
        apri(&mut f, &ctx);
        cambia_tutto(&mut f, &ctx);
        match come {
            "Cancel" => {
                let pos = controllo(&f, "Cancel");
                clic(&mut f, &ctx, pos);
            }
            "X" => {
                let x = f.impostazioni_chiudi().unwrap().center();
                clic(&mut f, &ctx, x);
            }
            _ => {
                giro(&mut f, &ctx, vec![tasto_giu(Key::Escape)]);
                giro(&mut f, &ctx, vec![]);
            }
        }
        assert!(!f.impostazioni_aperta(), "{come}: la finestra si chiude");
        niente_e_cambiato(&mut f, corpo, tema, &settings);
        // riaperta riparte da come stanno le cose, non dalla bozza scartata
        apri(&mut f, &ctx);
        let pos = controllo(&f, "OK");
        clic(&mut f, &ctx, pos);
        niente_e_cambiato(&mut f, corpo, tema, &settings);
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }
}

#[test]
fn enter_confirms_like_ok() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Show special characters");
    clic(&mut f, &ctx, pos);
    assert!(!f.workspace().special_chars());
    giro(&mut f, &ctx, vec![tasto_giu(Key::Enter)]);
    giro(&mut f, &ctx, vec![]);
    assert!(!f.impostazioni_aperta());
    assert!(f.workspace().special_chars());
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn ok_with_nothing_changed_writes_nothing() {
    let (mut f, ctx, p, settings) = finestra();
    apri(&mut f, &ctx);
    let pos = controllo(&f, "OK");
    clic(&mut f, &ctx, pos);
    assert!(!f.impostazioni_aperta());
    assert!(!settings.exists(), "nessuna modifica: nessun file");
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn enter_in_the_jump_dialog_does_not_confirm_the_settings_below_it() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let cambia = controllo(&f, "Change…");
    clic(&mut f, &ctx, cambia);
    assert!(
        f.app().dialog_kind().is_some(),
        "si e' aperto il dialogo dei salti"
    );
    giro(&mut f, &ctx, vec![tasto_giu(Key::Enter)]);
    giro(&mut f, &ctx, vec![]);
    assert!(f.impostazioni_aperta(), "Invio era del dialogo dei salti");
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

// ---- l'indice righe: spegnerlo dalle impostazioni passa da una conferma ----

#[test]
fn the_line_index_box_is_on_and_turning_it_off_needs_a_confirmation() {
    let (mut f, ctx, p, settings) = finestra();
    apri(&mut f, &ctx);
    assert!(f.workspace().index_enabled());
    // tolta la spunta e premuto OK: compare il dialogo che spiega, e le impostazioni restano aperte
    let pos = controllo(&f, "Line index");
    clic(&mut f, &ctx, pos);
    let ok = controllo(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(
        f.app().dialog_kind(),
        Some(mrp_tui::app::DialogKind::IndexOff)
    );
    assert!(
        f.impostazioni_aperta(),
        "le impostazioni aspettano la risposta"
    );
    assert!(f.workspace().index_enabled(), "non e' ancora spento");
    // Annulla: le impostazioni sono ancora li' con la bozza e l'indice e' acceso
    let annulla = f
        .dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == "Cancel")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, annulla);
    assert_eq!(f.app().dialog_kind(), None);
    assert!(f.impostazioni_aperta());
    assert!(f.workspace().index_enabled());
    assert!(
        !settings.exists()
            || !std::fs::read_to_string(&settings)
                .unwrap()
                .contains("index=0")
    );
    // di nuovo OK, stavolta OK nel dialogo: si spegne, le impostazioni si chiudono e si salva
    let ok = controllo(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(
        f.app().dialog_kind(),
        Some(mrp_tui::app::DialogKind::IndexOff)
    );
    let conferma = f
        .dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == "OK")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, conferma);
    giro(&mut f, &ctx, vec![]);
    assert!(!f.workspace().index_enabled());
    assert!(
        !f.impostazioni_aperta(),
        "finito il lavoro le impostazioni si chiudono"
    );
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(scritto.contains("index=0"), "{scritto}");
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn the_other_choices_are_applied_together_with_the_confirmed_index_off() {
    let (mut f, ctx, p, _) = finestra();
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Show special characters");
    clic(&mut f, &ctx, pos);
    let pos = controllo(&f, "Line index");
    clic(&mut f, &ctx, pos);
    let ok = controllo(&f, "OK");
    clic(&mut f, &ctx, ok);
    let conferma = f
        .dialogo_pulsanti()
        .iter()
        .find(|(_, t)| t == "OK")
        .unwrap()
        .0
        .center();
    clic(&mut f, &ctx, conferma);
    giro(&mut f, &ctx, vec![]);
    assert!(!f.workspace().index_enabled());
    assert!(
        f.workspace().special_chars(),
        "anche l'altra scelta e' stata applicata"
    );
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn turning_the_index_on_again_from_the_settings_is_immediate() {
    let (mut f, ctx, p, _) = finestra();
    f.workspace().set_index(false);
    apri(&mut f, &ctx);
    let pos = controllo(&f, "Line index");
    clic(&mut f, &ctx, pos);
    let ok = controllo(&f, "OK");
    clic(&mut f, &ctx, ok);
    assert_eq!(f.app().dialog_kind(), None, "accendere non chiede niente");
    assert!(f.workspace().index_enabled());
    assert!(!f.impostazioni_aperta());
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn the_view_menu_has_the_line_index_switch() {
    let vista: Vec<Cmd> = mrp_gui::menu::MENU[2].1.iter().flatten().copied().collect();
    assert!(vista.contains(&Cmd::LineIndex));
    let (mut f, _ctx, p, _) = finestra();
    assert_eq!(
        mrp_gui::menu::acceso(f.workspace(), Cmd::LineIndex),
        Some(true)
    );
    f.workspace().set_index(false);
    assert_eq!(
        mrp_gui::menu::acceso(f.workspace(), Cmd::LineIndex),
        Some(false)
    );
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn the_language_list_names_japanese_and_chinese_in_their_own_script_whatever_the_interface_language(
) {
    // senza un font asiatico di sistema (un server Linux) non c'e' niente da provare
    let c = mrp_gui::fonts::candidati;
    let ce_il_font = [
        mrp_gui::fonts::Scrittura::Giapponese,
        mrp_gui::fonts::Scrittura::CineseSemplificato,
    ]
    .iter()
    .all(|s| c(*s).iter().any(|p| std::path::Path::new(p).exists()));
    if !ce_il_font {
        return;
    }
    // l'app con tutte le lingue (come la lancia il programma), in inglese
    let (p, settings) = percorsi();
    std::fs::write(
        &p, b"uno
",
    )
    .unwrap();
    let cfg = move || AppConfig {
        chrome: false,
        settings_path: Some(settings.clone()),
        langs: mrp_core::lang::registry().0,
        ..AppConfig::default()
    };
    let mut app = App::open(&p, cfg()).unwrap();
    assert!(app.wait_idle(Duration::from_secs(20)));
    let mut f = Finestra::da_app(app, Box::new(cfg));
    let ctx = Context::default();
    giro(&mut f, &ctx, vec![]);
    giro(&mut f, &ctx, vec![]);
    let font = egui::FontId::proportional(14.0);
    let nomi: String = f
        .workspace()
        .active()
        .language_list()
        .iter()
        .map(|(_, n)| n.clone())
        .collect();
    assert!(
        nomi.contains("日本語") && nomi.contains("简体中文"),
        "{nomi}"
    );
    // con l'interfaccia in inglese e le impostazioni chiuse quei nomi non si possono scrivere: i font non ci sono
    assert!(!ctx.fonts(|fo| fo.has_glyphs(&font, &nomi)));
    apri(&mut f, &ctx);
    giro(&mut f, &ctx, vec![]);
    assert!(
        ctx.fonts(|fo| fo.has_glyphs(&font, &nomi)),
        "l'elenco delle lingue ha dei quadrati: {nomi}"
    );
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}
