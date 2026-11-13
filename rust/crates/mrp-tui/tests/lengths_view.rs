//! Il check dei tracciati nelle viste filtrate: INVIO apre la scheda di sola lettura col nome delle lunghezze; dentro una
//! vista, una nuova selezione rifà il filtro dal file ORIGINALE componendo le condizioni (il testo della vista e le
//! lunghezze nuove insieme); e il dialogo riaperto nella vista mostra tutte le lunghezze del file originale (istogramma
//! copiato), con quelle della vista gia' spuntate: spuntarle tutte non lascia nessuna condizione e si torna al file,
//! alla riga del cursore. Scritti da ARCH: 24 righe con tre lunghezze (18 da 40, quattro da 41, due da 12), tutte
//! con una "x" nel corpo: gli attesi sono l'aritmetica di questa costruzione.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::dialogs::DialogView;
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// La riga `i`: 40 byte il tracciato, 41 per `i % 7 == 2` (2, 9, 16, 23), 12 per `i` = 3 e 20; tutte con una "x".
fn riga(i: u32) -> String {
    let (target, prefisso) = match i {
        3 | 20 => (12, "corta"),
        _ if i % 7 == 2 => (41, "lunga"),
        _ => (40, "riga"),
    };
    let mut s = format!("{prefisso} {i:03}");
    while s.len() < target {
        s.push('x');
    }
    s.truncate(target);
    format!("{s}\n")
}

fn ws() -> (PathBuf, Workspace) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_lengths_view_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("a.log");
    std::fs::write(&p, (0..24u32).map(riga).collect::<String>()).unwrap();
    let cfg = AppConfig::default();
    let app = App::open(&p, cfg.clone()).unwrap();
    let mut w = Workspace::new(app, cfg);
    w.resize(100, 30);
    assert!(w.wait_idle(Duration::from_secs(20)));
    (d, w)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn alt(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

fn schermo(w: &mut Workspace) -> String {
    w.render_lines(100, 30).join("\n")
}

/// Alt+H nella scheda attiva e aspetta la fine della passata (le voci del dialogo piene).
fn apri(w: &mut Workspace) {
    w.handle_key(alt('h'));
    let scadenza = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(w.wait_idle(Duration::from_secs(5)));
        if let Some(DialogView::Lengths {
            items,
            progress: None,
            ..
        }) = w.active().dialog_view()
        {
            if !items.is_empty() {
                return;
            }
        }
        assert!(
            Instant::now() < scadenza,
            "la passata delle lunghezze non finisce"
        );
    }
}

#[test]
fn enter_opens_a_read_only_view_named_after_the_lengths() {
    let (d, mut w) = ws();
    apri(&mut w);
    // solo il tracciato da 40: A svuota la selezione, SPAZIO spunta la prima
    w.handle_key(k(KeyCode::Char('a')));
    w.handle_key(k(KeyCode::Char(' ')));
    w.handle_key(k(KeyCode::Enter));
    assert!(w.wait_idle(Duration::from_secs(20)));
    assert_eq!(w.len(), 2);
    assert_eq!(w.active_index(), 1);
    assert!(w.active().is_filter_view());
    assert_eq!(w.active().file_name(), "[filter] lengths: 40 B");
    assert!(schermo(&mut w).contains("18 lines"), "{}", schermo(&mut w));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn in_a_view_the_new_lengths_compose_with_the_text_of_the_view() {
    let (d, mut w) = ws();
    // una vista di testo: tutte le righe hanno una "x" nel corpo
    w.handle_key(alt('g'));
    w.active_mut().dialog_set_text("x");
    w.active_mut().dialog_submit();
    assert!(w.wait_idle(Duration::from_secs(20)));
    assert_eq!(w.active().file_name(), "[filter] x");
    assert!(schermo(&mut w).contains("24 lines"), "{}", schermo(&mut w));
    // nella vista: solo le righe da 40 (A svuota, SPAZIO spunta la prima)
    apri(&mut w);
    w.handle_key(k(KeyCode::Char('a')));
    w.handle_key(k(KeyCode::Char(' ')));
    w.handle_key(k(KeyCode::Enter));
    assert!(w.wait_idle(Duration::from_secs(20)));
    // il rifiltro parte dal file originale e compone: il testo della vista E le lunghezze nuove
    assert_eq!(w.len(), 2, "la vista nuova prende il posto della vecchia");
    assert!(w.active().is_filter_view());
    assert!(
        w.active().file_name().contains("lengths: 40 B"),
        "{}",
        w.active().file_name()
    );
    assert!(schermo(&mut w).contains("18 lines"), "{}", schermo(&mut w));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn all_checked_in_a_lengths_only_view_goes_back_to_the_file() {
    let (d, mut w) = ws();
    apri(&mut w);
    w.handle_key(k(KeyCode::Char('a')));
    w.handle_key(k(KeyCode::Char(' ')));
    w.handle_key(k(KeyCode::Enter));
    assert!(w.wait_idle(Duration::from_secs(20)));
    assert!(w.active().is_filter_view());
    // nella vista il dialogo mostra tutte le lunghezze del FILE (istogramma copiato dall'originale)
    // con la 40 gia' spuntata (le condizioni della vista): per tornare al file si spuntano anche le
    // altre due — 41 e 12, in fondo alla lista (18 da 40, quattro da 41, due da 12) — e INVIO
    apri(&mut w);
    w.handle_key(k(KeyCode::Down));
    w.handle_key(k(KeyCode::Char(' ')));
    w.handle_key(k(KeyCode::Down));
    w.handle_key(k(KeyCode::Char(' ')));
    w.handle_key(k(KeyCode::Enter));
    assert!(w.wait_idle(Duration::from_secs(20)));
    assert_eq!(w.active_index(), 0, "si torna al file originale");
    assert!(!w.active().is_filter_view());
    assert_eq!(w.len(), 2, "la vista resta aperta");
    assert_eq!(w.active().cursor(), 0, "alla riga del cursore della vista");
    let _ = std::fs::remove_dir_all(d);
}
