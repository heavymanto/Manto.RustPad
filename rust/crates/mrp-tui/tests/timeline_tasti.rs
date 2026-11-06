//! I tasti della timeline nel terminale: Alt+P cambia il passo dei periodi (automatico, 1 minuto, 10 minuti, 1 ora, 1 giorno e da
//! capo) e lo dice nell'intestazione del pannello e in un messaggio; Alt+E cambia il filtro per livelli (nessuno, ERROR, ERROR e
//! WARN, fino a INFO e da capo) e apre la vista con le sole righe scelte. Scritti da ARCH.
//!
//! Il file di prova: 144 righe, una ogni mezz'ora dal 2026-11-01, il livello gira ERROR, WARN, INFO, DEBUG (la riga `i` ha il
//! livello `i % 4`): 36 righe per livello, quindi il filtro con 1, 2 o 3 livelli ne tiene 36, 72, 108.

use mrp_core::timeline::Step;
use mrp_tui::app::timeline::TimelineStatus;
use mrp_tui::app::{App, AppConfig};
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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

/// Quante righe di testo ha la scheda attiva e se sono tutte di uno dei livelli dati.
fn righe_della_vista(w: &Workspace) -> Vec<String> {
    String::from_utf8_lossy(&w.active().document_text())
        .lines()
        .map(str::to_string)
        .collect()
}

fn intestazione(w: &mut Workspace) -> String {
    // la riga 0 e' l'intestazione del pannello (senza barra delle schede, con una scheda sola)
    w.render_lines(100, 24)[0].clone()
}

#[test]
fn alt_p_goes_through_the_steps_and_back_to_auto() {
    let (d, mut w) = con_timeline();
    assert!(w.active().timeline_auto());
    let giro = [
        (Step::Minute, "1 minute"),
        (Step::TenMinutes, "10 minutes"),
        (Step::Hour, "1 hour"),
        (Step::Day, "1 day"),
    ];
    for (passo, nome) in giro {
        w.handle_key(alt('p'));
        assert!(!w.active().timeline_auto(), "{nome}");
        // le tacche si ricalcolano col passo nuovo: il passo "in uso" li' e' quello delle tacche
        let fine = Instant::now() + Duration::from_secs(20);
        while w.active().timeline_step() != passo && Instant::now() < fine {
            w.tick();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(w.active().timeline_step(), passo, "{nome}");
        assert_eq!(w.active().flash(), Some(nome));
    }
    // dopo il giorno si torna al passo scelto da solo
    w.handle_key(alt('p'));
    assert!(w.active().timeline_auto());
    assert_eq!(w.active().flash(), Some("Auto"));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_step_shows_in_the_header_of_the_panel_with_a_tilde_when_it_is_automatic() {
    let (d, mut w) = con_timeline();
    // tre giorni: il passo automatico e' l'ora (72 periodi) e si riconosce dalla tilde
    let h = intestazione(&mut w);
    assert!(h.contains("~1 hour"), "{h:?}");
    w.handle_key(alt('p')); // 1 minuto
    w.handle_key(alt('p')); // 10 minuti
    pronta(&mut w);
    let h = intestazione(&mut w);
    assert!(h.contains("10 minutes") && !h.contains('~'), "{h:?}");
    w.handle_key(alt('p')); // 1 ora
    w.handle_key(alt('p')); // 1 giorno
    pronta(&mut w);
    let h = intestazione(&mut w);
    assert!(h.contains("1 day") && !h.contains('~'), "{h:?}");
    // l'intestazione sta nelle colonne del pannello (24 e il filetto)
    assert!(h.chars().take(25).collect::<String>().contains("1 day"));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_keys_do_nothing_when_the_timeline_is_off() {
    let (d, mut w) = con_timeline();
    w.active_mut().set_timeline(false);
    let prima = w.active().timeline_step();
    w.handle_key(alt('p'));
    assert_eq!(w.active().timeline_step(), prima);
    assert!(w.active().timeline_auto());
    w.handle_key(alt('e'));
    assert!(w.wait_idle(Duration::from_secs(20)));
    assert_eq!(w.len(), 1, "nessuna vista aperta");
    assert!(w.active().timeline_filter_levels().iter().all(|&l| !l));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn alt_e_goes_through_the_levels_and_opens_the_view_with_those_lines() {
    let (d, mut w) = con_timeline();
    // ERROR
    w.handle_key(alt('e'));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert_eq!(w.len(), 2);
    assert!(w.active().is_filter_view());
    assert_eq!(
        w.active().timeline_filter_levels(),
        [true, false, false, false]
    );
    let v = righe_della_vista(&w);
    assert_eq!(v.len(), 36);
    assert!(v.iter().all(|r| r.contains("[ERROR]")));
    // ERROR e WARN: la vista si rifa' con le due
    w.handle_key(alt('e'));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert!(w.wait_idle(Duration::from_secs(30)));
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
    w.handle_key(alt('e'));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert_eq!(
        w.active().timeline_filter_levels(),
        [true, true, true, false]
    );
    let v = righe_della_vista(&w);
    assert_eq!(v.len(), 108);
    assert!(v.iter().all(|r| !r.contains("[DEBUG]")));
    // e da capo: nessun livello, si torna al file vero
    w.handle_key(alt('e'));
    assert!(w.wait_idle(Duration::from_secs(30)));
    w.tick();
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert_eq!(w.active().timeline_filter_levels(), [false; 4]);
    assert!(!w.active().is_filter_view(), "di nuovo il file");
    assert_eq!(righe_della_vista(&w).len(), 144);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn levels_chosen_elsewhere_start_the_cycle_again_from_error() {
    let (d, mut w) = con_timeline();
    // per esempio i soli DEBUG scelti con la finestra: non sono un passo del giro
    w.active_mut().set_timeline_filter_level(3, true);
    w.handle_key(alt('e'));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert!(w.wait_idle(Duration::from_secs(30)));
    assert_eq!(
        w.active().timeline_filter_levels(),
        [true, false, false, false]
    );
    assert_eq!(righe_della_vista(&w).len(), 36);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_help_lists_the_two_keys() {
    let (d, w) = con_timeline();
    let aiuto = w.active().help_rows();
    assert!(
        aiuto
            .iter()
            .any(|(tasti, _)| tasti.contains("Alt+P") && tasti.contains("Alt+E")),
        "{aiuto:?}"
    );
    let _ = std::fs::remove_dir_all(d);
}
