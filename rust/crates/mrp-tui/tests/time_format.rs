//! Il formato dell'orario nella vita vera (F1, seconda mano): un log con grandi tabelle fra le righe con l'orario (come un backtest),
//! un log il cui orario compare solo in fondo, il formato standard ricordato nelle impostazioni, e i formati noti da scegliere nel
//! dialogo. Scritti da ARCH.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::dialogs::DialogView;
use mrp_tui::settings::Settings;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_tfmt_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn app_con(dati: &str, cfg: AppConfig) -> (App, PathBuf) {
    let d = cartella();
    let p = d.join("a.log");
    std::fs::write(&p, dati).unwrap();
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, d)
}

/// Un log come un backtest: una riga con l'orario, poi una tabella di quaranta righe senza.
fn log_a_tabelle(blocchi: usize) -> String {
    let mut s = String::new();
    for i in 0..blocchi {
        s.push_str(&format!(
            "2026-11-03 11:14:{:02}.738 +01:00 [INF] BALANCE USDT: 0\r\n",
            i % 60
        ));
        for k in 0..39 {
            s.push_str(&format!(
                "|     [SEC-{k:02}] OPEN NEW POSITION      |    NONE --> DOWN / NONE --> LONG    |\r\n"
            ));
        }
    }
    s
}

fn alt(a: &mut App, c: char) {
    a.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT));
}

