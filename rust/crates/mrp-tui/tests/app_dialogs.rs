//! Test dei dialoghi (aiuto, menu tema e lingua, vai a). Scritti da ARCH prima dell'implementazione.
//! Le liste dei dialoghi si controllano per contenuto (le stringhe vengono dalle lingue), gli stati per valore.

use mrp_core::lang::Lang;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::settings::Settings;
use mrp_tui::theme::ThemeId;
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_dlg_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, nome: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(nome);
        std::fs::write(&p, data).unwrap();
        p
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_text(a: &mut App, s: &str) {
    for c in s.chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
}

fn thirty() -> Vec<u8> {
    (1..=30)
        .map(|i| format!("line {i:02}\n"))
        .collect::<String>()
        .into_bytes()
}

/// Lingue di prova: l'inglese incorporato, l'italiano VERO del repo, e una lingua "xx" marcata AI.
fn langs() -> Vec<Lang> {
    let it = Lang::from_file(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang/it.json"),
    )
    .unwrap();
    let xx = Lang::from_json_str(
        "xx",
        "{\"_name\": \"Test\", \"_ai\": \"machine translated\", \"footer.f1\": \"XX1\"}",
    )
    .unwrap();
    vec![Lang::english(), it, xx]
}

fn app_with(dir: &TestDir, w: u16, h: u16, settings: bool) -> App {
    let p = dir.file("f.txt", &thirty());
    let cfg = AppConfig {
        langs: langs(),
        settings_path: if settings {
            Some(dir.0.join("prof").join("settings.txt"))
        } else {
            None
        },
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(w, h);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a
}

fn screen(a: &mut App, w: u16, h: u16) -> Vec<String> {
    render_lines(a, w, h)
        .into_iter()
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// La parte sinistra di una riga di stato: a destra ci sono nome, dimensione e modo, almeno tre spazi dopo.
fn sinistra(riga: &str) -> String {
    riga.split("   ")
        .next()
        .unwrap_or("")
        .trim_end()
        .to_string()
}

fn shows(a: &mut App, w: u16, h: u16, text: &str) -> bool {
    screen(a, w, h).iter().any(|l| l.contains(text))
}

// ---------- aiuto ----------

#[test]
fn help_opens_lists_the_keys_and_closes() {
    let dir = TestDir::new();
    let mut a = app_with(&dir, 80, 23, false);
    assert_eq!(a.dialog_kind(), None);
    a.handle_key(k(KeyCode::F(1)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Help));
    let en = Lang::english();
    // l'aiuto e' piu' lungo dello schermo: in cima ci sono il titolo, le prime righe e come si chiude; l'ultima (Esc) si vede
    // scorrendo fino in fondo
    for key in ["help.title", "help.l1", "help.l5", "help.close"] {
        let atteso = en.get(key).unwrap();
        assert!(
            shows(&mut a, 80, 24, atteso.trim_end()),
            "manca {key}: {atteso:?}"
        );
    }
    let ultima = en.get("help.l10").unwrap();
    assert!(!shows(&mut a, 80, 24, ultima.trim_end()));
    a.handle_key(k(KeyCode::End));
    assert!(
        shows(&mut a, 80, 24, ultima.trim_end()),
        "in fondo manca l'ultima riga"
    );
    assert!(shows(
        &mut a,
        80,
        24,
        en.get("help.close").unwrap().trim_end()
    ));
    a.handle_key(k(KeyCode::Home));
    assert_eq!(
        a.cursor_xy(),
        None,
        "con un dialogo aperto il cursore del terminale e' nascosto"
    );
    // i tasti non arrivano al documento mentre l'aiuto e' aperto
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.cursor(), 0);
    assert_eq!(a.dialog_kind(), Some(DialogKind::Help));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.dialog_kind(), None);
    assert!(a.cursor_xy().is_some());
}

// ---------- tema ----------

#[test]
fn theme_menu_lists_the_four_themes_and_applies_the_choice() {
    let dir = TestDir::new();
    let mut a = app_with(&dir, 90, 23, true);
    assert_eq!(a.theme_id(), ThemeId::Console);
    a.handle_key(k(KeyCode::F(6)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Theme));
    assert_eq!(
        a.menu_index(),
        Some(2),
        "parte dal tema corrente (Console, terzo)"
    );
    let en = Lang::english();
    for t in ThemeId::ALL {
        assert!(shows(&mut a, 90, 24, en.get(t.key()).unwrap()), "{t:?}");
        assert!(
            shows(&mut a, 90, 24, en.get(t.blurb_key()).unwrap()),
            "{t:?} descrizione"
        );
    }
    a.handle_key(k(KeyCode::Up));
    assert_eq!(a.menu_index(), Some(1));
    a.handle_key(k(KeyCode::Up));
    a.handle_key(k(KeyCode::Up));
    assert_eq!(a.menu_index(), Some(0), "in cima si ferma");
    a.handle_key(k(KeyCode::Down));
    assert_eq!(a.menu_index(), Some(1));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.theme_id(), ThemeId::Debian);
    assert_eq!(
        sinistra(&screen(&mut a, 90, 23)[21]),
        " theme: Debian (blue)"
    );
    let s = Settings::load(&dir.0.join("prof").join("settings.txt"))
        .expect("le impostazioni si salvano al cambio");
    assert_eq!(s.theme, ThemeId::Debian);
    assert_eq!(s.lang, None);
}

