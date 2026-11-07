//! Sostituisci e le ricerche con regole nel TUI (E36b): il dialogo (cerca, con, modo, maiuscole, ambito), l'anteprima prima
//! di applicare, l'applicazione come un solo passo annullabile, tutte le schede, e le opzioni nella ricerca normale.
//! Scritti da ARCH: gli attesi sono calcolati a mano sui testi di prova.

use mrp_core::encoding::Encoding;
use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::workspace::Workspace;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_rep_test_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn scrivi(dir: &Path, nome: &str, dati: &[u8]) -> PathBuf {
    let p = dir.join(nome);
    std::fs::write(&p, dati).unwrap();
    p
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn alt(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

fn ws_con(dir: &Path, file: &[(&str, &[u8])]) -> Workspace {
    let cfg = AppConfig::default();
    let mut w: Option<Workspace> = None;
    for (n, dati) in file {
        let p = scrivi(dir, n, dati);
        match &mut w {
            None => {
                w = Some(Workspace::new(
                    App::open(&p, cfg.clone()).unwrap(),
                    cfg.clone(),
                ))
            }
            Some(x) => x.open(&p).unwrap(),
        }
    }
    let mut w = w.unwrap();
    w.select(0);
    w.resize(100, 14);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w
}

fn digita(w: &mut Workspace, testo: &str) {
    for c in testo.chars() {
        w.handle_key(k(KeyCode::Char(c)));
    }
}

/// Aspetta che l'anteprima o la sostituzione finisca.
fn aspetta(w: &mut Workspace) {
    for _ in 0..1000 {
        w.tick();
        if !w.replace_busy() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("la sostituzione non finisce");
}

/// Lo schermo come un testo solo.
fn schermo(w: &mut Workspace) -> String {
    w.render_lines(100, 14).join("\n")
}

/// Apre il dialogo e scrive i due campi.
fn apri_sostituisci(w: &mut Workspace, cerca: &str, con: &str) {
    w.handle_key(ctrl('h'));
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::Replace));
    // i campi si ricordano da una volta all'altra: si svuotano e si scrive
    for _ in 0..40 {
        w.handle_key(k(KeyCode::Backspace));
    }
    digita(w, cerca);
    w.handle_key(k(KeyCode::Tab));
    for _ in 0..40 {
        w.handle_key(k(KeyCode::Backspace));
    }
    digita(w, con);
}

fn testo(w: &Workspace) -> Vec<u8> {
    w.active().document_text()
}

fn pulisci(d: PathBuf) {
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn replace_shows_a_preview_first_and_then_applies_as_one_undo_step() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"uno due uno\ndue uno\n")]);
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    // l'anteprima: il documento non e' toccato
    assert!(schermo(&mut w).contains("3 matches in 1 tab(s): Enter replaces all, Esc cancels"));
    assert_eq!(testo(&w), b"uno due uno\ndue uno\n");
    assert!(!w.active().is_modified());
    // il secondo Invio applica
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"1 due 1\ndue 1\n");
    assert_eq!(w.active().dialog_kind(), None);
    assert_eq!(
        w.active().flash(),
        Some("replaced 3 occurrence(s) in 1 tab(s)")
    );
    assert!(w.active().is_modified());
    assert!(w.active().is_edit(), "sostituire porta in modifica");
    // un solo annulla toglie tutte e tre
    w.handle_key(ctrl('z'));
    assert_eq!(testo(&w), b"uno due uno\ndue uno\n");
    pulisci(d);
}

#[test]
fn escape_closes_the_dialog_without_touching_anything() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"uno\n")]);
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    w.handle_key(k(KeyCode::Esc));
    assert_eq!(w.active().dialog_kind(), None);
    assert_eq!(testo(&w), b"uno\n");
    assert!(!w.active().is_modified());
    pulisci(d);
}