#[test]
fn a_log_with_big_tables_is_recognised_like_the_real_case() {
    let (mut a, d) = app_con(&log_a_tabelle(8), AppConfig::default());
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss.SSS")
    );
    assert_eq!(a.time_at_top().as_deref(), Some("2026-11-03 11:14:00.738"));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_time_that_shows_up_only_at_the_end_of_a_long_file_is_found_in_the_tail() {
    // 200 KB di banner senza orari, poi un log: il campione dell'inizio non basta, quello della coda si'
    let mut s = String::new();
    while s.len() < 200_000 {
        s.push_str("################################################################ banner\n");
    }
    for i in 0..30 {
        s.push_str(&format!("2026-11-03 11:14:{i:02}.738 [INF] x\n"));
    }
    let (mut a, d) = app_con(&s, AppConfig::default());
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss.SSS")
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_saved_standard_format_is_tried_first() {
    let cfg = AppConfig {
        time_format: Some(("yyyy-MM-dd HH:mm:ss".to_string(), 0)),
        ..AppConfig::default()
    };
    let (mut a, d) = app_con(&log_a_tabelle(4), cfg);
    // il log ha i millisecondi, ma la scelta standard e' quella senza: vince la standard
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss")
    );
    assert_eq!(
        a.time_default(),
        Some(("yyyy-MM-dd HH:mm:ss".to_string(), 0))
    );
    let _ = std::fs::remove_dir_all(d);
    // se il file non la legge, si riconosce da solo
    let cfg = AppConfig {
        time_format: Some(("dd/MM/yyyy HH:mm:ss".to_string(), 0)),
        ..AppConfig::default()
    };
    let (mut a, d) = app_con(&log_a_tabelle(4), cfg);
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss.SSS")
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_format_chosen_in_the_dialog_becomes_the_standard_and_is_saved() {
    let d = cartella();
    let settings = d.join("profilo").join("settings.txt");
    let cfg = AppConfig {
        settings_path: Some(settings.clone()),
        ..AppConfig::default()
    };
    let p = d.join("a.log");
    std::fs::write(&p, log_a_tabelle(4)).unwrap();
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.time_default(), None);
    alt(&mut a, 'o');
    a.dialog_set_field(0, "yyyy-MM-dd HH:mm");
    a.dialog_submit();
    assert_eq!(a.time_default(), Some(("yyyy-MM-dd HH:mm".to_string(), 0)));
    let scritto = std::fs::read_to_string(&settings).unwrap();
    assert!(
        scritto.contains("time_format=yyyy-MM-dd HH:mm"),
        "{scritto}"
    );
    assert!(scritto.contains("time_within=0"), "{scritto}");
    // e si rilegge
    let s = Settings::from_text(&scritto);
    assert_eq!(s.time_format, Some(("yyyy-MM-dd HH:mm".to_string(), 0)));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_settings_keep_a_valid_format_and_drop_a_bad_one() {
    let s = Settings::from_text("time_format=dd/MMM/yyyy:HH:mm:ss\ntime_within=40\n");
    assert_eq!(
        s.time_format,
        Some(("dd/MMM/yyyy:HH:mm:ss".to_string(), 40))
    );
    // un pattern che non va si ignora; un numero fuori misura si limita; senza formato il numero non conta
    assert_eq!(Settings::from_text("time_format=yyyyy\n").time_format, None);
    let s = Settings::from_text("time_format=HH:mm:ss\ntime_within=9999\n");
    assert_eq!(s.time_format, Some(("HH:mm:ss".to_string(), 200)));
    assert_eq!(Settings::from_text("time_within=5\n").time_format, None);
    // senza formato scelto non si scrive niente
    assert!(!Settings::default().to_text().contains("time_format"));
}

#[test]
fn the_known_formats_can_be_chosen_in_the_dialog() {
    let (mut a, d) = app_con(&log_a_tabelle(3), AppConfig::default());
    alt(&mut a, 'o');
    assert_eq!(a.dialog_kind(), Some(DialogKind::TimeMask));
    let Some(DialogView::TimeMask {
        presets, preset, ..
    }) = a.dialog_view()
    else {
        panic!("manca la vista");
    };
    assert_eq!(presets.len(), 19);
    assert_eq!(presets[0].0, "yyyy-MM-dd HH:mm:ss.SSS");
    assert_eq!(presets[0].1, "2026-11-01 02:00:00.877");
    assert_eq!(preset, 0);
    // sceglierne uno mette il pattern (e la finestra) nei campi
    let apache = presets
        .iter()
        .position(|(p, _)| p == "dd/MMM/yyyy:HH:mm:ss")
        .unwrap();
    a.dialog_time_preset(apache);
    assert_eq!(a.time_mask_text(0), "dd/MMM/yyyy:HH:mm:ss");
    assert_eq!(a.time_mask_text(1), "40");
    // Alt+N / Alt+P scorrono l'elenco
    alt(&mut a, 'n');
    assert_eq!(a.time_mask_text(0), presets[apache + 1].0);
    alt(&mut a, 'p');
    alt(&mut a, 'p');
    assert_eq!(a.time_mask_text(0), presets[apache - 1].0);
    // e dopo l'ultimo si ricomincia dal primo
    a.dialog_time_preset(18);
    alt(&mut a, 'n');
    assert_eq!(a.time_mask_text(0), presets[0].0);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_status_carries_the_format_of_the_file() {
    let (mut a, d) = app_con(&log_a_tabelle(3), AppConfig::default());
    assert_eq!(
        a.status().time_format.as_deref(),
        Some("yyyy-MM-dd HH:mm:ss.SSS")
    );
    let (mut b, d2) = app_con("niente orari qui\naltra riga\n", AppConfig::default());
    assert_eq!(b.status().time_format, None);
    let _ = std::fs::remove_dir_all(d);
    let _ = std::fs::remove_dir_all(d2);
}

#[test]
fn the_dialog_says_what_was_recognised_and_how_many_lines_each_known_format_reads() {
    // 3 blocchi: 3 righe con l'orario "yyyy-MM-dd HH:mm:ss.SSS +01:00" e 117 righe di tabella senza
    let (mut a, d) = app_con(&log_a_tabelle(3), AppConfig::default());
    alt(&mut a, 'o');
    let Some(DialogView::TimeMask {
        presets,
        matches,
        detected,
        preset,
        ..
    }) = a.dialog_view()
    else {
        panic!("manca la vista");
    };
    assert_eq!(matches.len(), presets.len());
    // riconosciuto: l'orario della prima riga e il pattern
    assert_eq!(
        detected,
        Some((
            "2026-11-03 11:14:00.738".to_string(),
            "yyyy-MM-dd HH:mm:ss.SSS".to_string()
        ))
    );
    // quelli che leggono il file: con i millisecondi (3 righe), senza (3 righe) e la sola data (3 righe); gli altri 0
    let i = |p: &str| presets.iter().position(|(q, _)| q == p).unwrap();
    assert_eq!(matches[i("yyyy-MM-dd HH:mm:ss.SSS")], 3);
    assert_eq!(matches[i("yyyy-MM-dd HH:mm:ss")], 3);
    assert_eq!(matches[i("yyyy-MM-dd")], 3);
    assert_eq!(matches[i("dd/MMM/yyyy:HH:mm:ss")], 0);
    assert_eq!(matches[i("HH:mm:ss")], 0);
    // il formato in uso e' quello evidenziato
    assert_eq!(preset, i("yyyy-MM-dd HH:mm:ss.SSS"));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_file_without_times_says_none_was_recognised_and_every_format_reads_zero_lines() {
    let (mut a, d) = app_con("niente orari qui\naltra riga\n", AppConfig::default());
    alt(&mut a, 'o');
    let Some(DialogView::TimeMask {
        matches, detected, ..
    }) = a.dialog_view()
    else {
        panic!("manca la vista");
    };
    assert_eq!(detected, None);
    assert!(matches.iter().all(|&n| n == 0), "{matches:?}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn choosing_a_known_format_in_the_dialog_changes_the_headline_after_ok() {
    let (mut a, d) = app_con(&log_a_tabelle(3), AppConfig::default());
    alt(&mut a, 'o');
    a.dialog_set_field(0, "yyyy-MM-dd");
    a.dialog_submit();
    alt(&mut a, 'o');
    let Some(DialogView::TimeMask { detected, .. }) = a.dialog_view() else {
        panic!("manca la vista");
    };
    assert_eq!(
        detected,
        Some(("2026-11-03 00:00:00".to_string(), "yyyy-MM-dd".to_string()))
    );
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_rules_written_by_hand_are_saved_read_back_and_limited() {
    let s = Settings {
        time_rules: vec![
            ("'@ 'yyyy-MM-dd HH:mm".to_string(), 200, 0),
            ("yyyy-MM-dd HH:mm".to_string(), 200, 1),
        ],
        ..Settings::default()
    };
    let testo = s.to_text();
    assert!(
        testo.contains("time_rule='@ 'yyyy-MM-dd HH:mm|200|0\n"),
        "{testo}"
    );
    assert!(
        testo.contains("time_rule=yyyy-MM-dd HH:mm|200|1\n"),
        "{testo}"
    );
    assert_eq!(Settings::from_text(&testo).time_rules, s.time_rules);
    // senza regole niente si scrive
    assert!(!Settings::default().to_text().contains("time_rule"));
    // un pattern che non va, una riga senza i tre pezzi e un numero fuori misura: le regole buone restano, il resto si limita o si salta
    let t = Settings::from_text(
        "time_rule=yyyyy|0|0\ntime_rule=senza pezzi\ntime_rule=HH:mm:ss|999|99\ntime_rule=HH:mm:ss|999|99\n",
    );
    assert_eq!(t.time_rules, vec![("HH:mm:ss".to_string(), 200, 9)]);
    // al piu' otto
    let molte: String = (0..12)
        .map(|i| format!("time_rule='x{i} 'yyyy-MM-dd HH:mm|200|0\n"))
        .collect();
    assert_eq!(Settings::from_text(&molte).time_rules.len(), 8);
    // un pattern con la barra verticale dentro si rilegge (si divide dalla destra)
    let barra = Settings::from_text("time_rule='a|b 'yyyy-MM-dd HH:mm|200|0\n");
    assert_eq!(
        barra.time_rules,
        vec![("'a|b 'yyyy-MM-dd HH:mm".to_string(), 200, 0)]
    );
}

#[test]
fn a_rule_written_in_one_tab_reaches_the_other_tabs_and_the_next_ones() {
    use mrp_tui::workspace::Workspace;
    // la finestra con i controlli nativi: la finestra "Tempo"
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let (a, d) = app_con(&log_a_tabelle(3), cfg.clone());
    let mut w = Workspace::new(a, cfg);
    w.resize(100, 24);
    let altro = d.join("b.log");
    std::fs::write(
        &altro, "x
",
    )
    .unwrap();
    w.open(&altro).unwrap();
    w.select(0);
    // una regola scritta a mano: un formato che non e' fra quelli noti
    w.active_mut().open_time_dialog();
    w.active_mut().dialog_set_field(0, "yyyy-MM-dd HH");
    w.active_mut().dialog_submit();
    w.tick();
    let regola = vec![("yyyy-MM-dd HH".to_string(), 0, 0)];
    assert_eq!(w.tabs()[0].time_rules(), regola);
    // l'altra scheda la conosce, e' una riga dell'elenco anche li' (se il file la legge)
    assert_eq!(w.tabs()[1].time_rules(), regola);
    let _ = std::fs::remove_dir_all(d);
}