#[test]
fn theme_menu_escape_changes_nothing() {
    let dir = TestDir::new();
    let mut a = app_with(&dir, 90, 23, true);
    a.handle_key(k(KeyCode::F(6)));
    a.handle_key(k(KeyCode::Up));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.theme_id(), ThemeId::Console);
    assert!(!dir.0.join("prof").exists(), "annullato: niente salvato");
}

#[test]
fn the_configured_theme_is_the_starting_one() {
    let dir = TestDir::new();
    let p = dir.file("f.txt", &thirty());
    let cfg = AppConfig {
        theme: ThemeId::Night,
        ..AppConfig::default()
    };
    let a = App::open(&p, cfg).unwrap();
    assert_eq!(a.theme_id(), ThemeId::Night);
}

// ---------- lingua ----------

#[test]
fn language_menu_marks_ai_translations_and_switches_the_ui() {
    let dir = TestDir::new();
    let mut a = app_with(&dir, 90, 23, true);
    assert_eq!(a.lang_code(), "en");
    a.handle_key(k(KeyCode::F(7)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Lang));
    assert_eq!(a.menu_index(), Some(0), "parte dalla lingua corrente");
    assert!(shows(&mut a, 90, 24, "English"));
    assert!(shows(&mut a, 90, 24, "Italiano"));
    assert!(
        shows(&mut a, 90, 24, "Test [AI]"),
        "le traduzioni AI si dichiarano nel menu"
    );
    assert!(
        shows(&mut a, 90, 24, "AI-translated: may be imprecise or wrong"),
        "e il menu chiude con l'avviso"
    );
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.lang_code(), "it");
    let s = screen(&mut a, 90, 23);
    assert_eq!(sinistra(&s[21]), " lingua: Italiano");
    assert!(
        s[22].contains("F1 Aiuto") && s[22].contains("F10 Esci"),
        "{:?}",
        s[22]
    );
    let salvate = Settings::load(&dir.0.join("prof").join("settings.txt")).unwrap();
    assert_eq!(salvate.lang.as_deref(), Some("it"));
}

#[test]
fn a_partial_language_falls_back_to_english_per_key() {
    let dir = TestDir::new();
    let mut a = app_with(&dir, 90, 23, false);
    a.handle_key(k(KeyCode::F(7)));
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.lang_code(), "xx");
    let s = screen(&mut a, 90, 23);
    assert!(s[22].contains("F1 XX1"), "la chiave tradotta: {:?}", s[22]);
    assert!(
        s[22].contains("F2 Save"),
        "le altre cadono in inglese: {:?}",
        s[22]
    );
}

#[test]
fn the_configured_language_is_the_starting_one_and_unknown_codes_fall_back() {
    let dir = TestDir::new();
    let p = dir.file("f.txt", &thirty());
    let a = App::open(
        &p,
        AppConfig {
            langs: langs(),
            lang_code: Some("it".to_string()),
            ..AppConfig::default()
        },
    )
    .unwrap();
    assert_eq!(a.lang_code(), "it");
    let a = App::open(
        &p,
        AppConfig {
            langs: langs(),
            lang_code: Some("zz".to_string()),
            ..AppConfig::default()
        },
    )
    .unwrap();
    assert_eq!(a.lang_code(), "en");
}

// ---------- vai a ----------

#[test]
fn goto_opens_a_prompt_with_title_and_hint_and_the_cursor_in_it() {
    let dir = TestDir::new();
    let mut a = app_with(&dir, 80, 23, false);
    a.handle_key(ctrl('g'));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Goto));
    assert!(shows(&mut a, 80, 24, "GO TO"));
    assert!(shows(
        &mut a,
        80,
        24,
        "line number, @byte offset, %percent or ~time"
    ));
    assert!(shows(&mut a, 80, 24, "Enter OK - Esc cancel"));
    type_text(&mut a, "12");
    assert!(
        screen(&mut a, 80, 23).iter().any(|l| l.contains("> 12")),
        "il testo scritto si vede dopo '> '"
    );
    assert!(
        a.cursor_xy().is_some(),
        "nel prompt il cursore e' dopo il testo"
    );
    a.handle_key(k(KeyCode::Backspace));
    assert!(screen(&mut a, 80, 23)
        .iter()
        .any(|l| l.contains("> 1") && !l.contains("> 12")));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.cursor(), 0);
}

