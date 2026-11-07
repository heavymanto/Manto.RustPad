//! L'aiuto (F1) che scorre e le voci in piu' del piede (Ctrl+R, Alt+B). Scritti da ARCH.
//! L'aiuto ha 25 righe (l'ultima, Esc, e' la 25a): in uno schermo di 24 righe ne stanno 21 (24 meno il bordo, 2, e la riga che dice
//! come si chiude, 1), quindi scorre di al piu' 4 righe. L'ordine e' quello di `HELP_KEYS`.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn app(testo: &str, w: u16, h: u16) -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_helpfoot_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, testo).unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(w, h);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn schermo(a: &mut App, w: u16, h: u16) -> String {
    render_lines(a, w, h).join("\n")
}

#[test]
fn the_help_lists_every_key_that_was_missing() {
    // alto abbastanza per vederle tutte: 25 righe + bordo + la riga che chiude = 28
    let (mut a, p) = app("uno\n", 100, 40);
    a.handle_key(k(KeyCode::F(1)));
    let s = schermo(&mut a, 100, 40);
    for atteso in [
        "Alt+T / Alt+O",
        "time range / time format of the lines",
        "Alt+B",
        "timeline: the file in time, next to the text",
        "Alt+X",
        "line index on / off",
        "Ctrl+R",
        "discard unsaved changes (back to the file on disk)",
        "Ctrl+Z / Ctrl+Y",
        "undo / redo (edit mode)",
        "Esc closes",
    ] {
        assert!(s.contains(atteso), "manca {atteso:?}:\n{s}");
    }
    // sta tutto: niente da scorrere, niente suggerimento
    assert!(!s.contains("Up/Down scroll"), "{s}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn on_a_short_screen_the_help_scrolls_and_always_shows_how_to_close() {
    let (mut a, p) = app("uno\n", 100, 24);
    a.handle_key(k(KeyCode::F(1)));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Help));
    // in cima: le prime 21 righe (fino a Alt+T / Alt+O), non le ultime; in fondo come si chiude e che si scorre
    let s = schermo(&mut a, 100, 24);
    assert!(s.contains("Arrows PgUp PgDn Home End"), "{s}");
    assert!(s.contains("Alt+T / Alt+O"), "{s}");
    assert!(!s.contains("Ctrl+R"), "{s}");
    assert!(!s.contains("timeline: the file in time"), "{s}");
    assert!(s.contains("Esc closes"), "{s}");
    assert!(s.contains("Up/Down scroll"), "{s}");
    // Giu: una riga in piu' in fondo (Alt+B), la prima sparisce
    a.handle_key(k(KeyCode::Down));
    let s = schermo(&mut a, 100, 24);
    assert!(!s.contains("Arrows PgUp PgDn Home End"), "{s}");
    assert!(s.contains("Ctrl+Home / Ctrl+End"), "{s}");
    assert!(s.contains("timeline: the file in time"), "{s}");
    assert!(!s.contains("Ctrl+R"), "{s}");
    // Su: com'era
    a.handle_key(k(KeyCode::Up));
    assert!(schermo(&mut a, 100, 24).contains("Arrows PgUp PgDn Home End"));
    // Su in cima non va oltre
    a.handle_key(k(KeyCode::Up));
    assert!(schermo(&mut a, 100, 24).contains("Arrows PgUp PgDn Home End"));
    // Fine: le ultime righe, compresa Ctrl+R; le prime quattro non si vedono piu'
    a.handle_key(k(KeyCode::End));
    let s = schermo(&mut a, 100, 24);
    assert!(s.contains("Ctrl+R"), "{s}");
    assert!(s.contains("Esc closes"), "{s}");
    assert!(!s.contains("Arrows PgUp PgDn Home End"), "{s}");
    assert!(!s.contains("start / end of the file"), "{s}");
    // Giu in fondo non va oltre: anche dopo cinque Giu, un Su sposta di una riga sola
    let prima = schermo(&mut a, 100, 24);
    for _ in 0..5 {
        a.handle_key(k(KeyCode::Down));
    }
    assert_eq!(schermo(&mut a, 100, 24), prima);
    a.handle_key(k(KeyCode::Up));
    let su = schermo(&mut a, 100, 24);
    assert_ne!(su, prima);
    // la riga che prima era appena sopra la finestra (undo / redo) torna in cima
    assert!(su.contains("undo / redo"), "{su}");
    assert!(!su.contains("Arrows PgUp PgDn Home End"), "{su}");
    a.handle_key(k(KeyCode::End));
    // Home: di nuovo in cima. PgGiu salta di una pagina (20 righe) fino in fondo, PgSu torna in cima
    a.handle_key(k(KeyCode::Home));
    assert!(schermo(&mut a, 100, 24).contains("Arrows PgUp PgDn Home End"));
    a.handle_key(k(KeyCode::PageDown));
    assert!(schermo(&mut a, 100, 24).contains("Ctrl+R"));
    a.handle_key(k(KeyCode::PageUp));
    assert!(schermo(&mut a, 100, 24).contains("Arrows PgUp PgDn Home End"));
    // l'aiuto riaperto riparte dall'inizio
    a.handle_key(k(KeyCode::End));
    a.handle_key(k(KeyCode::Esc));
    assert_eq!(a.dialog_kind(), None);
    a.handle_key(k(KeyCode::F(1)));
    assert!(schermo(&mut a, 100, 24).contains("Arrows PgUp PgDn Home End"));
    // gli altri tasti non lo chiudono
    a.handle_key(k(KeyCode::Char('x')));
    assert_eq!(a.dialog_kind(), Some(DialogKind::Help));
    let _ = std::fs::remove_file(p);
}