#[test]
fn changing_anything_after_the_preview_asks_for_a_new_preview() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"uno\n")]);
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    // un carattere in piu' nel campo "con": l'anteprima e' vecchia
    digita(&mut w, "2");
    assert!(!schermo(&mut w).contains("matches in"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(
        testo(&w),
        b"uno\n",
        "e' una nuova anteprima, non l'applicazione"
    );
    assert!(schermo(&mut w).contains("1 matches in 1 tab(s)"));
    // anche un'opzione cambiata (Alt+C) la toglie
    w.handle_key(alt('c'));
    assert!(!schermo(&mut w).contains("matches in"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"uno\n");
    // Backspace sul campo "con" (il cursore e' li') la toglie e cambia il testo: "12" -> "1"
    w.handle_key(k(KeyCode::Backspace));
    assert!(!schermo(&mut w).contains("matches in"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"1\n");
    pulisci(d);
}

#[test]
fn regex_mode_expands_groups() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"mario@rossi luigi@verdi\nnessuno\n")]);
    w.handle_key(ctrl('h'));
    digita(&mut w, "(\\w+)@(\\w+)");
    w.handle_key(k(KeyCode::Tab));
    digita(&mut w, "$2:$1");
    // testo -> glob -> regex
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    assert!(schermo(&mut w).contains("mode: regex"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("2 matches in 1 tab(s)"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"rossi:mario verdi:luigi\nnessuno\n");
    pulisci(d);
}

#[test]
fn glob_mode_and_ignore_case() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"Cat cot CUT cit\n")]);
    w.handle_key(ctrl('h'));
    digita(&mut w, "c?t");
    w.handle_key(k(KeyCode::Tab));
    digita(&mut w, "X");
    w.handle_key(alt('m'));
    assert!(schermo(&mut w).contains("mode: glob"));
    // con le maiuscole che contano: "cot" e "cit"
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"Cat X CUT X\n");
    w.handle_key(ctrl('z'));
    // senza: tutte e quattro
    w.handle_key(ctrl('h'));
    // il campo "cerca" riparte dall'ultimo cercato ("c?t") e il campo "con" ricorda "X"
    w.handle_key(alt('c'));
    assert!(schermo(&mut w).contains("ignore case"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("4 matches in 1 tab(s)"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"X X X X\n");
    pulisci(d);
}

#[test]
fn all_tabs_previews_and_applies_everywhere_and_this_tab_touches_only_one() {
    let d = cartella();
    let mut w = ws_con(
        &d,
        &[
            ("a.txt", b"uno\n"),
            ("b.txt", b"uno uno\n"),
            ("c.txt", b"niente\n"),
        ],
    );
    // solo questa scheda
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("1 matches in 1 tab(s)"));
    w.handle_key(k(KeyCode::Esc));
    // tutte: 3 occorrenze in 2 schede su 3
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(alt('a'));
    assert!(schermo(&mut w).contains("all tabs"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("3 matches in 2 tab(s)"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(
        w.active().flash(),
        Some("replaced 3 occurrence(s) in 2 tab(s)")
    );
    let testi: Vec<Vec<u8>> = w.tabs().iter().map(|a| a.document_text()).collect();
    assert_eq!(testi[0], b"1\n");
    assert_eq!(testi[1], b"1 1\n");
    assert_eq!(testi[2], b"niente\n");
    assert!(w.tabs()[0].is_modified() && w.tabs()[1].is_modified());
    assert!(!w.tabs()[2].is_modified());
    pulisci(d);
}

#[test]
fn no_matches_and_bad_input_are_said_in_the_dialog_and_run_nothing() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"uno\n")]);
    // niente da cercare
    w.handle_key(ctrl('h'));
    w.handle_key(k(KeyCode::Enter));
    assert!(schermo(&mut w).contains("type what to find"));
    assert!(!w.replace_busy());
    // nessuna occorrenza: lo dice, e un altro Invio rifa l'anteprima (non applica)
    digita(&mut w, "zzz");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("no matches"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("no matches"));
    assert!(!w.active().is_modified());
    // regex sbagliata
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    for _ in 0..3 {
        w.handle_key(k(KeyCode::Backspace));
    }
    digita(&mut w, "(");
    w.handle_key(k(KeyCode::Enter));
    assert!(!w.replace_busy());
    assert!(schermo(&mut w).contains("invalid pattern"));
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::Replace));
    pulisci(d);
}