#[test]
fn goto_a_line_centers_it_when_it_was_not_visible() {
    let dir = TestDir::new();
    let mut a = app_with(&dir, 30, 7, false); // 5 righe di testo
    a.handle_key(k(KeyCode::F(5)));
    type_text(&mut a, "12");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.cursor(), 88, "riga 12 = 11 * 8");
    assert_eq!(a.top(), 72, "centrata: due righe prima (riga 10 in cima)");
    // riga gia' visibile: nessuno scorrimento
    a.handle_key(k(KeyCode::F(5)));
    type_text(&mut a, "13");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!((a.top(), a.cursor()), (72, 96));
    // vicino alla fine: l'ultima pagina, non righe vuote
    a.handle_key(k(KeyCode::F(5)));
    type_text(&mut a, "30");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.cursor(), 232);
    assert_eq!(a.top(), 200, "ultima pagina: le righe 26..30");
    // e la prima
    a.handle_key(k(KeyCode::F(5)));
    type_text(&mut a, "1");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!((a.top(), a.cursor()), (0, 0));
}

#[test]
fn goto_byte_offset_and_percent() {
    let dir = TestDir::new();
    let mut a = app_with(&dir, 30, 7, false);
    a.handle_key(ctrl('g'));
    type_text(&mut a, "@100");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.cursor(), 100);
    assert_eq!(
        a.top(),
        80,
        "la riga 13 (offset 96) centrata: due righe prima, la 11 (offset 80)"
    );
    a.handle_key(ctrl('g'));
    type_text(&mut a, "%50");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.cursor(), 120, "240 * 50 / 100");
    a.handle_key(ctrl('g'));
    type_text(&mut a, "%100");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(
        a.cursor(),
        239,
        "100%: l'ultima posizione consentita (in lettura len - 1)"
    );
    a.handle_key(ctrl('g'));
    type_text(&mut a, "@999999");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.cursor(), 239, "oltre la fine: l'ultima posizione");
}

#[test]
fn goto_refuses_bad_input_with_a_message_and_does_not_move() {
    let dir = TestDir::new();
    let mut a = app_with(&dir, 80, 23, false);
    for (input, atteso) in [
        ("abc", " not a valid position: abc"),
        ("0", " not a valid position: 0"),
        ("-3", " not a valid position: -3"),
        ("%101", " not a valid position: %101"),
        ("@x", " not a valid position: @x"),
        ("99", " line 99 is beyond the end (30 lines)"),
    ] {
        a.handle_key(ctrl('g'));
        type_text(&mut a, input);
        a.handle_key(k(KeyCode::Enter));
        assert_eq!(a.dialog_kind(), None, "{input}");
        assert_eq!(a.cursor(), 0, "{input}");
        assert_eq!(sinistra(&screen(&mut a, 80, 23)[21]), atteso, "{input}");
        a.handle_key(k(KeyCode::Right));
        a.handle_key(k(KeyCode::Left));
    }
    // vuoto: chiude senza messaggio
    a.handle_key(ctrl('g'));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
}

#[test]
fn goto_a_line_needs_the_index() {
    let dir = TestDir::new();
    let p = dir.file("f.txt", &thirty());
    let mut a = App::open(
        &p,
        AppConfig {
            index: false,
            ..AppConfig::default()
        },
    )
    .unwrap();
    a.resize(80, 23);
    a.handle_key(ctrl('g'));
    type_text(&mut a, "12");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.cursor(), 0);
    assert_eq!(
        sinistra(&screen(&mut a, 80, 23)[21]),
        " Go to a line needs the line index: turn it on in the Settings (Alt+X)"
    );
    // ma offset e percentuale funzionano senza indice
    a.handle_key(ctrl('g'));
    type_text(&mut a, "@16");
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.cursor(), 16);
}

#[test]
fn the_help_ends_with_who_the_program_is() {
    let dir = TestDir::new();
    let p = dir.file("h.txt", b"uno`ndue`n");
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(80, 23);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.handle_key(k(KeyCode::F(1)));
    // in fondo alla tabella, dopo una riga vuota: chi e' questo programma
    a.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    let tutto = render_lines(&mut a, 80, 23).join("`n");
    assert!(
        tutto.contains(env!("CARGO_PKG_VERSION")),
        "la versione: {tutto}"
    );
    assert!(tutto.contains("AGPL-3.0"), "la licenza: {tutto}");
    assert!(
        tutto.contains("github.com/heavymanto/Manto.RustPad"),
        "il codice: {tutto}"
    );
}
