//! L'indicizzazione si ferma mentre l'utente salta nel file (scrollbar, vai a, salti di pagina, Ctrl+Fine) e riprende da
//! sola poco dopo l'ultimo salto. Scritti da ARCH.

use mrp_tui::app::{App, AppConfig};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Un file da 8 GiB (zeri, allocato senza scriverlo): l'indicizzazione dura abbastanza da trovarla in corso anche con la
/// cache calda e tanti processori.
fn file_enorme() -> std::path::PathBuf {
    // un nome per file: i test girano in parallelo
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_pausa_indice_{}_{}.bin", std::process::id(), n));
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(8 << 30).unwrap();
    p
}

fn apri() -> (App, std::path::PathBuf) {
    let p = file_enorme();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(80, 24);
    assert!(
        a.status().busy.is_some(),
        "l'indicizzazione deve essere in corso"
    );
    assert!(!a.index_paused(), "all'inizio non e' in pausa");
    (a, p)
}

/// Chiude senza aspettare l'indice (chiudere l'App lo annulla).
fn finisci(a: App, p: std::path::PathBuf) {
    drop(a);
    let _ = std::fs::remove_file(p);
}

#[test]
fn dragging_the_scrollbar_pauses_the_index_and_it_resumes_after_a_while() {
    let (mut a, p) = apri();
    a.scroll_to_fraction(0.5);
    assert!(a.index_paused());
    // un altro salto subito dopo tiene la pausa
    std::thread::sleep(Duration::from_millis(200));
    a.scroll_to_fraction(0.6);
    a.tick();
    assert!(a.index_paused());
    // senza altri salti riprende da sola
    std::thread::sleep(Duration::from_millis(700));
    a.tick();
    assert!(!a.index_paused());
    finisci(a, p);
}

#[test]
fn goto_and_the_page_jumps_and_ctrl_end_pause_the_index_too() {
    for (nome, azione) in [
        ("vai a", 0),
        ("Alt+PagGiu", 1),
        ("Ctrl+Fine", 2),
        ("Ctrl+Inizio", 3),
    ] {
        let (mut a, p) = apri();
        match azione {
            0 => a.goto_offset(4096),
            1 => a.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::ALT)),
            2 => a.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL)),
            _ => a.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL)),
        }
        assert!(a.index_paused(), "{nome}");
        finisci(a, p);
    }
}

#[test]
fn plain_typing_and_arrows_do_not_pause_the_index() {
    let (mut a, p) = apri();
    a.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    a.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert!(!a.index_paused());
    finisci(a, p);
}