/// L'ultima riga dello schermo (il piede).
fn piede(a: &mut App, w: u16) -> String {
    render_lines(a, w, 24).pop().unwrap()
}

#[test]
fn the_footer_gets_the_extra_keys_only_where_there_is_room() {
    let (mut a, p) = app("uno\n", 200, 24);
    // a 80 colonne il piede e' pieno com'era; le voci in piu' non ci sono
    a.resize(80, 24);
    let s80 = piede(&mut a, 80);
    assert!(s80.contains("F10"), "{s80}");
    assert!(!s80.contains("Alt+B") && !s80.contains("Ctrl+R"), "{s80}");
    // largo: la timeline c'e'; scartare no, perche' non c'e' niente da scartare
    a.resize(200, 24);
    let s200 = piede(&mut a, 200);
    assert!(s200.contains("Alt+B Timeline"), "{s200}");
    assert!(!s200.contains("Ctrl+R"), "{s200}");
    // il tail e' l'ultima voce e c'e' dove c'e' posto per tutte, mai prima della timeline
    assert!(s200.contains("Alt+F Tail"), "{s200}");
    assert!(
        s200.find("Alt+B").unwrap() < s200.find("Alt+F").unwrap(),
        "{s200}"
    );
    // con una modifica compare anche Ctrl+R, prima di Alt+B
    a.handle_key(k(KeyCode::F(4)));
    a.handle_key(k(KeyCode::Char('x')));
    let s200 = piede(&mut a, 200);
    assert!(s200.contains("Ctrl+R Discard"), "{s200}");
    assert!(
        s200.find("Ctrl+R").unwrap() < s200.find("Alt+B").unwrap(),
        "{s200}"
    );
    // la precedenza: a nessuna larghezza si vede Alt+B senza Ctrl+R, e quando le voci non ci stanno non si tagliano a meta'
    let (mut visto_r, mut visto_b) = (false, false);
    for w in 80..=200u16 {
        a.resize(w, 24);
        let s = piede(&mut a, w);
        let (r, b) = (s.contains("Ctrl+R Discard"), s.contains("Alt+B Timeline"));
        assert!(!b || r, "a {w} colonne c'e' Alt+B ma non Ctrl+R: {s}");
        assert!(
            r || !s.contains("Ctrl+R"),
            "a {w} colonne Ctrl+R e' tagliata: {s}"
        );
        assert!(
            b || !s.contains("Alt+B"),
            "a {w} colonne Alt+B e' tagliata: {s}"
        );
        assert!(s.contains("F10"), "a {w} colonne manca F10: {s}");
        // e finisce con una voce intera, mai con un pezzo ("Ctr", "Alt+B Timelin")
        let fine = s.trim_end();
        assert!(
            fine.ends_with("Quit")
                || fine.ends_with("Discard")
                || fine.ends_with("Timeline")
                || fine.ends_with("Tail"),
            "a {w} colonne il piede e' tagliato: {s}"
        );
        visto_r |= r;
        visto_b |= b;
    }
    assert!(visto_r && visto_b);
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_untitled_document_never_offers_to_discard() {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "mrp_helpfoot_untitled_{}_{}",
        std::process::id(),
        n
    ));
    let mut a = App::new_untitled(&dir.join("untitled"), 1, AppConfig::default()).unwrap();
    a.resize(200, 24);
    a.handle_key(k(KeyCode::Char('x')));
    assert!(a.is_modified());
    let s = piede(&mut a, 200);
    assert!(!s.contains("Ctrl+R"), "{s}");
    assert!(s.contains("Alt+B Timeline"), "{s}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_descriptions_in_the_help_start_all_at_the_same_column() {
    let (mut a, p) = app("uno\n", 120, 40);
    a.handle_key(k(KeyCode::F(1)));
    let righe = render_lines(&mut a, 120, 40);
    let mut colonne = Vec::new();
    for (tasti, descrizione) in a.help_rows() {
        let riga = righe
            .iter()
            .find(|r| r.contains(&tasti) && r.contains(&descrizione))
            .unwrap_or_else(|| panic!("manca {tasti:?} / {descrizione:?}:\n{}", righe.join("\n")));
        let byte = riga.find(&descrizione).unwrap();
        colonne.push((riga[..byte].chars().count(), tasti));
    }
    assert_eq!(colonne.len(), 27);
    let prima = colonne[0].0;
    for (c, tasti) in &colonne {
        assert_eq!(
            *c,
            prima,
            "la descrizione di {tasti:?} e' fuori colonna:\n{}",
            righe.join("\n")
        );
    }
    let _ = std::fs::remove_file(p);
}
