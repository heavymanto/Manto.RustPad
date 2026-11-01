//! "Vai a" un orario (fase 2, 3.4): nel prompt di Ctrl+G (F5) un testo che comincia con `~` e' un orario; il cursore va all'inizio della
//! prima riga con quell'orario o uno dopo (il log ha gli orari in ordine crescente). Scritti da ARCH: il log ha 600 righe, una al
//! minuto da 2026-10-31 00:00, ogni riga "2026-10-31 HH:MM:00 riga N\n".

use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// L'App sul log e gli offset d'inizio di ogni riga.
fn app() -> (App, PathBuf, Vec<u64>) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_gototime_{}_{}.log", std::process::id(), n));
    let mut t = String::new();
    let mut inizi = Vec::new();
    for i in 0..600u64 {
        inizi.push(t.len() as u64);
        t.push_str(&format!(
            "2026-10-31 {:02}:{:02}:00 riga {i}\n",
            i / 60,
            i % 60
        ));
    }
    std::fs::write(&p, t).unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(60, 10);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let _ = render_lines(&mut a, 60, 10);
    (a, p, inizi)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn vai(a: &mut App, testo: &str) {
    a.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    for c in testo.chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
    a.handle_key(k(KeyCode::Enter));
    let _ = render_lines(a, 60, 10);
}

#[test]
fn a_time_goes_to_the_first_line_at_or_after_it() {
    let (mut a, p, inizi) = app();
    // 03:20 = riga 200
    vai(&mut a, "~03:20");
    assert_eq!(a.cursor(), inizi[200]);
    vai(&mut a, "~2026-10-31 05:00:00");
    assert_eq!(a.cursor(), inizi[300]);
    // fra due righe: la dopo (03:20:30 -> 03:21)
    vai(&mut a, "~03:20:30");
    assert_eq!(a.cursor(), inizi[201]);
    // prima di tutto: la prima riga
    vai(&mut a, "~2026-10-30");
    assert_eq!(a.cursor(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_line_is_on_the_screen_after_the_jump() {
    let (mut a, p, _) = app();
    vai(&mut a, "~07:30");
    let s = render_lines(&mut a, 60, 10).join("\n");
    assert!(s.contains("2026-10-31 07:30:00 riga 450"), "{s}");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_bad_time_and_a_time_after_the_end_say_so_and_do_not_move() {
    let (mut a, p, inizi) = app();
    vai(&mut a, "~03:20");
    let qui = a.cursor();
    assert_eq!(qui, inizi[200]);
    vai(&mut a, "~25:99");
    assert_eq!(a.cursor(), qui);
    let m = a.status().flash.unwrap_or_default();
    assert!(m.contains("not a valid position"), "{m:?}");
    vai(&mut a, "~2027-01-01");
    assert_eq!(a.cursor(), qui);
    let m = a.status().flash.unwrap_or_default();
    assert!(m.contains("no line with that time"), "{m:?}");
    vai(&mut a, "~");
    assert_eq!(a.cursor(), qui);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_other_forms_still_work() {
    let (mut a, p, inizi) = app();
    vai(&mut a, "@10");
    assert_eq!(a.cursor(), 10);
    vai(&mut a, "%50");
    assert!(a.cursor() > inizi[250] && a.cursor() < inizi[350]);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_hint_mentions_the_time_form() {
    let (mut a, p, _) = app();
    a.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    let s = render_lines(&mut a, 60, 10).join("\n");
    assert!(s.contains("~time"), "{s}");
    let _ = std::fs::remove_file(p);
}