#[test]
fn keys_are_held_while_a_replace_runs_and_escape_stops_it() {
    let d = cartella();
    let grande = "riga di prova con uno dentro\n".repeat(50_000);
    let mut w = ws_con(&d, &[("a.txt", grande.as_bytes())]);
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(k(KeyCode::Enter));
    // subito dopo l'Invio il lavoro gira: i tasti non entrano nei campi
    assert!(w.replace_busy());
    digita(&mut w, "xyz");
    w.handle_key(k(KeyCode::Esc));
    aspetta(&mut w);
    assert!(!w.replace_busy());
    // o si e' fermata (annullata) o ha fatto in tempo a contare: in nessun caso e' stata applicata, e "xyz" non e' entrato
    assert_eq!(testo(&w), grande.as_bytes());
    let s = schermo(&mut w);
    assert!(!s.contains("xyz"));
    assert!(
        s.contains("replace cancelled") || s.contains("matches in 1 tab(s)"),
        "{s}"
    );
    assert_eq!(w.active().dialog_kind(), Some(DialogKind::Replace));
    pulisci(d);
}

#[test]
fn the_cursor_follows_the_text_it_was_on() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"aaa bbb\n")]);
    // il cursore in fondo alla riga: dopo "aaa" -> "X" (due byte in meno) sta due byte prima
    w.handle_key(k(KeyCode::End));
    assert_eq!(w.active().cursor(), 7);
    apri_sostituisci(&mut w, "aaa", "X");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"X bbb\n");
    assert_eq!(w.active().cursor(), 5);
    pulisci(d);
}

#[test]
fn a_cursor_inside_the_replaced_text_lands_after_the_new_text() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"aaa bbb\n")]);
    w.handle_key(k(KeyCode::Right));
    assert_eq!(w.active().cursor(), 1);
    apri_sostituisci(&mut w, "aaa", "XY");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"XY bbb\n");
    assert_eq!(w.active().cursor(), 2);
    pulisci(d);
}

#[test]
fn a_cursor_before_the_replacement_does_not_move() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"aaa bbb\n")]);
    w.handle_key(k(KeyCode::Right));
    apri_sostituisci(&mut w, "bbb", "Z");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"aaa Z\n");
    assert_eq!(w.active().cursor(), 1);
    pulisci(d);
}

#[test]
fn replace_keeps_earlier_edits_and_undoes_only_itself_first() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"uno\n")]);
    w.handle_key(modifica());
    digita(&mut w, ">");
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b">1\n");
    w.handle_key(ctrl('z'));
    assert_eq!(testo(&w), b">uno\n");
    w.handle_key(ctrl('z'));
    assert_eq!(testo(&w), b"uno\n");
    pulisci(d);
}

// ---- le altre codifiche ----

#[test]
fn plain_replace_works_in_shift_jis_and_regex_is_refused_there() {
    let d = cartella();
    let sj = Encoding::by_name("shift-jis").unwrap();
    let cfg = AppConfig {
        encoding: Some(sj),
        ..AppConfig::default()
    };
    let p = scrivi(&d, "j.txt", &sj.encode_str("日本 日本語\n").unwrap());
    let mut w = Workspace::new(App::open(&p, cfg.clone()).unwrap(), cfg);
    w.resize(100, 14);
    assert!(w.wait_idle(Duration::from_secs(20)));
    apri_sostituisci(&mut w, "日", "月");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("2 matches in 1 tab(s)"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), sj.encode_str("月本 月本語\n").unwrap());
    // la regex qui no: lo dice
    w.handle_key(ctrl('h'));
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("need a UTF-8 document"));
    pulisci(d);
}

