//! La barra di stato "a colpo d'occhio": riga / righe totali, colonna, byte e percentuale; la selezione (byte e righe, o righe
//! per colonne per un blocco); gli interruttori accesi (caratteri speciali, colori dei log) cliccabili. Scritti da ARCH: gli attesi
//! sono calcolati a mano sul file "uno\ndue\ntre\nquattro\n" (righe a 0, 4, 8, 12).

use mrp_core::lang::Catalog;
use mrp_gui::barre::testi_stato;
use mrp_tui::app::{App, AppConfig, SelInfo, StatusInfo};
use mrp_tui::commands::Cmd;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn app() -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_stato_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, "uno\ndue\ntre\nquattro\n").unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(40, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn tasto(a: &mut App, c: KeyCode, m: KeyModifiers) {
    a.handle_key(KeyEvent::new(c, m));
}

fn sinistra(s: &StatusInfo) -> String {
    testi_stato(s, &Catalog::english()).sinistra
}

#[test]
fn the_position_says_the_line_out_of_the_total() {
    let (mut a, p) = app();
    let s = a.status();
    assert_eq!(s.line_count, Some(4));
    assert_eq!(sinistra(&s), "Ln 1 / 4   Col 1   byte 0   0%");
    tasto(&mut a, KeyCode::Down, KeyModifiers::NONE);
    tasto(&mut a, KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(sinistra(&a.status()), "Ln 2 / 4   Col 2   byte 5   25%");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_stream_selection_says_bytes_and_lines() {
    let (mut a, p) = app();
    assert_eq!(a.status().selection, None);
    // Maiusc+Destra x2 = "un" (2 byte, 1 riga)
    for _ in 0..2 {
        tasto(&mut a, KeyCode::Right, KeyModifiers::SHIFT);
    }
    let s = a.status();
    assert_eq!(
        s.selection,
        Some(SelInfo {
            bytes: 2,
            lines: Some(1),
            block: None
        })
    );
    assert_eq!(
        sinistra(&s),
        "Ln 1 / 4   Col 3   byte 2   10%   Selected: 2 bytes, 1 lines"
    );
    // Maiusc+Giu x2 da li': dal byte 0 al byte 10 ("uno\ndue\ntr" = 10 byte), righe 1..3 = 3 righe
    tasto(&mut a, KeyCode::Down, KeyModifiers::SHIFT);
    tasto(&mut a, KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(
        a.status().selection,
        Some(SelInfo {
            bytes: 10,
            lines: Some(3),
            block: None
        })
    );
    // Ctrl+A: tutto, in lettura non oltre l'ultimo a-capo (19 byte, 4 righe)
    tasto(&mut a, KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert_eq!(
        a.status().selection,
        Some(SelInfo {
            bytes: 19,
            lines: Some(4),
            block: None
        })
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_column_block_says_rows_by_columns() {
    let (mut a, p) = app();
    tasto(&mut a, KeyCode::F(8), KeyModifiers::NONE);
    for _ in 0..2 {
        tasto(&mut a, KeyCode::Right, KeyModifiers::SHIFT);
    }
    for _ in 0..2 {
        tasto(&mut a, KeyCode::Down, KeyModifiers::SHIFT);
    }
    let s = a.status();
    assert_eq!(
        s.selection,
        Some(SelInfo {
            bytes: 0,
            lines: None,
            block: Some((3, 2))
        })
    );
    assert!(
        sinistra(&s).ends_with("Block: 3 rows × 2 columns"),
        "{}",
        sinistra(&s)
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn selection_without_the_line_index_says_only_bytes() {
    let mut s = app().0.status();
    s.line = None;
    s.line_count = None;
    s.selection = Some(SelInfo {
        bytes: 7,
        lines: None,
        block: None,
    });
    assert_eq!(
        sinistra(&s),
        "Ln ? / ?   Col 1   byte 0   0%   Selected: 7 bytes"
    );
}

#[test]
fn the_switches_are_always_on_the_right_grey_when_off_and_a_click_changes_them() {
    let (mut a, p) = app();
    let cat = Catalog::english();
    let pezzi = |a: &mut App| testi_stato(&a.status(), &cat).destra;
    // spenti stanno in barra lo stesso (un clic li riaccende), in grigio
    let v = pezzi(&mut a);
    let sp = v.iter().find(|p| p.comando == Some(Cmd::Special)).unwrap();
    let lg = v
        .iter()
        .find(|p| p.comando == Some(Cmd::LogColors))
        .unwrap();
    assert!(sp.spento && lg.spento);
    a.set_special_chars(true, false);
    a.set_log_colors(true, false);
    let v = pezzi(&mut a);
    let sp = v.iter().find(|p| p.comando == Some(Cmd::Special)).unwrap();
    let lg = v
        .iter()
        .find(|p| p.comando == Some(Cmd::LogColors))
        .unwrap();
    assert_eq!((sp.testo.as_str(), lg.testo.as_str()), ("\u{b6}", "LOG"));
    assert!(!sp.spento && !lg.spento);
    // stanno prima della codifica
    let i_sp = v
        .iter()
        .position(|p| p.comando == Some(Cmd::Special))
        .unwrap();
    let i_enc = v
        .iter()
        .position(|p| p.comando == Some(Cmd::Encoding))
        .unwrap();
    assert!(i_sp < i_enc);
    let _ = std::fs::remove_file(p);
}
