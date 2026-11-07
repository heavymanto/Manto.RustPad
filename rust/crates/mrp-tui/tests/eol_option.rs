//! L'opzione `--eol lf|crlf|cr` (Fase 2, 1.4): impone lo stile di fine riga che scrive Invio (e che la riga di stato mostra),
//! al posto di quello riconosciuto dal file o di quello della piattaforma per un documento nuovo. Le righe che ci sono gia' non
//! si toccano. Scritti da ARCH: gli attesi sono calcolati a mano.

use mrp_core::lineending::LineEnding;
use mrp_tui::app::{App, AppConfig};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Out {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run(args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_mrp-tui"))
        .env("MRP_NO_PROFILE", "1")
        .args(args)
        .output()
        .unwrap();
    Out {
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        code: o.status.code().unwrap_or(-1),
    }
}

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_eol_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    p
}

/// Il testo della riga di stato del dump (la penultima riga dello schermo).
fn stato(o: &Out) -> String {
    let righe: Vec<&str> = o.stdout.lines().collect();
    righe[righe.len() - 2].to_string()
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

#[test]
fn the_option_sets_the_style_the_status_line_shows() {
    let lf = file(b"uno\ndue\n");
    let p = lf.to_str().unwrap();
    let o = run(&["--dump", "60x5", p]);
    assert!(
        !stato(&o).contains("LF") && !stato(&o).contains("CR"),
        "senza opzione: LF, non si scrive: {}",
        stato(&o)
    );
    let o = run(&["--eol", "crlf", "--dump", "60x5", p]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(stato(&o).contains("CRLF"), "{}", stato(&o));
    let o = run(&["--eol", "CR", "--dump", "60x5", p]);
    let s = stato(&o);
    assert!(s.contains("CR") && !s.contains("CRLF"), "{s}");
    let _ = std::fs::remove_file(lf);
    // un file CRLF: senza opzione CRLF, con --eol lf niente etichetta
    let crlf = file(b"uno\r\ndue\r\n");
    let p = crlf.to_str().unwrap();
    assert!(stato(&run(&["--dump", "60x5", p])).contains("CRLF"));
    assert!(!stato(&run(&["--eol", "lf", "--dump", "60x5", p])).contains("CRLF"));
    let _ = std::fs::remove_file(crlf);
}

#[test]
fn a_bad_style_is_a_usage_error() {
    let f = file(b"uno\n");
    let o = run(&["--eol", "windows", f.to_str().unwrap()]);
    assert_eq!(o.code, 2);
    assert_eq!(
        o.stderr.trim(),
        "unknown line ending: windows (lf, crlf, cr)"
    );
    let o = run(&["--eol"]);
    assert_eq!(o.code, 2, "senza valore");
    let _ = std::fs::remove_file(f);
}

#[test]
fn help_lists_the_option_in_every_language() {
    let o = run(&["--help"]);
    assert!(o.stdout.contains("--eol"), "{}", o.stdout);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang");
    for l in ["de", "es", "it", "ja", "pt-BR", "zh-CN"] {
        let o = run(&["--lang-dir", dir.to_str().unwrap(), "--lang", l, "--help"]);
        assert!(o.stdout.contains("--eol"), "{l}: {}", o.stdout);
    }
}

// ----- l'App -----

fn app(dati: &[u8], eol: Option<LineEnding>) -> (App, PathBuf) {
    let p = file(dati);
    let cfg = AppConfig {
        eol,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(60, 7);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

#[test]
fn enter_writes_the_forced_style() {
    for (eol, atteso) in [
        (LineEnding::Lf, "\n"),
        (LineEnding::CrLf, "\r\n"),
        (LineEnding::Cr, "\r"),
    ] {
        // un file senza a-capo (niente da riconoscere) e uno con l'altro stile: l'opzione vince
        for dati in [&b"ab"[..], b"x\ny\n", b"x\r\ny\r\n"] {
            let (mut a, p) = app(dati, Some(eol));
            a.handle_key(modifica());
            a.handle_key(k(KeyCode::Enter));
            let t = a.document_text();
            assert!(
                t.starts_with(atteso.as_bytes()),
                "{eol:?} su {dati:?}: {t:?}"
            );
            assert_eq!(a.line_ending(), eol);
            let _ = std::fs::remove_file(p);
        }
    }
}

#[test]
fn without_the_option_the_file_decides_as_before() {
    let (a, p) = app(b"x\r\ny\r\n", None);
    assert_eq!(a.line_ending(), LineEnding::CrLf);
    let (b, q) = app(b"x\ny\n", None);
    assert_eq!(b.line_ending(), LineEnding::Lf);
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn the_forced_style_leaves_the_existing_lines_alone() {
    let (mut a, p) = app(b"uno\ndue\n", Some(LineEnding::CrLf));
    a.handle_key(modifica());
    a.handle_key(k(KeyCode::Enter));
    assert_eq!(a.document_text(), b"\r\nuno\ndue\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_new_untitled_document_takes_the_forced_style_too() {
    let d = std::env::temp_dir().join(format!("mrp_eol_untitled_{}", std::process::id()));
    for (eol, atteso) in [(LineEnding::Lf, "\n"), (LineEnding::CrLf, "\r\n")] {
        let cfg = AppConfig {
            eol: Some(eol),
            ..AppConfig::default()
        };
        let mut a = App::new_untitled(&d, 1, cfg).unwrap();
        a.resize(60, 7);
        a.handle_key(k(KeyCode::Enter));
        assert_eq!(a.document_text(), atteso.as_bytes(), "{eol:?}");
        drop(a);
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn forcing_cr_on_a_file_with_line_feeds_keeps_its_lines_apart() {
    // l'a-capo del documento resta quello del file: le righe che ci sono non si fondono in una
    let f = file(b"uno\ndue\n");
    let o = run(&["--eol", "cr", "--dump", "60x5", f.to_str().unwrap()]);
    let righe: Vec<&str> = o.stdout.lines().collect();
    assert!(
        righe[0].contains("uno") && !righe[0].contains("due"),
        "{:?}",
        righe[0]
    );
    assert!(righe[1].contains("due"), "{:?}", righe[1]);
    let _ = std::fs::remove_file(f);
}

#[test]
fn forcing_cr_on_a_file_without_line_breaks_makes_enter_break_the_line() {
    let (mut a, p) = app(b"ab", Some(LineEnding::Cr));
    a.handle_key(modifica());
    a.handle_key(k(KeyCode::Enter));
    let righe = mrp_tui::ui::render_lines(&mut a, 60, 6);
    // riga 1 vuota, riga 2 "ab": il CR e' un a-capo vero per il documento
    assert!(righe[0].ends_with('│'), "{:?}", righe[0]);
    assert!(righe[1].ends_with("ab"), "{:?}", righe[1]);
    let _ = std::fs::remove_file(p);
}

/// Alt+E: passa alla modifica (F4 e' Sostituisci).
fn modifica() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
}