#[test]
fn all_tabs_skips_the_tabs_that_cannot_use_a_regex_and_still_does_the_others() {
    let d = cartella();
    let sj = Encoding::by_name("shift-jis").unwrap();
    let cfg = AppConfig::default();
    let pa = scrivi(&d, "a.txt", b"x1 x22\n");
    let pj = scrivi(&d, "j.txt", &sj.encode_str("x3\n").unwrap());
    let mut w = Workspace::new(App::open(&pa, cfg.clone()).unwrap(), cfg);
    w.open(&pj).unwrap();
    // la seconda scheda si legge come Shift_JIS (scelta dell'utente con F9)
    w.select(1);
    w.active_mut().set_encoding(sj);
    w.select(0);
    w.resize(100, 14);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w.handle_key(ctrl('h'));
    digita(&mut w, "x\\d+");
    w.handle_key(k(KeyCode::Tab));
    digita(&mut w, "N");
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    w.handle_key(alt('a'));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    // a.txt ha due occorrenze; j.txt e' saltata
    assert!(schermo(&mut w).contains("2 matches in 1 tab(s)"));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(w.tabs()[0].document_text(), b"N N\n");
    assert_eq!(w.tabs()[1].document_text(), sj.encode_str("x3\n").unwrap());
    pulisci(d);
}

// ---- la ricerca normale con le opzioni ----

fn app_con(dir: &Path, dati: &[u8]) -> App {
    let p = scrivi(dir, "f.txt", dati);
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(100, 14);
    assert!(a.wait_idle(Duration::from_secs(20)));
    a
}

#[test]
fn find_with_ignore_case_and_glob() {
    let d = cartella();
    let mut a = app_con(&d, b"alpha Beta\nGAMMA delta\nepsilon\n");
    // senza maiuscole: "beta" trova "Beta" (a 6, lunga 4)
    a.handle_key(ctrl('f'));
    a.handle_key(alt('c'));
    for c in "beta".chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.cursor(), 6);
    assert_eq!(a.found(), Some((6, 10)));
    // glob "g*a" senza maiuscole, da 6 in avanti: il meno possibile e' "GA" (a 11, lunga 2)
    a.handle_key(ctrl('f'));
    for _ in 0..4 {
        a.handle_key(k(KeyCode::Backspace));
    }
    for c in "g*a".chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
    a.handle_key(alt('m'));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((11, 13)));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn find_regex_forward_and_backward_report_the_match_length() {
    let d = cartella();
    let mut a = app_con(&d, b"ab12 cd345 ef\nx6\n");
    a.handle_key(ctrl('f'));
    a.handle_key(alt('m'));
    a.handle_key(alt('m'));
    for c in "\\d+".chars() {
        a.handle_key(k(KeyCode::Char(c)));
    }
    a.handle_key(k(KeyCode::Enter));
    assert!(a.wait_idle(Duration::from_secs(20)));
    // "12" a 2, lunga 2: evidenziata 2..4
    assert_eq!(a.cursor(), 2);
    assert_eq!(a.found(), Some((2, 4)));
    a.handle_key(k(KeyCode::F(3)));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((7, 10)), "345");
    a.handle_key(k(KeyCode::F(3)));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((15, 16)), "6 sulla seconda riga");
    // all'indietro dal cursore (15): 345
    a.handle_key(KeyEvent::new(KeyCode::F(3), KeyModifiers::SHIFT));
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.found(), Some((7, 10)));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn find_with_a_bad_regex_says_so_and_does_not_search() {
    let d = cartella();
    let mut a = app_con(&d, b"abc\n");
    a.handle_key(ctrl('f'));
    a.handle_key(alt('m'));
    a.handle_key(alt('m'));
    a.handle_key(k(KeyCode::Char('(')));
    a.handle_key(k(KeyCode::Enter));
    assert!(a.flash().unwrap_or("").starts_with("invalid pattern"));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_find_dialog_shows_the_options_line() {
    let d = cartella();
    let mut a = app_con(&d, b"abc\n");
    a.handle_key(ctrl('f'));
    let righe = mrp_tui::ui::render_lines(&mut a, 100, 14).join("\n");
    assert!(
        righe.contains("mode: text | match case  (Alt+M, Alt+C)"),
        "{righe}"
    );
    a.handle_key(alt('m'));
    a.handle_key(alt('c'));
    let righe = mrp_tui::ui::render_lines(&mut a, 100, 14).join("\n");
    assert!(
        righe.contains("mode: glob | ignore case  (Alt+M, Alt+C)"),
        "{righe}"
    );
    let _ = std::fs::remove_dir_all(d);
}

// ---- i casi trovati dalle mutazioni ----

#[test]
fn tab_keys_do_nothing_while_a_replace_runs() {
    let d = cartella();
    let grande = "riga di prova con uno dentro\n".repeat(50_000);
    let mut w = ws_con(&d, &[("a.txt", grande.as_bytes()), ("b.txt", b"x\n")]);
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(k(KeyCode::Enter));
    assert!(w.replace_busy());
    w.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT));
    assert_eq!(
        w.active_index(),
        0,
        "non si cambia scheda durante il lavoro"
    );
    w.handle_key(ctrl('w'));
    assert_eq!(w.len(), 2, "e non si chiude");
    aspetta(&mut w);
    pulisci(d);
}

