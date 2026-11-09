//! I menu della timeline nel terminale: Alt+P (o un clic sull'intestazione del pannello) apre il menu del passo dei periodi
//! (automatico, 1 minuto, 10 minuti, 1 ora, 1 giorno); Alt+S apre quello dei livelli (tutte le righe, errori, errori e avvisi,
//! fino a INFO) e la scelta apre la vista con le sole righe di quei livelli. Spegnere e riaccendere la timeline (Alt+B) rida' i
//! periodi. Scritti da ARCH.
//!
//! Il file di prova: 144 righe, una ogni mezz'ora dal 2026-11-01, il livello gira ERROR, WARN, INFO, DEBUG (la riga `i` ha il
//! livello `i % 4`): 36 righe per livello, quindi il filtro con 1, 2 o 3 livelli ne tiene 36, 72, 108.

use mrp_core::timeline::Step;
use mrp_tui::app::timeline::TimelineStatus;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn dati() -> String {
    let mut dati = String::new();
    for i in 0..144u32 {
        let livello = ["ERROR", "WARN", "INFO", "DEBUG"][(i % 4) as usize];
        dati.push_str(&format!(
            "2026-11-{:02} {:02}:{:02}:00 [{livello}] riga {i}\n",
            1 + i / 48,
            (i % 48) / 2,
            (i % 2) * 30
        ));
    }
    dati
}

