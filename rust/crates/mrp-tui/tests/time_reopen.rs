//! La finestra Filtri riaperta ricorda il tempo scelto: la regola dell'utente resta quella selezionata e le date del filtro vengono dal
//! file letto con quella regola. Scritti da ARCH sul log di `time_example.rs`: l'orario all'inizio riga e' del 2026-11-03, la data
//! dentro la riga (dopo `@ `) va dal 2019-09-10 08:00 al 2019-09-20 00:00.

use mrp_tui::app::{App, AppConfig, DialogKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn app() -> (App, PathBuf) {
    let mut dati = String::new();
    for i in 0..30u32 {
        let ore = 8 + 8 * i;
        let (giorno, ora) = (10 + ore / 24, ore % 24);
        dati.push_str(&format!(
            "2026-11-03 11:19:{i:02}.350 +01:00 [INF] XC4 funding @ 2019-09-{giorno:02} {ora:02}:00 rate=1\n"
        ));
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_treopen_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(120, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

/// Sceglie la regola `'@ 'yyyy-MM-dd HH:mm` dalla finestra e preme OK (senza filtro per data).
fn scegli_la_regola(a: &mut App) {
    a.open_time_dialog();
    a.dialog_time_set_example("funding @ 2019-09-10 08:00");
    a.dialog_time_find_rule();
    a.dialog_time_use_example(0);
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None, "{}", a.time_error_text());
}

#[test]
fn reopening_the_window_keeps_my_rule_selected() {
    let (mut a, p) = app();
    scegli_la_regola(&mut a);
    a.open_time_dialog();
    assert_eq!(a.dialog_kind(), Some(DialogKind::Time));
    let scelta = a.time_source_selected();
    let fonti = a.time_sources();
    let nome = scelta.map(|i| fonti[i].pattern.clone());
    assert_eq!(
        nome.as_deref(),
        Some("'@ 'yyyy-MM-dd HH:mm"),
        "fonti: {:?}",
        fonti.iter().map(|f| &f.pattern).collect::<Vec<_>>()
    );
    assert_eq!(a.time_field_text(0), "'@ 'yyyy-MM-dd HH:mm");
    let _ = std::fs::remove_file(p);
}

/// Come sopra ma la regola e' gia' nelle impostazioni (scritta in un'altra occasione): si sceglie dall'elenco, col pulsante.
fn app_con_regola_salvata() -> (App, PathBuf) {
    let (_, p) = app();
    let cfg = AppConfig {
        chrome: false,
        time_rules: vec![("'@ 'yyyy-MM-dd HH:mm".to_string(), 200, 0)],
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(120, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

#[test]
fn a_saved_rule_picked_from_the_list_stays_selected_when_reopening() {
    let (mut a, p) = app_con_regola_salvata();
    a.open_time_dialog();
    let i = a
        .time_sources()
        .iter()
        .position(|s| s.pattern == "'@ 'yyyy-MM-dd HH:mm")
        .expect("la regola e' nell'elenco");
    assert_ne!(a.time_source_selected(), Some(i), "all'inizio c'e' l'altro");
    a.dialog_time_pick_source(i);
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None, "{}", a.time_error_text());
    a.open_time_dialog();
    let scelta = a.time_source_selected().expect("una fonte scelta");
    assert_eq!(a.time_sources()[scelta].pattern, "'@ 'yyyy-MM-dd HH:mm");
    let cal = a.time_calendar(2).expect("il calendario");
    assert_eq!(cal.first, Some((2019, 9, 10)), "{cal:?}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_date_filter_after_my_rule_offers_the_dates_of_the_rule() {
    let (mut a, p) = app();
    scegli_la_regola(&mut a);
    a.open_time_dialog();
    // il calendario del campo "Dal" si apre sul mese del primo giorno letto con la regola: settembre 2019
    let cal = a.time_calendar(2).expect("il calendario");
    assert_eq!(cal.first, Some((2019, 9, 10)), "{cal:?}");
    assert_eq!(cal.last, Some((2019, 9, 20)));
    assert_eq!(cal.month, (2019, 9));
    let _ = std::fs::remove_file(p);
}

#[test]
fn it_is_still_so_after_a_date_filter_was_applied() {
    let (mut a, p) = app();
    a.open_time_dialog();
    a.dialog_time_set_example("funding @ 2019-09-10 08:00");
    a.dialog_time_find_rule();
    a.dialog_time_use_example(0);
    a.dialog_set_field(2, "2019-09-11");
    a.dialog_set_field(3, "2019-09-12");
    a.dialog_submit();
    assert!(a.time_window().is_some(), "{}", a.time_error_text());
    a.open_time_dialog();
    let scelta = a.time_source_selected().expect("una fonte scelta");
    assert_eq!(a.time_sources()[scelta].pattern, "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(a.time_range_text(0), "2019-09-11");
    let cal = a.time_calendar(2).expect("il calendario");
    assert_eq!(cal.first.map(|d| d.0), Some(2019));
    let _ = std::fs::remove_file(p);
}

/// Le regole del file delle impostazioni dell'utente (`yyyy-MM-dd HH:mm -` entro 200 come formato standard, e `yyyy-MM-dd HH:mm`
/// entro 0): ogni tempo dell'elenco, scelto con OK, deve restare quello selezionato riaprendo la finestra.
#[test]
fn every_source_of_the_list_stays_selected_after_ok_with_the_users_rules() {
    let mut dati = String::new();
    for i in 0..40u32 {
        dati.push_str(&format!(
            "2026-11-03 11:{:02}:00 [INF] job run - 2019-09-{:02} {:02}:00 - fine\n",
            i % 60,
            10 + i / 24,
            i % 24
        ));
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_treopen3_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = || AppConfig {
        chrome: false,
        time_format: Some(("yyyy-MM-dd HH:mm -".to_string(), 200)),
        time_rules: vec![
            ("yyyy-MM-dd HH:mm".to_string(), 0, 0),
            ("yyyy-MM-dd HH:mm -".to_string(), 200, 0),
        ],
        ..AppConfig::default()
    };
    let mut a0 = App::open(&p, cfg()).unwrap();
    assert!(a0.wait_idle(Duration::from_secs(20)));
    a0.open_time_dialog();
    let elenco: Vec<(String, usize, usize)> = a0
        .time_sources()
        .iter()
        .map(|s| (s.pattern.clone(), s.within, s.skip))
        .collect();
    assert!(elenco.len() >= 2, "{elenco:?}");
    for i in 0..elenco.len() {
        let mut a = App::open(&p, cfg()).unwrap();
        assert!(a.wait_idle(Duration::from_secs(20)));
        a.open_time_dialog();
        a.dialog_time_pick_source(i);
        a.dialog_submit();
        assert_eq!(a.dialog_kind(), None, "{i}: {}", a.time_error_text());
        a.open_time_dialog();
        let scelta = a.time_source_selected();
        let ora: Vec<(String, usize, usize)> = a
            .time_sources()
            .iter()
            .map(|s| (s.pattern.clone(), s.within, s.skip))
            .collect();
        assert_eq!(
            scelta.map(|k| ora[k].clone()),
            Some(elenco[i].clone()),
            "voce {i}: prima {elenco:?}, dopo {ora:?}"
        );
        // e l'elenco e' lo stesso di prima, nello stesso ordine: la voce scelta non salta in cima e le altre non si rimescolano
        assert_eq!(ora, elenco, "voce {i}: l'elenco ha cambiato ordine");
    }
    let _ = std::fs::remove_file(p);
}

/// Una regola che legge poche righe, tutte all'inizio: l'ultima non si trova (si cerca solo nelle ultime righe), ma il calendario si apre
/// lo stesso, sul 2019 della prima.
#[test]
fn a_rule_that_reads_only_a_few_lines_still_opens_the_calendar_on_its_first_date() {
    let mut dati = String::from(
        "2026-11-03 11:00:00 [INF] run @ 2019-09-10 08:00 start
",
    );
    for i in 0..6000u32 {
        dati.push_str(&format!(
            "2026-11-03 11:{:02}:00 [INF] riga senza data {i}
",
            i % 60
        ));
    }
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_treopen4_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        chrome: false,
        time_rules: vec![("'@ 'yyyy-MM-dd HH:mm".to_string(), 200, 0)],
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.open_time_dialog();
    let i = a
        .time_sources()
        .iter()
        .position(|s| s.pattern == "'@ 'yyyy-MM-dd HH:mm")
        .expect("la regola e' nell'elenco");
    a.dialog_time_pick_source(i);
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None, "{}", a.time_error_text());
    a.open_time_dialog();
    let cal = a.time_calendar(2).expect("il calendario");
    assert_eq!(cal.month, (2019, 9), "{cal:?}");
    assert_eq!(cal.first, Some((2019, 9, 10)));
    // la fine non si e' trovata: il suggerimento "Al" non dice un ultimo orario che non e' stato letto
    match a.dialog_view() {
        Some(mrp_tui::app::dialogs::DialogView::Time {
            from_hint, to_hint, ..
        }) => {
            assert!(!from_hint.is_empty());
            assert_eq!(to_hint, "");
        }
        altro => panic!("la finestra Filtri, non {altro:?}"),
    }
    let _ = std::fs::remove_file(p);
}

/// "Mie regole": una si toglie dall'elenco e anche dal file delle impostazioni; un tempo che non e' una regola non si toglie.
#[test]
fn taking_a_rule_off_the_list_removes_it_from_the_settings_file_too() {
    let (_, p) = app();
    let imp = std::env::temp_dir().join(format!("mrp_trm_{}_settings.txt", std::process::id()));
    let _ = std::fs::remove_file(&imp);
    let mia = ("yyyy-MM-dd HH:mm".to_string(), 0, 0);
    let altra = ("'@ 'yyyy-MM-dd HH:mm".to_string(), 200, 0);
    let cfg = AppConfig {
        chrome: false,
        time_rules: vec![mia.clone(), altra.clone()],
        settings_path: Some(imp.clone()),
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    assert!(a.wait_idle(Duration::from_secs(20)));
    a.open_time_dialog();
    let voce = |a: &App, r: &(String, usize, usize)| {
        a.time_sources()
            .iter()
            .position(|s| s.pattern == r.0 && s.within == r.1 && s.skip == r.2)
    };
    // un tempo trovato da solo non e' una regola: toglierlo non fa niente (neppure se l'utente ha scritto la stessa regola: e' la
    // data dopo "@ ", che il file offre comunque)
    let n = a.time_sources().len();
    a.dialog_time_remove_rule(0);
    let uguale = voce(&a, &altra).expect("la data dopo \"@ \" e' nell'elenco");
    a.dialog_time_remove_rule(uguale);
    assert_eq!(a.time_sources().len(), n);
    assert_eq!(a.time_rules().len(), 2);
    // la regola dell'utente esce dall'elenco e dalle regole
    let i = voce(&a, &mia).expect("la mia regola e' nell'elenco");
    a.dialog_time_remove_rule(i);
    assert_eq!(voce(&a, &mia), None);
    assert_eq!(a.time_rules(), vec![altra.clone()]);
    // e dal file delle impostazioni
    let scritto = std::fs::read_to_string(&imp).unwrap();
    assert!(
        !scritto.contains("time_rule=yyyy-MM-dd HH:mm|0|0"),
        "{scritto}"
    );
    assert!(
        scritto.contains("time_rule='@ 'yyyy-MM-dd HH:mm|200|0"),
        "{scritto}"
    );
    let _ = std::fs::remove_file(imp);
    let _ = std::fs::remove_file(p);
}