#[test]
fn escape_stops_the_jobs_of_every_tab_not_only_the_active_one() {
    let d = cartella();
    // la scheda attiva e' piccola (finisce subito), l'altra grande: Esc deve fermare anche quella
    let grande = "contatto mario@rossi nome cognome altro testo ancora\n".repeat(1_200_000);
    let mut w = ws_con(&d, &[("a.txt", b"x@y\n"), ("b.txt", grande.as_bytes())]);
    w.handle_key(ctrl('h'));
    digita(&mut w, r"\w+@\w+");
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    w.handle_key(alt('a'));
    w.handle_key(k(KeyCode::Enter));
    w.handle_key(k(KeyCode::Esc));
    aspetta(&mut w);
    assert!(
        schermo(&mut w).contains("replace cancelled"),
        "{}",
        schermo(&mut w)
    );
    pulisci(d);
}

#[test]
fn an_invalid_pattern_is_reported_even_when_the_active_tab_cannot_use_regex() {
    let d = cartella();
    let sj = Encoding::by_name("shift-jis").unwrap();
    let cfg = AppConfig {
        encoding: Some(sj),
        ..AppConfig::default()
    };
    let p = scrivi(&d, "j.txt", &sj.encode_str("abc\n").unwrap());
    let mut w = Workspace::new(App::open(&p, cfg.clone()).unwrap(), cfg);
    w.resize(100, 14);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w.handle_key(ctrl('h'));
    digita(&mut w, "(");
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    w.handle_key(k(KeyCode::Enter));
    assert!(
        schermo(&mut w).contains("invalid pattern"),
        "{}",
        schermo(&mut w)
    );
    pulisci(d);
}

#[test]
fn too_many_matches_stop_the_preview_for_literals_and_for_regexes() {
    let d = cartella();
    let dati = "a\n".repeat(2_000_001);
    let mut w = ws_con(&d, &[("a.txt", dati.as_bytes())]);
    for modo in [0, 2] {
        w.handle_key(ctrl('h'));
        for _ in 0..40 {
            w.handle_key(k(KeyCode::Backspace));
        }
        digita(&mut w, "a");
        for _ in 0..modo {
            w.handle_key(alt('m'));
        }
        w.handle_key(k(KeyCode::Enter));
        aspetta(&mut w);
        let s = schermo(&mut w);
        assert!(
            s.contains("too many matches (over 2000000): narrow the search"),
            "modo {modo}: {s}"
        );
        // e il documento non e' stato toccato
        assert!(!w.active().is_modified());
        w.handle_key(k(KeyCode::Esc));
    }
    pulisci(d);
}

#[test]
fn mode_and_scope_changes_also_drop_the_preview() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"uno\n")]);
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("matches in"));
    w.handle_key(alt('m'));
    assert!(!schermo(&mut w).contains("matches in"));
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("matches in"));
    w.handle_key(alt('a'));
    assert!(!schermo(&mut w).contains("matches in"));
    pulisci(d);
}

#[test]
fn a_cursor_exactly_where_the_replacement_starts_stays_there() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"aaa bbb\n")]);
    for _ in 0..4 {
        w.handle_key(k(KeyCode::Right));
    }
    assert_eq!(w.active().cursor(), 4);
    apri_sostituisci(&mut w, "bbb", "Z");
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert_eq!(testo(&w), b"aaa Z\n");
    assert_eq!(w.active().cursor(), 4);
    pulisci(d);
}

#[test]
fn plain_text_without_case_is_not_taken_as_exact_bytes() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"ciao Ciao CIAO\n")]);
    w.handle_key(ctrl('h'));
    digita(&mut w, "ciao");
    w.handle_key(alt('c'));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(schermo(&mut w).contains("3 matches in 1 tab(s)"));
    pulisci(d);
}

