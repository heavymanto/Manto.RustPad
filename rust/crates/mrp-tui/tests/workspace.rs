//! Le schede del TUI (E34): piu' file aperti insieme, la barra in cima, i tasti per cambiare scheda, aprire e chiudere,
//! la chiusura generale con le conferme, il mouse sulla barra. Scritti da ARCH: gli attesi sono calcolati a mano sulle
//! etichette (" nome " per scheda, " nome* " se modificata) e sulle regole dei tasti.

use mrp_tui::app::{App, AppConfig, DialogKind};
use mrp_tui::theme::ThemeId;
use mrp_tui::ui::render_lines;
use mrp_tui::workspace::Workspace;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::Terminal;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Una cartella nuova per ogni prova (i nomi dei file sono quelli che si vedono nella barra).
fn cartella() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("mrp_ws_test_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn scrivi(dir: &std::path::Path, nome: &str, dati: &[u8]) -> PathBuf {
    let p = dir.join(nome);
    std::fs::write(&p, dati).unwrap();
    p
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn mod_k(c: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(c, m)
}

fn ctrl(c: char) -> KeyEvent {
    mod_k(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn alt(c: KeyCode) -> KeyEvent {
    mod_k(c, KeyModifiers::ALT)
}

fn ws_con(dir: &std::path::Path, nomi: &[&str]) -> Workspace {
    let cfg = AppConfig::default();
    let primo = App::open(&dir.join(nomi[0]), cfg.clone()).unwrap();
    let mut ws = Workspace::new(primo, cfg);
    for n in &nomi[1..] {
        ws.open(&dir.join(n)).unwrap();
    }
    ws.select(0);
    ws.resize(40, 8);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    ws
}

fn due() -> (PathBuf, Workspace) {
    let d = cartella();
    scrivi(&d, "a.txt", b"uno\ndue\n");
    scrivi(&d, "b.txt", b"tre\nquattro\ncinque\n");
    let ws = ws_con(&d, &["a.txt", "b.txt"]);
    (d, ws)
}

fn nome_attivo(ws: &Workspace) -> String {
    ws.active().file_name().to_string()
}

fn pulisci(d: PathBuf) {
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_single_tab_draws_exactly_like_the_app_with_no_bar() {
    let d = cartella();
    scrivi(&d, "a.txt", b"uno\ndue\n");
    let mut ws = ws_con(&d, &["a.txt"]);
    let mut app = App::open(&d.join("a.txt"), AppConfig::default()).unwrap();
    app.resize(40, 8);
    assert!(app.wait_idle(Duration::from_secs(20)));
    assert_eq!(ws.render_lines(40, 8), render_lines(&mut app, 40, 8));
    pulisci(d);
}

#[test]
fn two_tabs_draw_the_bar_on_top_and_the_active_document_below() {
    let (d, mut ws) = due();
    let righe = ws.render_lines(40, 8);
    assert_eq!(righe.len(), 8);
    assert_eq!(righe[0], " a.txt  b.txt");
    // la riga 1 e' il titolo della scheda attiva, poi il suo testo con il gutter
    assert!(righe[1].starts_with(" a.txt"), "{:?}", righe[1]);
    assert_eq!(righe[2], "1│uno");
    assert_eq!(righe[3], "2│due");
    // la scheda attiva sta 1 riga piu' in basso, ma ha tutte le sue righe: il piede e' in fondo
    assert!(righe[7].starts_with(" F1 Help"), "{:?}", righe[7]);
    pulisci(d);
}

#[test]
fn tab_keys_move_between_tabs_and_wrap() {
    let (d, mut ws) = due();
    assert_eq!(nome_attivo(&ws), "a.txt");
    ws.handle_key(alt(KeyCode::Right));
    assert_eq!(nome_attivo(&ws), "b.txt");
    ws.handle_key(alt(KeyCode::Right));
    assert_eq!(nome_attivo(&ws), "a.txt", "dall'ultima si torna alla prima");
    ws.handle_key(alt(KeyCode::Left));
    assert_eq!(nome_attivo(&ws), "b.txt", "dalla prima si va all'ultima");
    ws.handle_key(mod_k(KeyCode::PageUp, KeyModifiers::CONTROL));
    assert_eq!(nome_attivo(&ws), "a.txt");
    ws.handle_key(mod_k(KeyCode::PageDown, KeyModifiers::CONTROL));
    assert_eq!(nome_attivo(&ws), "b.txt");
    ws.handle_key(alt(KeyCode::Char('1')));
    assert_eq!(nome_attivo(&ws), "a.txt");
    ws.handle_key(alt(KeyCode::Char('2')));
    assert_eq!(nome_attivo(&ws), "b.txt");
    // una scheda che non c'e': niente
    ws.handle_key(alt(KeyCode::Char('9')));
    assert_eq!(nome_attivo(&ws), "b.txt");
    // PgSu/PgGiu senza Ctrl restano dell'app (scorrono il testo)
    ws.handle_key(k(KeyCode::PageDown));
    assert_eq!(nome_attivo(&ws), "b.txt");
    pulisci(d);
}

#[test]
fn tab_keys_are_ignored_while_a_dialog_is_open() {
    let (d, mut ws) = due();
    ws.handle_key(k(KeyCode::F(1)));
    assert_eq!(ws.active().dialog_kind(), Some(DialogKind::Help));
    ws.handle_key(alt(KeyCode::Right));
    assert_eq!(nome_attivo(&ws), "a.txt");
    ws.handle_key(k(KeyCode::Esc));
    ws.handle_key(alt(KeyCode::Right));
    assert_eq!(nome_attivo(&ws), "b.txt");
    pulisci(d);
}

#[test]
fn each_tab_keeps_its_own_cursor_and_edits() {
    let (d, mut ws) = due();
    ws.handle_key(k(KeyCode::Down));
    assert_eq!(ws.active().cursor(), 4);
    ws.handle_key(alt(KeyCode::Right));
    assert_eq!(ws.active().cursor(), 0);
    ws.handle_key(k(KeyCode::F(4)));
    ws.handle_key(k(KeyCode::Char('X')));
    assert_eq!(ws.active().document_text(), b"Xtre\nquattro\ncinque\n");
    // la barra segna con * la scheda modificata
    assert_eq!(ws.render_lines(40, 8)[0], " a.txt  b.txt*");
    ws.handle_key(alt(KeyCode::Left));
    assert_eq!(
        ws.active().cursor(),
        4,
        "la prima scheda e' come l'avevo lasciata"
    );
    assert_eq!(ws.active().document_text(), b"uno\ndue\n");
    pulisci(d);
}

#[test]
fn ctrl_o_opens_a_file_in_a_new_tab_and_a_known_file_is_just_brought_forward() {
    let (d, mut ws) = due();
    let c = scrivi(&d, "c.txt", b"ciao\n");
    ws.handle_key(ctrl('o'));
    assert_eq!(ws.active().dialog_kind(), Some(DialogKind::Open));
    for ch in c.to_string_lossy().chars() {
        ws.handle_key(k(KeyCode::Char(ch)));
    }
    ws.handle_key(k(KeyCode::Enter));
    assert_eq!(ws.len(), 3);
    assert_eq!(nome_attivo(&ws), "c.txt");
    assert_eq!(ws.active().dialog_kind(), None);
    assert_eq!(ws.render_lines(40, 8)[0], " a.txt  b.txt  c.txt");
    // lo stesso file ancora (anche con un percorso scritto diversamente): nessuna scheda nuova
    ws.select(0);
    ws.handle_key(ctrl('o'));
    let giro = d.join(".").join("b.txt");
    for ch in giro.to_string_lossy().chars() {
        ws.handle_key(k(KeyCode::Char(ch)));
    }
    ws.handle_key(k(KeyCode::Enter));
    assert_eq!(ws.len(), 3);
    assert_eq!(nome_attivo(&ws), "b.txt");
    pulisci(d);
}

#[test]
fn opening_a_missing_file_says_so_and_changes_nothing() {
    let (d, mut ws) = due();
    ws.handle_key(ctrl('o'));
    for ch in d.join("manca.txt").to_string_lossy().chars() {
        ws.handle_key(k(KeyCode::Char(ch)));
    }
    ws.handle_key(k(KeyCode::Enter));
    assert_eq!(ws.len(), 2);
    assert_eq!(nome_attivo(&ws), "a.txt");
    let avviso = ws.active().flash().expect("un avviso").to_string();
    assert!(avviso.contains("manca.txt"), "{avviso}");
    // un prompt vuoto o Esc non fanno niente
    ws.handle_key(ctrl('o'));
    ws.handle_key(k(KeyCode::Enter));
    ws.handle_key(ctrl('o'));
    ws.handle_key(k(KeyCode::Esc));
    assert_eq!(ws.len(), 2);
    pulisci(d);
}

#[test]
fn a_new_tab_takes_the_theme_and_language_of_the_active_one_and_a_change_reaches_all() {
    let (d, mut ws) = due();
    // F6, un passo giu' (Night -> Debian per il primo menu: parte dal tema corrente, Console) e Invio
    let prima = ws.active().theme_id();
    assert_eq!(prima, ThemeId::Console);
    ws.handle_key(k(KeyCode::F(6)));
    ws.handle_key(k(KeyCode::Down));
    ws.handle_key(k(KeyCode::Enter));
    let nuovo = ws.active().theme_id();
    assert_eq!(
        nuovo,
        ThemeId::Minimal,
        "Console e' la terza voce, la quarta e' Minimal"
    );
    for a in ws.tabs() {
        assert_eq!(a.theme_id(), nuovo);
    }
    let c = scrivi(&d, "c.txt", b"x\n");
    ws.open(&c).unwrap();
    assert_eq!(ws.active().theme_id(), nuovo);
    pulisci(d);
}

#[test]
fn ctrl_w_closes_a_clean_tab_at_once_and_the_last_one_ends_the_session() {
    let (d, mut ws) = due();
    ws.handle_key(ctrl('w'));
    assert_eq!(ws.len(), 1);
    assert_eq!(nome_attivo(&ws), "b.txt");
    assert!(!ws.should_quit());
    // con una sola scheda niente barra
    let prima = ws.render_lines(40, 8)[0].clone();
    assert!(
        prima.starts_with(" b.txt") && prima.ends_with("19 B  VIEW"),
        "{prima:?}"
    );
    ws.handle_key(ctrl('w'));
    assert_eq!(ws.len(), 1, "l'ultima scheda resta, ma si esce");
    assert!(ws.should_quit());
    pulisci(d);
}

#[test]
fn closing_a_modified_tab_asks_first() {
    let (d, mut ws) = due();
    ws.handle_key(k(KeyCode::F(4)));
    ws.handle_key(k(KeyCode::Char('X')));
    ws.handle_key(ctrl('w'));
    assert_eq!(ws.active().dialog_kind(), Some(DialogKind::Confirm));
    assert_eq!(ws.len(), 2);
    // annulla: resta
    ws.handle_key(k(KeyCode::Char('c')));
    assert_eq!(ws.len(), 2);
    assert_eq!(ws.active().dialog_kind(), None);
    assert!(ws.active().is_modified());
    // scarta: si chiude, resta l'altra scheda, e il file su disco non e' cambiato
    ws.handle_key(ctrl('w'));
    ws.handle_key(k(KeyCode::Char('d')));
    assert_eq!(ws.len(), 1);
    assert_eq!(nome_attivo(&ws), "b.txt");
    assert_eq!(std::fs::read(d.join("a.txt")).unwrap(), b"uno\ndue\n");
    pulisci(d);
}

#[test]
fn closing_a_modified_tab_that_is_not_active_brings_it_forward_to_ask() {
    let (d, mut ws) = due();
    ws.handle_key(alt(KeyCode::Right));
    ws.handle_key(k(KeyCode::F(4)));
    ws.handle_key(k(KeyCode::Char('X')));
    ws.select(0);
    ws.close(1);
    assert_eq!(nome_attivo(&ws), "b.txt");
    assert_eq!(ws.active().dialog_kind(), Some(DialogKind::Confirm));
    pulisci(d);
}

#[test]
fn quitting_asks_for_each_modified_tab_and_ends_only_when_all_are_settled() {
    let d = cartella();
    for n in ["a.txt", "b.txt", "c.txt"] {
        scrivi(&d, n, b"uno\n");
    }
    let mut ws = ws_con(&d, &["a.txt", "b.txt", "c.txt"]);
    // modifico a.txt e c.txt, non b.txt
    for i in [0, 2] {
        ws.select(i);
        ws.handle_key(k(KeyCode::F(4)));
        ws.handle_key(k(KeyCode::Char('X')));
    }
    ws.select(1);
    ws.handle_key(k(KeyCode::F(10)));
    // la prima con modifiche e' a.txt: viene avanti e chiede
    assert_eq!(nome_attivo(&ws), "a.txt");
    assert_eq!(ws.active().dialog_kind(), Some(DialogKind::Confirm));
    assert!(!ws.should_quit());
    ws.handle_key(k(KeyCode::Char('d')));
    // ora c.txt
    assert_eq!(ws.len(), 2);
    assert_eq!(nome_attivo(&ws), "c.txt");
    assert_eq!(ws.active().dialog_kind(), Some(DialogKind::Confirm));
    ws.handle_key(k(KeyCode::Char('d')));
    // restava b.txt, pulito: si esce
    assert!(ws.should_quit() || ws.len() == 1);
    assert!(ws.should_quit());
    // niente e' stato scritto
    assert_eq!(std::fs::read(d.join("a.txt")).unwrap(), b"uno\n");
    assert_eq!(std::fs::read(d.join("c.txt")).unwrap(), b"uno\n");
    pulisci(d);
}

#[test]
fn cancelling_during_quit_stops_the_quit_and_a_later_f10_starts_again() {
    let (d, mut ws) = due();
    ws.handle_key(k(KeyCode::F(4)));
    ws.handle_key(k(KeyCode::Char('X')));
    ws.handle_key(k(KeyCode::F(10)));
    assert_eq!(ws.active().dialog_kind(), Some(DialogKind::Confirm));
    ws.handle_key(k(KeyCode::Esc));
    assert!(!ws.should_quit());
    assert_eq!(ws.len(), 2);
    // se si scarta una scheda a mano dopo un annullamento, l'altra non viene chiesta da sola
    ws.handle_key(alt(KeyCode::Right));
    ws.handle_key(ctrl('w'));
    assert_eq!(ws.len(), 1);
    assert_eq!(ws.active().dialog_kind(), None);
    assert!(ws.active().is_modified());
    assert!(!ws.should_quit());
    // F10 di nuovo: chiede per a.txt, e salvando (S) si chiude e si esce
    ws.handle_key(k(KeyCode::F(10)));
    ws.handle_key(k(KeyCode::Char('s')));
    for _ in 0..400 {
        ws.tick();
        if ws.should_quit() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(ws.should_quit());
    assert_eq!(std::fs::read(d.join("a.txt")).unwrap(), b"Xuno\ndue\n");
    pulisci(d);
}

#[test]
fn ctrl_q_quits_like_f10() {
    let (d, mut ws) = due();
    ws.handle_key(ctrl('q'));
    assert!(ws.should_quit());
    pulisci(d);
}

fn clic(col: u16, riga: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row: riga,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn a_click_on_the_bar_selects_the_tab_and_a_click_on_the_text_counts_below_the_bar() {
    let (d, mut ws) = due();
    // il disegno ricorda dove stanno le schede
    let _ = ws.render_lines(40, 8);
    // " a.txt " = colonne 0..7, " b.txt " = 7..14
    ws.handle_mouse(clic(9, 0));
    assert_eq!(nome_attivo(&ws), "b.txt");
    ws.handle_mouse(clic(2, 0));
    assert_eq!(nome_attivo(&ws), "a.txt");
    // oltre l'ultima scheda: niente
    ws.handle_mouse(clic(30, 0));
    assert_eq!(nome_attivo(&ws), "a.txt");
    // un clic sul testo: la riga di schermo 3 e' la seconda riga del documento ("due")
    let _ = ws.render_lines(40, 8);
    ws.handle_mouse(clic(3, 3));
    assert_eq!(ws.active().cursor(), 5, "'u' di due");
    // un clic sul titolo della scheda (riga 1) non fa niente
    ws.handle_mouse(clic(3, 1));
    assert_eq!(ws.active().cursor(), 5);
    pulisci(d);
}

#[test]
fn many_tabs_scroll_the_bar_to_keep_the_active_one_visible() {
    let d = cartella();
    let nomi: Vec<String> = (0..10).map(|i| format!("file{i}.txt")).collect();
    for n in &nomi {
        scrivi(&d, n, b"x\n");
    }
    let riferimenti: Vec<&str> = nomi.iter().map(String::as_str).collect();
    let mut ws = ws_con(&d, &riferimenti);
    // 11 colonne per scheda, 40 colonne: partendo dalla prima se ne vedono 3 e un pezzo della quarta
    assert_eq!(
        ws.render_lines(40, 8)[0],
        " file0.txt  file1.txt  file2.txt  file3."
    );
    // all'ultima la barra parte dalla settima: 7, 8 e 9 fanno 33 colonne, con la sesta sarebbero 44
    ws.select(9);
    assert_eq!(
        ws.render_lines(40, 8)[0],
        " file7.txt  file8.txt  file9.txt"
    );
    // un clic sulla parte visibile della quarta (all'inizio) la sceglie
    ws.select(0);
    let _ = ws.render_lines(40, 8);
    ws.handle_mouse(clic(35, 0));
    assert_eq!(nome_attivo(&ws), "file3.txt");
    pulisci(d);
}

#[test]
fn the_terminal_cursor_sits_one_row_lower_with_the_bar() {
    let (d, mut ws) = due();
    ws.handle_key(k(KeyCode::F(4)));
    let mut t = Terminal::new(TestBackend::new(40, 8)).unwrap();
    t.draw(|f| ws.draw(f)).unwrap();
    let pos = t.get_cursor_position().unwrap();
    // gutter di 2 celle, prima riga di testo = riga di schermo 2 (barra 0, titolo 1)
    assert_eq!((pos.x, pos.y), (2, 2));
    pulisci(d);
}

// ---- i casi trovati dalle mutazioni ----

#[test]
fn a_click_exactly_on_the_first_column_of_a_tab_selects_that_tab() {
    let (d, mut ws) = due();
    let _ = ws.render_lines(40, 8);
    // " a.txt " = 0..7, " b.txt " = 7..14: la colonna 6 e' dell'una, la 7 dell'altra
    ws.handle_mouse(clic(7, 0));
    assert_eq!(nome_attivo(&ws), "b.txt");
    ws.handle_mouse(clic(6, 0));
    assert_eq!(nome_attivo(&ws), "a.txt");
    pulisci(d);
}

#[test]
fn the_bar_does_not_scroll_when_the_tabs_fit_exactly() {
    let d = cartella();
    let nomi: Vec<String> = (0..3).map(|i| format!("file{i}.txt")).collect();
    for n in &nomi {
        scrivi(&d, n, b"x\n");
    }
    let riferimenti: Vec<&str> = nomi.iter().map(String::as_str).collect();
    let mut ws = ws_con(&d, &riferimenti);
    ws.select(2);
    // 3 schede da 11 colonne = 33 colonne: ci stanno tutte
    assert_eq!(
        ws.render_lines(33, 8)[0],
        " file0.txt  file1.txt  file2.txt"
    );
    pulisci(d);
}

#[test]
fn a_tab_brought_forward_takes_the_size_of_the_screen() {
    let d = cartella();
    scrivi(&d, "a.txt", b"x\n");
    let testo: String = (0..100).map(|i| format!("riga {i:02}\n")).collect();
    scrivi(&d, "b.txt", testo.as_bytes());
    let mut ws = ws_con(&d, &["a.txt", "b.txt"]);
    ws.select(1);
    ws.handle_key(k(KeyCode::PageDown));
    let con_barra = ws.active().cursor();
    // lo stesso file in un'app a se' alta una riga di meno dello schermo (la barra ne prende una)
    let mut app = App::open(&d.join("b.txt"), AppConfig::default()).unwrap();
    app.resize(40, 7);
    assert!(app.wait_idle(Duration::from_secs(20)));
    app.handle_key(k(KeyCode::PageDown));
    assert_eq!(con_barra, app.cursor());
    let mut grande = App::open(&d.join("b.txt"), AppConfig::default()).unwrap();
    grande.resize(40, 24);
    assert!(grande.wait_idle(Duration::from_secs(20)));
    grande.handle_key(k(KeyCode::PageDown));
    assert_ne!(
        con_barra,
        grande.cursor(),
        "la prova distingue le due altezze"
    );
    pulisci(d);
}

fn con_lingue(dir: &std::path::Path, nomi: &[&str]) -> Workspace {
    let lang_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang");
    let (langs, _) = mrp_core::lang::registry_in(Some(&lang_dir));
    let cfg = AppConfig {
        langs,
        ..AppConfig::default()
    };
    let primo = App::open(&dir.join(nomi[0]), cfg.clone()).unwrap();
    let mut ws = Workspace::new(primo, cfg);
    for n in &nomi[1..] {
        ws.open(&dir.join(n)).unwrap();
    }
    ws.select(0);
    ws.resize(40, 8);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    ws
}

#[test]
fn the_language_chosen_in_a_tab_reaches_the_others_and_the_new_ones() {
    let d = cartella();
    scrivi(&d, "a.txt", b"x\n");
    scrivi(&d, "b.txt", b"y\n");
    scrivi(&d, "c.txt", b"z\n");
    let mut ws = con_lingue(&d, &["a.txt", "b.txt"]);
    assert_eq!(ws.active().lang_code(), "en");
    // F7: le lingue sono en, de, es, it, ...: tre passi giu' dall'inglese e' l'italiano
    ws.handle_key(k(KeyCode::F(7)));
    for _ in 0..3 {
        ws.handle_key(k(KeyCode::Down));
    }
    ws.handle_key(k(KeyCode::Enter));
    for a in ws.tabs() {
        assert_eq!(a.lang_code(), "it");
    }
    ws.open(&d.join("c.txt")).unwrap();
    assert_eq!(ws.active().lang_code(), "it");
    pulisci(d);
}

#[test]
fn the_open_prompt_ignores_spaces_around_the_path() {
    let (d, mut ws) = due();
    let c = scrivi(&d, "c.txt", b"ciao\n");
    ws.handle_key(ctrl('o'));
    for ch in format!("  {}  ", c.display()).chars() {
        ws.handle_key(k(KeyCode::Char(ch)));
    }
    ws.handle_key(k(KeyCode::Enter));
    assert_eq!(nome_attivo(&ws), "c.txt");
    pulisci(d);
}

#[test]
fn a_tab_that_finishes_saving_while_not_active_closes_and_the_active_one_stays() {
    let d = cartella();
    for n in ["a.txt", "b.txt", "c.txt"] {
        scrivi(&d, n, b"uno\n");
    }
    let mut ws = ws_con(&d, &["a.txt", "b.txt", "c.txt"]);
    ws.handle_key(k(KeyCode::F(4)));
    ws.handle_key(k(KeyCode::Char('X')));
    ws.handle_key(ctrl('w'));
    // S: salva e poi chiude; il salvataggio e' in corso e si passa alla scheda b prima che finisca
    ws.handle_key(k(KeyCode::Char('s')));
    ws.select(1);
    assert_eq!(nome_attivo(&ws), "b.txt");
    for _ in 0..400 {
        ws.tick();
        if ws.len() == 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(ws.len(), 2, "la scheda salvata si e' chiusa da sola");
    assert_eq!(nome_attivo(&ws), "b.txt", "l'attiva e' sempre b");
    assert_eq!(ws.active_index(), 0);
    assert_eq!(std::fs::read(d.join("a.txt")).unwrap(), b"Xuno\n");
    pulisci(d);
}
