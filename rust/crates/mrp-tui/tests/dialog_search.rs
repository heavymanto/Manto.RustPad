//! Cerca e Sostituisci come dati (B4a-2): `DialogView::Find` e `DialogView::Replace` portano i testi, il modo, le maiuscole,
//! l'ambito, lo stato dell'anteprima, e le azioni `dialog_set_text`, `dialog_set_field`, `dialog_focus`, `dialog_set_mode`,
//! `dialog_set_case`, `dialog_set_scope_all` li guidano come farebbero i tasti. Scritti da ARCH: gli attesi sono calcolati a
//! mano. Il file di prova e' "foo Bar baz bar\n": "Bar" comincia al byte 4, "bar" al 12.

use mrp_core::pattern::Mode;
use mrp_tui::app::{App, AppConfig, DialogKind, ReplaceAsk, ReplaceSpec};
use mrp_tui::dialogs::{DialogView, SearchLabels};
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn file(dati: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_dsearch_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    p
}

fn app(dati: &[u8]) -> (App, PathBuf) {
    let p = file(dati);
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(70, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn ws(dati: &[u8]) -> (Workspace, PathBuf) {
    let p = file(dati);
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let a = App::open(&p, cfg.clone()).unwrap();
    let mut ws = Workspace::new(a, cfg);
    ws.resize(70, 24);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    (ws, p)
}

fn k(c: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(c, m)
}

fn ctrl(c: char) -> KeyEvent {
    k(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn etichette_en() -> SearchLabels {
    SearchLabels {
        mode: "mode".to_string(),
        text: "text".to_string(),
        glob: "glob".to_string(),
        regex: "regex".to_string(),
        case: "match case".to_string(),
        scope_one: "this tab".to_string(),
        scope_all: "all tabs".to_string(),
        invert: "Hide the lines that match instead".to_string(),
    }
}

// ----- Cerca -----

#[test]
fn find_is_a_prompt_with_mode_and_case_options() {
    let (mut a, p) = app(b"foo Bar baz bar\n");
    a.handle_key(ctrl('f'));
    assert!(a.dialog_has_view());
    let DialogView::Find {
        title,
        hint,
        text,
        mode,
        case,
        labels,
        go,
        invert,
    } = a.dialog_view().unwrap()
    else {
        panic!("non e' Cerca");
    };
    assert_eq!(title, "FIND");
    assert_eq!(hint, "text to search for (exact bytes)");
    assert_eq!((text.as_str(), mode, case), ("", Mode::Text, true));
    assert_eq!(labels, etichette_en());
    assert_eq!(go, "Find");
    assert_eq!(invert, None, "l'opzione del filtro c'e' solo nel filtro");
    let _ = std::fs::remove_file(p);
}

#[test]
fn find_runs_with_the_chosen_case_option() {
    let (mut a, p) = app(b"foo Bar baz bar\n");
    // con le maiuscole: solo "bar" minuscolo, al byte 12
    a.handle_key(ctrl('f'));
    a.dialog_set_text("bar");
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((12, 15)));
    // senza: il primo e' "Bar", al byte 4 (dal cursore, che ora e' a 12: il giro riparte e lo trova)
    a.handle_key(ctrl('f'));
    a.dialog_set_case(false);
    a.dialog_set_text("bar");
    let DialogView::Find { case, text, .. } = a.dialog_view().unwrap() else {
        panic!();
    };
    assert!(!case);
    assert_eq!(text, "bar");
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(
        a.found(),
        Some((4, 7)),
        "ignorando le maiuscole il giro riparte da \"Bar\""
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn find_modes_text_glob_and_regex() {
    let (mut a, p) = app(b"foo Bar baz bar\n");
    a.handle_key(ctrl('f'));
    a.dialog_set_mode(Mode::Regex);
    a.dialog_set_text("b[a-z]z");
    let DialogView::Find { mode, .. } = a.dialog_view().unwrap() else {
        panic!();
    };
    assert_eq!(mode, Mode::Regex);
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((8, 11)), "baz");
    a.handle_key(ctrl('f'));
    a.dialog_set_mode(Mode::Glob);
    a.dialog_set_text("b?z");
    a.dialog_submit();
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((8, 11)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn escape_closes_find_without_searching() {
    let (mut a, p) = app(b"foo Bar baz bar\n");
    a.handle_key(ctrl('f'));
    a.dialog_set_text("bar");
    a.dialog_cancel();
    assert_eq!(a.dialog_kind(), None);
    assert!(a.found().is_none());
    let _ = std::fs::remove_file(p);
}

// ----- Sostituisci -----

#[test]
fn replace_is_two_fields_options_and_a_status() {
    let (mut a, p) = app(b"foo Bar baz bar\n");
    a.handle_key(ctrl('h'));
    let DialogView::Replace {
        title,
        find_label,
        with_label,
        find,
        with,
        field,
        mode,
        case,
        all_tabs,
        status,
        ready,
        labels,
        preview,
        apply,
    } = a.dialog_view().unwrap()
    else {
        panic!("non e' Sostituisci");
    };
    assert_eq!(
        (title.as_str(), find_label.as_str(), with_label.as_str()),
        ("REPLACE", "find", "with")
    );
    assert_eq!((find.as_str(), with.as_str(), field), ("", "", 0));
    assert_eq!(
        (mode, case, all_tabs, status.as_str(), ready),
        (Mode::Text, true, false, "", false)
    );
    assert_eq!(labels, etichette_en());
    assert_eq!(
        (preview.as_str(), apply.as_str()),
        ("Preview", "Replace all")
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_fields_the_options_and_the_focus_are_set_by_actions() {
    let (mut a, p) = app(b"foo Bar baz bar\n");
    a.handle_key(ctrl('h'));
    a.dialog_set_field(0, "bar");
    a.dialog_set_field(1, "BAZ");
    a.dialog_focus(1);
    a.dialog_set_mode(Mode::Glob);
    a.dialog_set_case(false);
    a.dialog_set_scope_all(true);
    let DialogView::Replace {
        find,
        with,
        field,
        mode,
        case,
        all_tabs,
        ..
    } = a.dialog_view().unwrap()
    else {
        panic!();
    };
    assert_eq!((find.as_str(), with.as_str(), field), ("bar", "BAZ", 1));
    assert_eq!((mode, case, all_tabs), (Mode::Glob, false, true));
    // un campo che non c'e': niente
    a.dialog_set_field(7, "x");
    a.dialog_focus(9);
    let DialogView::Replace {
        find, with, field, ..
    } = a.dialog_view().unwrap()
    else {
        panic!();
    };
    assert_eq!((find.as_str(), with.as_str()), ("bar", "BAZ"));
    assert_eq!(
        field, 1,
        "9 e' dispari: l'altro campo non esiste, il fuoco resta o va a un campo valido"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn submitting_asks_for_the_preview_with_the_options() {
    let (mut a, p) = app(b"foo Bar baz bar\n");
    a.handle_key(ctrl('h'));
    a.dialog_set_field(0, "bar");
    a.dialog_set_field(1, "qux");
    a.dialog_set_case(false);
    a.dialog_set_scope_all(true);
    a.dialog_submit();
    assert_eq!(
        a.take_replace_request(),
        Some(ReplaceAsk {
            spec: ReplaceSpec {
                find: "bar".to_string(),
                with: "qux".to_string(),
                mode: Mode::Text,
                case: false,
            },
            apply: false,
            all_tabs: true,
        })
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn an_empty_find_field_says_so_and_asks_for_nothing() {
    let (mut a, p) = app(b"foo\n");
    a.handle_key(ctrl('h'));
    a.dialog_submit();
    assert!(a.take_replace_request().is_none());
    let DialogView::Replace { status, .. } = a.dialog_view().unwrap() else {
        panic!();
    };
    assert_eq!(status, "type what to find");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_preview_then_the_replacement_through_the_workspace() {
    let (mut w, p) = ws(b"a a a\n");
    w.handle_key(ctrl('h'));
    w.active_mut().dialog_set_field(0, "a");
    w.active_mut().dialog_set_field(1, "b");
    w.active_mut().dialog_submit();
    w.tick();
    assert!(w.wait_idle(Duration::from_secs(20)));
    let DialogView::Replace { status, ready, .. } = w.active().dialog_view().unwrap() else {
        panic!("il dialogo resta aperto dopo l'anteprima");
    };
    assert_eq!(
        status,
        "3 matches in 1 tab(s): Enter replaces all, Esc cancels"
    );
    assert!(ready);
    assert_eq!(
        w.active().document_text(),
        b"a a a\n",
        "l'anteprima non cambia niente"
    );
    // cambiare un'opzione toglie l'anteprima
    w.active_mut().dialog_set_case(false);
    let DialogView::Replace { ready, status, .. } = w.active().dialog_view().unwrap() else {
        panic!();
    };
    assert!(!ready && status.is_empty());
    // di nuovo l'anteprima, poi si applica
    w.active_mut().dialog_submit();
    w.tick();
    assert!(w.wait_idle(Duration::from_secs(20)));
    w.active_mut().dialog_submit();
    w.tick();
    assert!(w.wait_idle(Duration::from_secs(20)));
    assert_eq!(w.active().document_text(), b"b b b\n");
    // un solo annulla riporta il testo
    w.handle_key(ctrl('z'));
    assert_eq!(w.active().document_text(), b"a a a\n");
    let _ = std::fs::remove_file(p);
}

#[test]
fn editing_a_field_takes_the_preview_away() {
    let (mut w, p) = ws(b"a a a\n");
    w.handle_key(ctrl('h'));
    w.active_mut().dialog_set_field(0, "a");
    w.active_mut().dialog_submit();
    w.tick();
    assert!(w.wait_idle(Duration::from_secs(20)));
    let DialogView::Replace { ready, .. } = w.active().dialog_view().unwrap() else {
        panic!();
    };
    assert!(ready);
    w.active_mut().dialog_set_field(1, "x");
    let DialogView::Replace { ready, .. } = w.active().dialog_view().unwrap() else {
        panic!();
    };
    assert!(!ready);
    let _ = std::fs::remove_file(p);
}

#[test]
fn all_dialogs_have_a_view_now() {
    let (mut a, p) = app(b"foo\n");
    for (tasto, kind) in [
        (k(KeyCode::F(1), KeyModifiers::NONE), DialogKind::Help),
        (k(KeyCode::F(6), KeyModifiers::NONE), DialogKind::Theme),
        (k(KeyCode::F(7), KeyModifiers::NONE), DialogKind::Lang),
        (k(KeyCode::F(9), KeyModifiers::NONE), DialogKind::Encoding),
        (ctrl('g'), DialogKind::Goto),
        (ctrl('f'), DialogKind::Find),
        (ctrl('h'), DialogKind::Replace),
        (ctrl('o'), DialogKind::Open),
    ] {
        a.handle_key(tasto);
        assert_eq!(a.dialog_kind(), Some(kind));
        assert!(a.dialog_has_view(), "{kind:?}");
        assert!(a.dialog_view().is_some(), "{kind:?}");
        a.dialog_cancel();
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn every_option_change_takes_the_preview_away() {
    for cambia in 0..3 {
        let (mut w, p) = ws(b"a a a\n");
        w.handle_key(ctrl('h'));
        w.active_mut().dialog_set_field(0, "a");
        w.active_mut().dialog_submit();
        w.tick();
        assert!(w.wait_idle(Duration::from_secs(20)));
        let DialogView::Replace { ready, .. } = w.active().dialog_view().unwrap() else {
            panic!();
        };
        assert!(ready, "opzione {cambia}: l'anteprima e' pronta");
        match cambia {
            0 => w.active_mut().dialog_set_mode(Mode::Regex),
            1 => w.active_mut().dialog_set_case(false),
            _ => w.active_mut().dialog_set_scope_all(true),
        }
        let DialogView::Replace { ready, .. } = w.active().dialog_view().unwrap() else {
            panic!();
        };
        assert!(
            !ready,
            "opzione {cambia}: cambiata, l'anteprima non vale piu'"
        );
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn the_replace_fields_are_not_touched_from_other_dialogs() {
    let (mut a, p) = app(b"foo\n");
    a.handle_key(ctrl('f'));
    a.dialog_set_field(0, "zzz");
    a.dialog_set_field(1, "yyy");
    a.dialog_cancel();
    a.handle_key(ctrl('h'));
    let DialogView::Replace { find, with, .. } = a.dialog_view().unwrap() else {
        panic!();
    };
    assert_eq!((find.as_str(), with.as_str()), ("", ""));
    let _ = std::fs::remove_file(p);
}
