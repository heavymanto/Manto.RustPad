//! Il dialogo del check dei tracciati nell'`App` (Alt+H): la passata in background conta l'istogramma delle lunghezze
//! delle righe, la lista esce per conteggio decrescente (i formati dominanti prima, le anomalie in fondo), SPAZIO spunta
//! la voce sotto il cursore, A inverte, INVIO applica e compone con le altre condizioni della vista filtrata (tutte
//! spuntate = nessun filtro), ESC chiude senza toccare niente. Scritti da ARCH: il file ha 128 righe con tre lunghezze
//! sole, costruite qui (il tracciato da 40 byte, sei righe da 41, due da 12): gli attesi sono l'aritmetica di questa
//! costruzione, mai l'output del programma.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::dialogs::DialogView;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// La riga `i`: contenuto lungo esattamente `target` byte (solo ASCII, quindi i byte sono i caratteri). Il tracciato e'
/// 40 byte; sei righe (`i % 21 == 5`: 5, 26, 47, 68, 89, 110) sono 41; due (`i` = 0 e 64) sono 12, le anomalie.
fn riga(i: u32) -> String {
    let (target, prefisso) = match i {
        0 | 64 => (12, "corta"),
        _ if i % 21 == 5 => (41, "lunga"),
        _ => (40, "riga"),
    };
    let mut s = format!("{prefisso} {i:03}");
    while s.len() < target {
        s.push('x');
    }
    s.truncate(target);
    format!("{s}\n")
}

fn app() -> (App, PathBuf) {
    let dati: String = (0..128u32).map(riga).collect();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_lengths_app_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 30);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn alt(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

/// Le voci del dialogo aperto (la passata e' gia' finita): (lunghezza, conteggio, spuntata).
fn items(a: &App) -> Vec<(u64, u64, bool)> {
    match a.dialog_view() {
        Some(DialogView::Lengths { items, .. }) => items,
        _ => panic!("il dialogo delle lunghezze non e' aperto"),
    }
}

/// Apre il dialogo con Alt+H e aspetta la fine della passata: le voci, per conteggio decrescente, tutte spuntate.
fn apri(a: &mut App) {
    a.handle_key(alt('h'));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Lengths));
    let scadenza = Instant::now() + Duration::from_secs(20);
    loop {
        a.lengths_tick();
        if let Some(DialogView::Lengths {
            items,
            progress: None,
            ..
        }) = a.dialog_view()
        {
            if !items.is_empty() {
                return;
            }
        }
        assert!(
            Instant::now() < scadenza,
            "la passata delle lunghezze non finisce"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn alt_h_opens_the_dialog_with_the_histogram_by_count() {
    let (mut a, p) = app();
    apri(&mut a);
    // per conteggio decrescente: il tracciato (120 righe), le sei da 41, le due anomale da 12 in fondo
    assert_eq!(
        items(&a),
        vec![(40, 120, true), (41, 6, true), (12, 2, true)]
    );
    assert!(matches!(
        a.dialog_view(),
        Some(DialogView::Lengths { title, .. }) if title == "LENGTHS"
    ));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_keys_move_the_bar_and_space_toggles_the_row_under_it() {
    let (mut a, p) = app();
    apri(&mut a);
    // SPAZIO sulla prima (il cursore parte da 0): la spunta si toglie e si rimette
    a.handle_key(k(KeyCode::Char(' ')));
    assert!(!items(&a)[0].2);
    a.handle_key(k(KeyCode::Char(' ')));
    assert!(items(&a)[0].2);
    // GIU': la barra segue il dialogo nuovo, la spunta tocca la riga sotto e non la prima
    a.handle_key(k(KeyCode::Down));
    a.handle_key(k(KeyCode::Char(' ')));
    assert!(!items(&a)[1].2);
    assert!(items(&a)[0].2, "la prima riga non si tocca");
    // FINE: il cursore sull'ultima voce, l'anomala
    a.handle_key(k(KeyCode::End));
    a.handle_key(k(KeyCode::Char(' ')));
    assert!(!items(&a)[2].2);
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_inverts_and_enter_applies_the_anomalous_lengths_only() {
    let (mut a, p) = app();
    apri(&mut a);
    // A svuota la selezione: la via piu' corta per "solo le anomale" e' scegliere quelle in fondo
    a.handle_key(k(KeyCode::Char('a')));
    assert!(items(&a).iter().all(|i| !i.2));
    a.handle_key(k(KeyCode::End));
    a.handle_key(k(KeyCode::Char(' ')));
    assert_eq!(items(&a)[2], (12, 2, true));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("la vista delle anomalie");
    assert_eq!(fatto.lines, 2);
    assert_eq!(fatto.pattern, "lengths: 12 B");
    assert_eq!(
        fatto.conditions.expect("le condizioni").lengths,
        Some(HashSet::from([12]))
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn esc_closes_without_filtering_and_the_dialog_reopens_from_the_filter() {
    let (mut a, p) = app();
    apri(&mut a);
    a.handle_key(k(KeyCode::Char(' ')));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.dialog_kind(), None);
    assert!(a.wait_idle(Duration::from_secs(5)));
    assert!(a.take_filter_done().is_none(), "ESC non filtra");
    // la selezione abortita non si ricorda: si riparte dal filtro (qui: nessuno, tutto spuntato)
    apri(&mut a);
    assert!(items(&a).iter().all(|i| i.2));
    // un filtro vero: solo il tracciato da 40
    a.handle_key(k(KeyCode::Char('a')));
    a.handle_key(k(KeyCode::Char(' ')));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("la vista del tracciato");
    assert_eq!(
        (fatto.lines, fatto.pattern.as_str()),
        (120, "lengths: 40 B")
    );
    // questa scheda e' la vista: riaprire propone le lunghezze del filtro, non tutte
    a.set_filter_view(fatto);
    apri(&mut a);
    assert_eq!(
        items(&a),
        vec![(40, 120, true), (41, 6, false), (12, 2, false)]
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_empty_selection_refuses_with_no_line_matches() {
    let (mut a, p) = app();
    apri(&mut a);
    a.handle_key(k(KeyCode::Char('a')));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(
        a.take_filter_done().is_none(),
        "nessuna riga: nessuna vista"
    );
    assert_eq!(a.flash(), Some("no line matches"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn in_a_lengths_view_a_new_selection_refilters_with_the_new_lengths() {
    let (mut a, p) = app();
    apri(&mut a);
    a.handle_key(k(KeyCode::Char('a')));
    a.handle_key(k(KeyCode::Char(' ')));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    let fatto = a.take_filter_done().expect("la vista del tracciato");
    a.set_filter_view(fatto);
    // nella vista (la selezione parte dal filtro: {40}): anche l'anomala da 12
    apri(&mut a);
    a.handle_key(k(KeyCode::End));
    a.handle_key(k(KeyCode::Char(' ')));
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.dialog_kind(), None);
    // le nuove condizioni vanno al file originale: la vista chiede il rifiltro, non se ne va
    let rifiltro = a
        .take_refilter()
        .expect("il rifiltro con le lunghezze nuove");
    assert_eq!(rifiltro.lengths, Some(HashSet::from([12, 40])));
    assert!(
        a.take_goto_original().is_none(),
        "resta una condizione: niente ritorno al file"
    );
    let _ = std::fs::remove_file(p);
}