#[test]
fn the_terminal_cursor_sits_in_the_replace_dialog_on_the_active_field() {
    let d = cartella();
    let mut w = ws_con(&d, &[("a.txt", b"uno\n")]);
    w.handle_key(ctrl('h'));
    digita(&mut w, "ab");
    let _ = schermo(&mut w);
    let (x1, y1) = w.active().cursor_xy().expect("il cursore sta nel dialogo");
    w.handle_key(k(KeyCode::Tab));
    digita(&mut w, "xyz");
    let _ = schermo(&mut w);
    let (x2, y2) = w.active().cursor_xy().unwrap();
    // il campo "con" e' la riga sotto, e la colonna segue il testo scritto (3 caratteri contro 2)
    assert_eq!(y2, y1 + 1);
    assert_eq!(x2, x1 + 1);
    pulisci(d);
}

// ---- trovati dalla revisione R26C ----

#[test]
fn closing_or_opening_a_tab_through_the_api_while_a_replace_runs_is_refused() {
    // la finestra chiude e apre schede senza passare dai tasti: durante il lavoro le schede non si spostano, altrimenti
    // la sostituzione resta in attesa per sempre di una scheda che non c'e' piu'
    let d = cartella();
    let grande = "riga di prova con uno dentro\n".repeat(50_000);
    let mut w = ws_con(&d, &[("a.txt", grande.as_bytes()), ("b.txt", b"x\n")]);
    w.select(0);
    apri_sostituisci(&mut w, "uno", "1");
    w.handle_key(k(KeyCode::Enter));
    assert!(w.replace_busy());
    w.close(1);
    assert_eq!(w.len(), 2, "la chiusura e' rifiutata");
    let altro = scrivi(&d, "c.txt", b"c\n");
    w.open(&altro).unwrap();
    assert_eq!(w.len(), 2, "l'apertura e' rifiutata");
    aspetta(&mut w);
    assert!(
        schermo(&mut w).contains("50000 matches in 1 tab(s)"),
        "il lavoro e' finito regolarmente: {}",
        schermo(&mut w)
    );
    // finito il lavoro si chiude e si apre di nuovo
    w.handle_key(k(KeyCode::Esc));
    w.close(1);
    assert_eq!(w.len(), 1);
    w.open(&altro).unwrap();
    assert_eq!(w.len(), 2);
    pulisci(d);
}

#[test]
fn the_summary_says_how_many_tabs_were_skipped() {
    let d = cartella();
    let sj = Encoding::by_name("shift-jis").unwrap();
    let cfg = AppConfig::default();
    let pa = scrivi(&d, "a.txt", b"x1 x22\n");
    let pj = scrivi(&d, "j.txt", &sj.encode_str("x3\n").unwrap());
    let mut w = Workspace::new(App::open(&pa, cfg.clone()).unwrap(), cfg);
    w.open(&pj).unwrap();
    w.select(1);
    w.active_mut().set_encoding(sj);
    w.select(0);
    w.resize(100, 14);
    assert!(w.wait_idle(Duration::from_secs(20)));
    w.handle_key(ctrl('h'));
    digita(&mut w, r"x\d+");
    w.handle_key(k(KeyCode::Tab));
    digita(&mut w, "N");
    w.handle_key(alt('m'));
    w.handle_key(alt('m'));
    w.handle_key(alt('a'));
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(
        schermo(&mut w).contains("2 matches in 1 tab(s)"),
        "{}",
        schermo(&mut w)
    );
    assert!(
        schermo(&mut w).contains("(1 tab(s) skipped)"),
        "{}",
        schermo(&mut w)
    );
    w.handle_key(k(KeyCode::Enter));
    aspetta(&mut w);
    assert!(
        schermo(&mut w).contains("replaced 2 occurrence(s) in 1 tab(s) (1 tab(s) skipped)"),
        "{}",
        schermo(&mut w)
    );
    pulisci(d);
}

/// Alt+E: passa alla modifica (F4 e' Sostituisci).
fn modifica() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)
}
