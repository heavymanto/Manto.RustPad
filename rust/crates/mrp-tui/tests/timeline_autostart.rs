//! La timeline che parte da sola e il lato della barra: le impostazioni che se ne ricordano. Scritti da ARCH.

use mrp_tui::app::timeline::{autostart_size_ok, TimelinePref, AUTOSTART_MAX};
use mrp_tui::app::{App, AppConfig};
use mrp_tui::settings::Settings;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(testo: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlauto_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, testo).unwrap();
    p
}

const DATATO: &str = "2026-11-01 10:00:00 [INFO ] uno\n2026-11-01 10:30:00 [WARN ] due\n2026-11-02 08:00:00 [ERROR] tre\n";

fn apri(testo: &str, chrome: bool, pref: TimelinePref) -> (App, PathBuf) {
    let p = file(testo);
    let cfg = AppConfig {
        chrome,
        timeline: pref,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

#[test]
fn a_file_with_a_recognised_date_starts_the_timeline_once() {
    let (mut a, p) = apri(DATATO, false, TimelinePref::default());
    assert!(!a.timeline_on());
    a.timeline_tick();
    assert!(a.timeline_on());
    assert!(!a.timeline_pref().on, "l'apertura non cambia la preferenza");
    // una volta sola: se l'utente la spegne, resta spenta
    a.set_timeline(false);
    a.timeline_tick();
    assert!(!a.timeline_on());
    let _ = std::fs::remove_file(p);
}

#[test]
fn nothing_recognised_nothing_starts() {
    let (mut a, p) = apri("niente\nda\nvedere\n", false, TimelinePref::default());
    a.timeline_tick();
    assert!(!a.timeline_on());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_switch_and_the_terminal_keep_it_off() {
    let spenta = TimelinePref {
        no_autostart: true,
        ..TimelinePref::default()
    };
    let (mut a, p) = apri(DATATO, false, spenta);
    a.timeline_tick();
    assert!(!a.timeline_on());
    let _ = std::fs::remove_file(p);
    // nel terminale (con la cornice) non parte
    let (mut a, p) = apri(DATATO, true, TimelinePref::default());
    a.timeline_tick();
    assert!(!a.timeline_on());
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_document_with_unsaved_edits_does_not_start_it() {
    let (mut a, p) = apri(DATATO, false, TimelinePref::default());
    // F4 passa in modifica, poi un carattere sporca il documento
    a.handle_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE));
    a.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(a.is_modified());
    a.timeline_tick();
    assert!(!a.timeline_on());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_size_limit_for_starting_by_itself_is_two_gib_included() {
    let due_gib = 2u64 * 1024 * 1024 * 1024;
    assert_eq!(AUTOSTART_MAX, due_gib);
    assert!(autostart_size_ok(0));
    assert!(autostart_size_ok(due_gib - 1));
    assert!(
        autostart_size_ok(due_gib),
        "esattamente 2 GiB e' ancora permesso"
    );
    assert!(!autostart_size_ok(due_gib + 1));
    assert!(!autostart_size_ok(u64::MAX));
}

/// Un file vero appena sopra il limite: pesante (2 GiB sul disco), quindi fuori dal giro normale (`--ignored` per lanciarlo).
#[test]
#[ignore = "crea un file da 2 GiB"]
fn a_file_over_the_limit_does_not_start_it_even_with_dates() {
    let p = file(DATATO);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&p)
        .unwrap()
        .set_len(AUTOSTART_MAX + 1)
        .unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 24);
    assert!(a.wait_idle(Duration::from_secs(120)));
    a.timeline_tick();
    assert!(!a.timeline_on());
    drop(a);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_side_and_the_switch_are_remembered_in_the_settings_file() {
    let mut s = Settings::default();
    assert!(!s.timeline.right && !s.timeline.no_autostart);
    let t = s.to_text();
    assert!(
        !t.contains("timeline_side") && !t.contains("timeline_autostart"),
        "{t}"
    );
    s.timeline.right = true;
    s.timeline.no_autostart = true;
    let t = s.to_text();
    assert!(t.contains("timeline_side=right\n"), "{t}");
    assert!(t.contains("timeline_autostart=0\n"), "{t}");
    let r = Settings::from_text(&t);
    assert!(r.timeline.right && r.timeline.no_autostart);
    // il valore a mano
    let r = Settings::from_text("timeline_side=left\ntimeline_autostart=1\n");
    assert!(!r.timeline.right && !r.timeline.no_autostart);
    let r = Settings::from_text("timeline_side=su\ntimeline_autostart=boh\n");
    assert!(
        !r.timeline.right && !r.timeline.no_autostart,
        "valori non validi: di fabbrica"
    );
}

#[test]
fn the_side_and_the_switch_are_written_to_the_settings_file_when_chosen() {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("mrp_tlauto_imp_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    let imp = dir.join("settings.txt");
    let p = file(DATATO);
    let cfg = AppConfig {
        chrome: false,
        settings_path: Some(imp.clone()),
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    let scritto = || std::fs::read_to_string(&imp).unwrap_or_default();
    a.set_timeline_right(true);
    assert!(scritto().contains("timeline_side=right\n"), "{}", scritto());
    a.set_timeline_autostart(false);
    assert!(
        scritto().contains("timeline_autostart=0\n"),
        "{}",
        scritto()
    );
    // e tornano di fabbrica: le due righe spariscono
    a.set_timeline_right(false);
    a.set_timeline_autostart(true);
    let t = scritto();
    assert!(
        !t.contains("timeline_side") && !t.contains("timeline_autostart"),
        "{t}"
    );
    drop(a);
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_view_that_continues_a_tab_does_not_start_the_timeline_the_user_turned_off() {
    // il file ha date e la partenza da sola e' accesa: la scheda madre l'ha accesa e l'utente l'ha spenta
    let (mut madre, p) = apri(DATATO, false, TimelinePref::default());
    madre.timeline_tick();
    assert!(madre.timeline_on());
    madre.set_timeline(false);
    // la vista che la continua (stesso file, per il collaudo) eredita lo stato spento e non riparte da sola
    let (mut figlia, q) = apri(DATATO, false, TimelinePref::default());
    figlia.inherit_time_view(&madre);
    figlia.timeline_tick();
    assert!(!figlia.timeline_on());
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn choosing_the_side_changes_the_preference_and_keeps_the_rest() {
    let (mut a, p) = apri(DATATO, false, TimelinePref::default());
    a.set_timeline(true);
    a.set_timeline_right(true);
    assert!(a.timeline_right());
    assert!(a.timeline_pref().right && a.timeline_pref().on);
    // una scelta sulla barra (il passo) non perde il lato
    a.set_timeline_show_empty(true);
    assert!(a.timeline_pref().right && a.timeline_pref().empty);
    a.set_timeline_autostart(false);
    assert!(!a.timeline_autostart());
    a.set_timeline_show_empty(false);
    assert!(a.timeline_pref().right && a.timeline_pref().no_autostart);
    let _ = std::fs::remove_file(p);
}
