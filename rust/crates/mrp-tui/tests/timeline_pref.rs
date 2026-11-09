//! La timeline si riapre com'era quando si e' chiusa (accesa, disposizione, periodi vuoti) e i periodi senza righe non fanno
//! una distesa di righe uguali. Il passo no: ogni file riparte da un giorno, una scelta fine vale solo la sua scheda.
//! Scritti da ARCH: l'oracolo e' il testo del file di impostazioni e i giorni scritti nel log di prova.

use mrp_core::timeline::Step;
use mrp_tui::app::timeline::{TimelineMode, TimelinePref, TimelineStatus};
use mrp_tui::app::{App, AppConfig};
use mrp_tui::settings::Settings;
use mrp_tui::workspace::Workspace;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unico(nome: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("mrp_tlpref_{}_{}_{}", std::process::id(), n, nome))
}

/// Un log con tre giorni pieni (1 e 2 novembre, 10 dicembre): in mezzo ci sono 37 giorni vuoti. Quattro righe al giorno.
fn log_con_buchi() -> PathBuf {
    let mut dati = String::new();
    for (g, mese) in [(1, 11), (2, 11), (10, 12)] {
        for i in 0..4 {
            dati.push_str(&format!("2026-{mese:02}-{g:02} 0{i}:00:00 [INFO] riga\n"));
        }
    }
    let p = unico("buchi.log");
    std::fs::write(&p, dati).unwrap();
    p
}

fn cfg_gui(settings: Option<PathBuf>, timeline: TimelinePref) -> AppConfig {
    AppConfig {
        chrome: false,
        settings_path: settings,
        timeline,
        ..AppConfig::default()
    }
}