fn alt(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

fn tasto(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

/// Il Workspace sul file di prova, 100 x 24, con la timeline accesa e pronta (passo automatico).
fn con_timeline() -> (PathBuf, Workspace) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_tltasti_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("a.log");
    std::fs::write(&p, dati()).unwrap();
    let cfg = AppConfig::default();
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut w = Workspace::new(app, cfg);
    w.resize(100, 24);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w.active_mut().set_timeline(true);
    pronta(&mut w);
    (d, w)
}

/// Gira finche' la timeline della scheda attiva ha le tacche (o passano 20 secondi).
fn pronta(w: &mut Workspace) {
    let fine = Instant::now() + Duration::from_secs(20);
    loop {
        w.tick();
        if w.active().timeline_status() == TimelineStatus::Ready || Instant::now() >= fine {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(w.active().timeline_status(), TimelineStatus::Ready);
}

/// Aspetta che le tacche abbiano il passo `passo` (o passano 20 secondi).
fn aspetta_passo(w: &mut Workspace, passo: Step) {
    let fine = Instant::now() + Duration::from_secs(20);
    while w.active().timeline_step() != passo && Instant::now() < fine {
        w.tick();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Apre il menu con `apri` (Alt+P o Alt+S), va alla voce `voce` dalla voce evidenziata di partenza e conferma.
fn scegli(w: &mut Workspace, apri: char, voce: usize) {
    w.handle_key(alt(apri));
    let da = w.active().menu_index().expect("un menu");
    for _ in da..voce {
        w.handle_key(tasto(KeyCode::Down));
    }
    for _ in voce..da {
        w.handle_key(tasto(KeyCode::Up));
    }
    w.handle_key(tasto(KeyCode::Enter));
}

/// Le righe di testo della scheda attiva.
fn righe_della_vista(w: &Workspace) -> Vec<String> {
    String::from_utf8_lossy(&w.active().document_text())
        .lines()
        .map(str::to_string)
        .collect()
}

fn schermo(w: &mut Workspace) -> String {
    w.render_lines(100, 24).join("\n")
}

fn intestazione(w: &mut Workspace) -> String {
    // la riga 0 e' l'intestazione del pannello (senza barra delle schede, con una scheda sola)
    w.render_lines(100, 24)[0].clone()
}

fn finisce_il_filtro(w: &mut Workspace) {
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert!(w.wait_idle(Duration::from_secs(30)));
}

#[test]
fn alt_p_opens_a_menu_of_the_steps_that_starts_from_the_current_one() {
    let (d, mut w) = con_timeline();
    // di fabbrica (e a ogni file che si apre) il passo e' un giorno fisso: il menu parte da lì
    assert!(!w.active().timeline_auto());
    w.handle_key(alt('p'));
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::TimelineStep));
    assert_eq!(w.active().menu_index(), Some(4), "un giorno di fabbrica");
    let s = schermo(&mut w);
    for voce in [
        "TIMELINE: PERIOD",
        "Auto  chosen from the length of the log",
        "1 minute",
        "10 minutes",
        "1 hour",
        "1 day",
    ] {
        assert!(s.contains(voce), "manca {voce:?} in\n{s}");
    }
    // Esc rinuncia: non cambia niente
    w.handle_key(tasto(KeyCode::Esc));
    assert_eq!(w.active().dialog_kind(), None);
    assert!(!w.active().timeline_auto());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_step_chosen_in_the_menu_applies_and_the_menu_starts_from_it_next_time() {
    let (d, mut w) = con_timeline();
    let giro = [
        (1, Step::Minute, "1 minute"),
        (2, Step::TenMinutes, "10 minutes"),
        (3, Step::Hour, "1 hour"),
        (4, Step::Day, "1 day"),
    ];
    for (voce, passo, nome) in giro {
        scegli(&mut w, 'p', voce);
        assert_eq!(w.active().dialog_kind(), None);
        assert!(!w.active().timeline_auto(), "{nome}");
        assert_eq!(w.active().flash(), Some(nome));
        aspetta_passo(&mut w, passo);
        assert_eq!(w.active().timeline_step(), passo, "{nome}");
        // riaperto, il menu parte dalla scelta di adesso
        w.handle_key(alt('p'));
        assert_eq!(w.active().menu_index(), Some(voce), "{nome}");
        w.handle_key(tasto(KeyCode::Esc));
    }
    // la prima voce rimette l'automatico
    scegli(&mut w, 'p', 0);
    assert!(w.active().timeline_auto());
    assert_eq!(w.active().flash(), Some("Auto"));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_step_shows_in_the_header_of_the_panel_with_a_tilde_when_it_is_automatic() {
    let (d, mut w) = con_timeline();
    // di fabbrica il passo e' un giorno fisso: nell'intestazione, senza tilde
    let h = intestazione(&mut w);
    assert!(h.contains("1 day") && !h.contains('~'), "{h:?}");
    // l'automatico: tre giorni, il passo e' l'ora (72 periodi) e si riconosce dalla tilde
    scegli(&mut w, 'p', 0);
    aspetta_passo(&mut w, Step::Hour);
    pronta(&mut w);
    let h = intestazione(&mut w);
    assert!(h.contains("~1 hour"), "{h:?}");
    scegli(&mut w, 'p', 2); // 10 minuti
    aspetta_passo(&mut w, Step::TenMinutes);
    pronta(&mut w);
    let h = intestazione(&mut w);
    assert!(h.contains("10 minutes") && !h.contains('~'), "{h:?}");
    scegli(&mut w, 'p', 4); // 1 giorno
    aspetta_passo(&mut w, Step::Day);
    pronta(&mut w);
    let h = intestazione(&mut w);
    assert!(h.contains("1 day") && !h.contains('~'), "{h:?}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_click_on_the_header_of_the_panel_opens_the_step_menu() {
    let (d, mut w) = con_timeline();
    let _ = schermo(&mut w);
    w.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::TimelineStep));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_keys_do_nothing_when_the_timeline_is_off() {
    let (d, mut w) = con_timeline();
    w.active_mut().set_timeline(false);
    w.handle_key(alt('p'));
    assert_eq!(w.active().dialog_kind(), None);
    w.handle_key(alt('s'));
    assert_eq!(w.active().dialog_kind(), None);
    assert_eq!(w.len(), 1, "nessuna vista aperta");
    assert!(w.active().timeline_filter_levels().iter().all(|&l| !l));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn alt_e_opens_a_menu_of_the_levels_and_the_choice_opens_the_view_with_those_lines() {
    let (d, mut w) = con_timeline();
    w.handle_key(alt('s'));
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::TimelineLevels));
    assert_eq!(w.active().menu_index(), Some(0));
    let s = schermo(&mut w);
    for voce in [
        "TIMELINE: SHOW ONLY",
        "All the lines (no filter)",
        "Errors only",
        "Errors and warnings",
        "Errors, warnings and info",
    ] {
        assert!(s.contains(voce), "manca {voce:?} in\n{s}");
    }
    w.handle_key(tasto(KeyCode::Esc));
    assert_eq!(w.len(), 1, "Esc non apre niente");
    // errori
    scegli(&mut w, 's', 1);
    finisce_il_filtro(&mut w);
    assert_eq!(w.len(), 2);
    assert!(w.active().is_filter_view());
    assert_eq!(
        w.active().timeline_filter_levels(),
        [true, false, false, false]
    );
    let v = righe_della_vista(&w);
    assert_eq!(v.len(), 36);
    assert!(v.iter().all(|r| r.contains("[ERROR]")));
    // nella vista il menu parte da "solo errori"; errori e avvisi: la vista si rifa' con le due
    w.handle_key(alt('s'));
    assert_eq!(w.active().menu_index(), Some(1));
    w.handle_key(tasto(KeyCode::Esc));
    scegli(&mut w, 's', 2);
    finisce_il_filtro(&mut w);
    assert_eq!(
        w.active().timeline_filter_levels(),
        [true, true, false, false]
    );
    let v = righe_della_vista(&w);
    assert_eq!(v.len(), 72);
    assert!(v
        .iter()
        .all(|r| r.contains("[ERROR]") || r.contains("[WARN]")));
    // fino a INFO
    scegli(&mut w, 's', 3);
    finisce_il_filtro(&mut w);
    assert_eq!(
        w.active().timeline_filter_levels(),
        [true, true, true, false]
    );
    let v = righe_della_vista(&w);
    assert_eq!(v.len(), 108);
    assert!(v.iter().all(|r| !r.contains("[DEBUG]")));
    // la prima voce: nessun livello, si torna al file vero
    scegli(&mut w, 's', 0);
    assert!(w.wait_idle(Duration::from_secs(30)));
    w.tick();
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert_eq!(w.active().timeline_filter_levels(), [false; 4]);
    assert!(!w.active().is_filter_view(), "di nuovo il file");
    assert_eq!(righe_della_vista(&w).len(), 144);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn levels_chosen_elsewhere_start_the_menu_from_the_first_entry() {
    let (d, mut w) = con_timeline();
    // per esempio i soli DEBUG scelti con la finestra: non sono una voce del menu
    w.active_mut().set_timeline_filter_level(3, true);
    w.handle_key(alt('s'));
    assert_eq!(w.active().menu_index(), Some(0));
    w.handle_key(tasto(KeyCode::Esc));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn turning_the_timeline_off_and_on_again_gives_the_periods_again() {
    let (d, mut w) = con_timeline();
    let prima = w.active().timeline_rows(20).len();
    assert!(prima > 0);
    w.handle_key(alt('b'));
    assert!(!w.active().timeline_on());
    w.handle_key(alt('b'));
    assert!(w.active().timeline_on());
    pronta(&mut w);
    assert_eq!(
        w.active().timeline_rows(20).len(),
        prima,
        "la barra non e' vuota"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_footer_shows_the_two_menus_only_while_the_timeline_is_on() {
    let (d, mut w) = con_timeline();
    w.resize(200, 24);
    let piede = w.render_lines(200, 24).last().unwrap().clone();
    assert!(piede.contains("Alt+B Timeline"), "{piede:?}");
    assert!(piede.contains("Alt+P Period"), "{piede:?}");
    assert!(piede.contains("Alt+S Levels"), "{piede:?}");
    w.handle_key(alt('b'));
    let piede = w.render_lines(200, 24).last().unwrap().clone();
    assert!(piede.contains("Alt+B Timeline"), "{piede:?}");
    assert!(
        !piede.contains("Alt+P") && !piede.contains("Alt+S"),
        "{piede:?}"
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_help_lists_the_two_keys() {
    let (d, w) = con_timeline();
    let aiuto = w.active().help_rows();
    assert!(
        aiuto
            .iter()
            .any(|(tasti, _)| tasti.contains("Alt+P") && tasti.contains("Alt+S")),
        "{aiuto:?}"
    );
    let _ = std::fs::remove_dir_all(d);
}
