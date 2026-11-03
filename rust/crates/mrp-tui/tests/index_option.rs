//! L'indice righe come impostazione (di fabbrica acceso): spegnerlo (Alt+X o dalle impostazioni) mostra un dialogo che spiega
//! cosa non ci sara' piu' e chiede OK / Annulla; spento, niente numeri di riga, niente "vai alla riga N" e niente totale. Si puo'
//! riaccendere. Scritti da ARCH.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::dialogs::DialogView;
use mrp_tui::settings::Settings;
use mrp_tui::ui::render_lines;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_idx_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un workspace con un file di dieci righe e le impostazioni salvate in `settings.txt` della cartella.
fn ws(index: bool) -> (Workspace, PathBuf) {
    let d = cartella();
    let p = d.join("a.log");
    let dati: String = (1..=10).map(|i| format!("riga numero {i}\n")).collect();
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        index,
        settings_path: Some(d.join("profilo").join("settings.txt")),
        ..AppConfig::default()
    };
    let a = App::open(&p, cfg.clone()).unwrap();
    let mut w = Workspace::new(a, cfg);
    w.resize(80, 24);
    assert!(w.wait_idle(Duration::from_secs(20)));
    (w, d)
}

fn alt(w: &mut Workspace, c: char) {
    w.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT));
}

fn tasto(w: &mut Workspace, c: KeyCode) {
    w.handle_key(KeyEvent::new(c, KeyModifiers::NONE));
}

fn schermo(w: &mut Workspace) -> String {
    w.render_lines(80, 24).join("\n")
}

#[test]
fn the_index_is_on_by_default_and_the_setting_is_written_only_when_off() {
    assert!(Settings::default().index);
    assert!(!Settings::default().to_text().contains("index"));
    let spento = Settings {
        index: false,
        ..Settings::default()
    };
    assert!(spento.to_text().contains("index=0"));
    assert!(!Settings::from_text("index=0\n").index);
    assert!(!Settings::from_text("index=off\n").index);
    assert!(Settings::from_text("index=1\n").index);
    assert!(Settings::from_text("niente\n").index);
}

#[test]
fn turning_it_off_asks_first_and_explains_what_goes_away() {
    let (mut w, d) = ws(true);
    alt(&mut w, 'x');
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::IndexOff));
    assert!(w.index_enabled(), "non e' ancora spento");
    let Some(DialogView::IndexOff {
        title,
        text,
        features,
        note,
    }) = w.active().dialog_view()
    else {
        panic!("manca la vista");
    };
    assert_eq!(title, "TURN OFF THE LINE INDEX?");
    assert!(text.contains("will not be available"), "{text}");
    assert_eq!(features.len(), 5);
    assert!(features[0].contains("Line numbers") && features[1].contains("Go to a line"));
    assert!(note.contains("turn it on again"), "{note}");
    // Annulla (Esc): resta acceso
    tasto(&mut w, KeyCode::Esc);
    assert_eq!(w.active().dialog_kind(), None);
    assert!(w.index_enabled() && w.active().index_enabled());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn ok_turns_it_off_in_every_tab_and_saves_it() {
    let (mut w, d) = ws(true);
    // una seconda scheda
    let q = d.join("b.log");
    std::fs::write(&q, "uno\ndue\n").unwrap();
    w.open(&q).unwrap();
    assert!(w.wait_idle(Duration::from_secs(20)));
    alt(&mut w, 'x');
    tasto(&mut w, KeyCode::Enter);
    assert_eq!(w.active().dialog_kind(), None);
    assert!(!w.index_enabled());
    assert!(
        w.tabs().iter().all(|a| !a.index_enabled()),
        "in tutte le schede"
    );
    // salvato
    let scritto = std::fs::read_to_string(d.join("profilo").join("settings.txt")).unwrap();
    assert!(scritto.contains("index=0"), "{scritto}");
    // e le schede nuove nascono senza indice
    let r = d.join("c.log");
    std::fs::write(&r, "x\n").unwrap();
    w.open(&r).unwrap();
    assert!(!w.active().index_enabled());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn without_the_index_there_are_no_line_numbers_and_no_total_and_go_to_a_line_says_why() {
    let (mut w, d) = ws(true);
    // con l'indice: numeri di riga nel margine e totale
    let s = schermo(&mut w);
    assert!(s.contains("riga numero 1") && s.contains("1│"), "{s}");
    assert_eq!(w.active_mut().status().line_count, Some(10));
    alt(&mut w, 'x');
    tasto(&mut w, KeyCode::Enter);
    let s = schermo(&mut w);
    assert!(s.contains("riga numero 1") && !s.contains("│riga"), "{s}");
    assert!(!s.contains("1│"), "niente margine dei numeri: {s}");
    assert_eq!(w.active_mut().status().line_count, None);
    assert_eq!(w.active_mut().status().line, None);
    // "vai alla riga 5" non c'e': lo dice, e il cursore non si muove
    w.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    w.active_mut().dialog_set_text("5");
    w.active_mut().dialog_submit();
    assert_eq!(w.active().cursor(), 0);
    let f = w.active().flash().unwrap_or_default().to_string();
    assert!(f.contains("line index"), "{f}");
    // l'offset (@N) e la percentuale (%P) funzionano ancora
    w.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    w.active_mut().dialog_set_text("@16");
    w.active_mut().dialog_submit();
    assert_eq!(w.active().cursor(), 16);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn it_can_be_turned_on_again_without_asking_and_builds_in_the_background() {
    let (mut w, d) = ws(false);
    assert!(!w.index_enabled());
    // aperto senza indice: nessun lavoro in corso, niente totale
    assert_eq!(w.active_mut().status().line_count, None);
    alt(&mut w, 'x');
    // acceso: subito, senza dialogo
    assert_eq!(w.active().dialog_kind(), None);
    assert!(w.index_enabled() && w.active().index_enabled());
    assert!(w.wait_idle(Duration::from_secs(20)));
    assert_eq!(w.active_mut().status().line_count, Some(10));
    let scritto = std::fs::read_to_string(d.join("profilo").join("settings.txt")).unwrap();
    assert!(!scritto.contains("index=0"), "{scritto}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_file_opened_with_the_index_off_never_starts_the_index_job() {
    let (mut w, d) = ws(false);
    // wait_idle ritorna subito: nessuna indicizzazione in corso, nemmeno all'apertura
    assert!(w.active_mut().status().busy.is_none());
    assert_eq!(w.active_mut().status().line_count, None);
    let s = render_lines(w.active_mut(), 80, 24).join("\n");
    assert!(!s.contains("1│"), "{s}");
    let _ = std::fs::remove_dir_all(d);
}