fn pronta(a: &mut App) {
    let fine = Instant::now() + Duration::from_secs(20);
    loop {
        a.timeline_tick();
        if a.timeline_status() == TimelineStatus::Ready || Instant::now() >= fine {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_settings_file_keeps_the_timeline_and_ignores_what_it_does_not_understand() {
    let s = Settings {
        timeline: TimelinePref {
            on: true,
            bytes: true,
            empty: true,
            right: false,
            no_autostart: false,
        },
        ..Settings::default()
    };
    let testo = s.to_text();
    for riga in ["timeline=1", "timeline_mode=bytes", "timeline_empty=1"] {
        assert!(
            testo.lines().any(|r| r == riga),
            "manca {riga:?} in {testo:?}"
        );
    }
    assert!(
        !testo.contains("timeline_step"),
        "il passo non si scrive: {testo:?}"
    );
    assert_eq!(Settings::from_text(&testo).timeline, s.timeline);
    // di fabbrica non scrive niente della timeline
    assert!(!Settings::default().to_text().contains("timeline"));
    // valori che non si capiscono: restano i valori di fabbrica
    let strano = Settings::from_text(
        "timeline=forse\ntimeline_step=anno\ntimeline_mode=su\ntimeline_empty=2\n",
    );
    assert_eq!(strano.timeline, TimelinePref::default());
    // una riga di una versione vecchia (quando il passo si scriveva) non conta piu' niente: e' una
    // chiave come le altre sconosciute, e un 10min salvato non rompe i file lunghi di oggi
    let vecchia = Settings::from_text("timeline=1\ntimeline_step=10min\n");
    assert_eq!(
        vecchia.timeline,
        TimelinePref {
            on: true,
            ..TimelinePref::default()
        }
    );
}

#[test]
fn what_the_user_chooses_is_written_but_the_step_resets_to_a_day() {
    let log = log_con_buchi();
    let imp = unico("settings.txt");
    let mut a = App::open(&log, cfg_gui(Some(imp.clone()), TimelinePref::default())).unwrap();
    assert!(!a.timeline_on(), "di fabbrica spenta");
    a.set_timeline(true);
    a.set_timeline_step(Step::Hour);
    a.set_timeline_mode(TimelineMode::Bytes);
    a.set_timeline_show_empty(true);
    let scritte = Settings::load(&imp).expect("le impostazioni si sono scritte");
    assert_eq!(
        scritte.timeline,
        TimelinePref {
            on: true,
            bytes: true,
            empty: true,
            right: false,
            no_autostart: false,
        }
    );
    // chiusa e riaperta: com'era, tranne il passo, che riparte da un giorno
    let b = App::open(&log, cfg_gui(Some(imp.clone()), scritte.timeline)).unwrap();
    assert!(b.timeline_on());
    assert_eq!(b.timeline_step(), Step::Day);
    assert!(!b.timeline_auto());
    assert_eq!(b.timeline_mode(), TimelineMode::Bytes);
    assert!(b.timeline_show_empty());
    // nella prima scheda la scelta fine resta sua: e' del file che si sta guardando
    assert_eq!(a.timeline_step(), Step::Hour);
    // spenta: anche questo si ricorda, e il passo di una scheda non finisce nel file
    a.set_timeline(false);
    a.set_timeline_auto();
    let scritte = Settings::load(&imp).unwrap();
    assert!(!scritte.timeline.on);
    let c = App::open(&log, cfg_gui(Some(imp.clone()), scritte.timeline)).unwrap();
    assert!(!c.timeline_on());
    assert!(!c.timeline_auto());
    assert_eq!(c.timeline_step(), Step::Day);
    let _ = std::fs::remove_file(log);
    let _ = std::fs::remove_file(imp);
}

#[test]
fn the_terminal_opens_with_the_choice_of_the_window_and_does_not_lose_it() {
    let log = log_con_buchi();
    let imp = unico("settings.txt");
    let pref = TimelinePref {
        on: true,
        ..TimelinePref::default()
    };
    // chrome = true e' il terminale: ha il suo pannello (`timeline_pannello.rs`), quindi la scelta della finestra vale anche li' e
    // resta nel file anche se il terminale scrive
    let cfg = AppConfig {
        chrome: true,
        settings_path: Some(imp.clone()),
        timeline: pref,
        ..AppConfig::default()
    };
    let mut a = App::open(&log, cfg).unwrap();
    assert!(a.timeline_on());
    a.set_nav_bar(true, true);
    assert!(Settings::load(&imp).unwrap().timeline.on);
    let _ = std::fs::remove_file(log);
    let _ = std::fs::remove_file(imp);
}

#[test]
fn the_switch_reaches_the_tabs_opened_later_the_step_stays_of_its_tab() {
    let log = log_con_buchi();
    let altro = log_con_buchi();
    let imp = unico("settings.txt");
    let cfg = cfg_gui(Some(imp.clone()), TimelinePref::default());
    let mut ws = Workspace::new(App::open(&log, cfg.clone()).unwrap(), cfg);
    ws.open(&altro).unwrap();
    ws.select(0);
    ws.active_mut().set_timeline(true);
    ws.active_mut().set_timeline_step(Step::TenMinutes);
    ws.tick();
    // la scheda che non si e' toccata non si accende, ma una nuova nasce come l'utente ha lasciato
    assert!(!ws.tabs()[1].timeline_on());
    assert_eq!(ws.tabs()[1].timeline_pref(), ws.tabs()[0].timeline_pref());
    let terzo = log_con_buchi();
    ws.open(&terzo).unwrap();
    assert!(ws.active().timeline_on());
    // il passo no: una scelta fine vale solo la scheda dove e' stata fatta, un file nuovo riparte
    // da un giorno (un 10min rimasto salvato rompeva i file di mesi con "troppi periodi")
    assert_eq!(ws.active().timeline_step(), Step::Day);
    // un'altra scheda che scrive le impostazioni (cambia la barra dei salti) non cancella la timeline
    ws.select(1);
    ws.active_mut().set_nav_bar(true, true);
    let s = Settings::load(&imp).unwrap();
    assert!(s.timeline.on);
    assert_eq!(
        s.timeline,
        TimelinePref {
            on: true,
            ..TimelinePref::default()
        }
    );
    for p in [log, altro, terzo, imp] {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn days_without_lines_are_hidden_unless_asked_for() {
    let log = log_con_buchi();
    let mut a = App::open(&log, cfg_gui(None, TimelinePref::default())).unwrap();
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    pronta(&mut a);
    // oracolo: nel log ci sono tre giorni con delle righe (1 e 2 novembre, 10 dicembre)
    let righe = a.timeline_rows(1000);
    assert_eq!(righe.len(), 3, "{righe:?}");
    assert!(righe.iter().all(|r| r.bytes > 0));
    assert_eq!(
        righe.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
        ["2026-11-01", "2026-11-02", "2026-12-10"]
    );
    // le righe riempiono tutta l'altezza della barra, senza buchi
    assert_eq!(righe[0].y0, 0.0);
    assert!((righe[2].y1 - 1.0).abs() < 1e-6);
    assert!((righe[0].y1 - righe[1].y0).abs() < 1e-6);
    // chiedendoli, i giorni vuoti ci sono tutti: dal 1 novembre al 10 dicembre sono 40 giorni
    a.set_timeline_show_empty(true);
    let tutti = a.timeline_rows(1000);
    assert_eq!(tutti.len(), 40);
    assert_eq!(tutti.iter().filter(|r| r.bytes > 0).count(), 3);
    let _ = std::fs::remove_file(log);
}

#[test]
fn a_click_on_the_compact_bar_lands_on_the_line_of_that_day() {
    let log = log_con_buchi();
    let mut a = App::open(&log, cfg_gui(None, TimelinePref::default())).unwrap();
    a.resize(100, 24);
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    pronta(&mut a);
    // l'ultimo terzo della barra e' il 10 dicembre: porta alla sua prima riga (le righe 9..12 del log)
    let inizio = a.timeline_rows(1000)[2].offset;
    assert!(a.timeline_offset_at(0.9, 1000).unwrap() >= inizio);
    assert_eq!(a.timeline_time_at(0.9, 1000).unwrap()[..10], *"2026-12-10");
    assert_eq!(a.timeline_time_at(0.01, 1000).unwrap()[..10], *"2026-11-01");
    let _ = std::fs::remove_file(log);
}
