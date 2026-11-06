//! La timeline nel terminale: la colonna accanto al testo (le righe per periodo, la barra divisa fra i livelli, il tratto che si
//! legge), il testo che si sposta, i clic sul pannello, il tasto Alt+B e il giro dei lavori del `Workspace`. Scritti da ARCH.
//!
//! Il file di prova: 144 righe, una ogni mezz'ora dal 2026-11-01, il livello gira ERROR, WARN, INFO, DEBUG (la riga `i` ha il
//! livello `i % 4`, e il numero in fondo ha da una a tre cifre: le righe sono lunghe 34-37 byte, e i tre giorni pesano un po'
//! diversamente; gli attesi dei clic li calcola `clic_atteso` sul testo). Con il passo di un giorno sono tre periodi, ciascuno
//! con 12 righe per livello, e tutti con la barra piena (il piu' leggero e' il 97% del piu' pesante: 12 celle).
//! Uno schermo di 100 x 24: la riga 0 e' il titolo, la 1 l'intestazione del pannello (e la prima riga di testo), il corpo ha 20
//! righe (le 2..=21), poi lo stato e i tasti. Tre periodi in 20 righe: il centro della riga `i` del corpo e' `(i + 0,5) / 20`,
//! quindi il primo periodo occupa le righe 0..=6, il secondo 7..=12, il terzo 13..=19.

use mrp_core::timeline::Step;
use mrp_tui::app::timeline::{TimelinePref, TimelineStatus};
use mrp_tui::app::{App, AppConfig};
use mrp_tui::theme::{Theme, ThemeId};
use mrp_tui::ui::{draw, render_lines};
use mrp_tui::workspace::Workspace;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::style::Color;
use ratatui::Terminal;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn scrivi(testo: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tlpan_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, testo).unwrap();
    p
}

fn dati() -> String {
    let mut dati = String::new();
    for i in 0..144u32 {
        let livello = ["ERROR", "WARN", "INFO", "DEBUG"][(i % 4) as usize];
        dati.push_str(&format!(
            "2026-11-{:02} {:02}:{:02}:00 [{livello}] riga {i}\n",
            1 + i / 48,
            (i % 48) / 2,
            (i % 2) * 30
        ));
    }
    dati
}

/// La scheda con la cornice del terminale, 100 x 24, la timeline spenta.
fn spenta_su(testo: &str) -> (App, PathBuf) {
    let p = scrivi(testo);
    let cfg = AppConfig {
        chrome: true,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 23);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let _ = render_lines(&mut a, 100, 23);
    (a, p)
}

/// Gira finche' lo stato e' quello atteso (o passano 20 secondi).
fn finche(a: &mut App, mut basta: impl FnMut(&App) -> bool) {
    let fine = Instant::now() + Duration::from_secs(20);
    loop {
        a.timeline_tick();
        if basta(a) || Instant::now() >= fine {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Le tacche e i livelli pronti.
fn pronta(a: &mut App) {
    finche(a, |a| {
        a.timeline_status() == TimelineStatus::Ready && a.timeline_levels_ready()
    });
}

/// La scheda del file di prova con la timeline accesa, a giorni, pronta.
fn con_pannello() -> (App, PathBuf) {
    let (mut a, p) = spenta_su(&dati());
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    pronta(&mut a);
    assert!(a.timeline_levels_ready());
    (a, p)
}

fn alt(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT)
}

fn tasto(c: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(c, m)
}

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn giu(a: &mut App, col: u16, row: u16) {
    a.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), col, row));
    let _ = render_lines(a, 100, 23);
}

fn trascina(a: &mut App, col: u16, row: u16) {
    a.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), col, row));
    let _ = render_lines(a, 100, 23);
}

fn su(a: &mut App, col: u16, row: u16) {
    a.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), col, row));
    let _ = render_lines(a, 100, 23);
}

/// Dove porta un clic sulla riga `i` del corpo (di 20, a giorni): il centro della riga cade nel periodo `k` (un terzo a testa) a
/// una `frazione` di quel periodo; il punto e' quella frazione dei byte del giorno, poi l'inizio della riga di testo che lo
/// contiene. Si calcola sul testo del file (i giorni sono 48 righe di fila), non sull'indice; ritorna l'offset e il numero (da 0)
/// della riga. Dice se il punto e' a meno di 3 byte da un confine fra righe: li' l'arrotondamento deciderebbe il collaudo.
fn clic_atteso(i: usize) -> (u64, usize) {
    let y = (i as f64 + 0.5) / 20.0;
    let k = (y * 3.0) as usize;
    let frazione = y * 3.0 - k as f64;
    let testo = dati();
    let mut inizi: Vec<u64> = Vec::new();
    let mut o = 0u64;
    for riga in testo.split_inclusive('\n') {
        inizi.push(o);
        o += riga.len() as u64;
    }
    let da = inizi[k * 48];
    let a = inizi.get((k + 1) * 48).copied().unwrap_or(o);
    let grezzo = da + (frazione * (a - da) as f64) as u64;
    let riga = inizi.partition_point(|&s| s <= grezzo) - 1;
    let prossimo = inizi.get(riga + 1).copied().unwrap_or(o);
    assert!(
        grezzo - inizi[riga] >= 3 && prossimo - grezzo >= 3,
        "riga {i} del corpo: il punto {grezzo} e' troppo vicino a un confine ({}..{prossimo})",
        inizi[riga]
    );
    (inizi[riga], riga)
}

/// I primi `n` caratteri di una riga disegnata.
fn primi(riga: &str, n: usize) -> String {
    riga.chars().take(n).collect()
}

/// Il carattere in colonna `x` di una riga disegnata.
fn colonna(riga: &str, x: usize) -> char {
    riga.chars().nth(x).unwrap_or(' ')
}

#[test]
fn the_panel_has_a_row_per_period_with_a_bar_split_by_level() {
    let (mut a, p) = con_pannello();
    let righe = render_lines(&mut a, 100, 23);
    // l'intestazione: il nome col passo (fisso, a giorni; i livelli sono contati) nelle 16 colonne del titolo e la legenda dei
    // livelli (forma e lettera, 8 colonne)
    assert_eq!(
        primi(&righe[0], 24),
        format!(" Timeline 1 day{}█E▓W▒I░D", " ")
    );
    // tre periodi uguali: 12 celle, tre per livello (ERROR, WARN, INFO, DEBUG)
    let barra = "███▓▓▓▒▒▒░░░";
    for i in 0..20usize {
        let etichetta = match i {
            0 => "2026-11-01",
            7 => "2026-11-02",
            13 => "2026-11-03",
            _ => "",
        };
        let atteso = format!("{etichetta:<11} {barra}");
        assert_eq!(primi(&righe[1 + i], 24), atteso, "riga {i} del corpo");
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_rule_marks_where_the_file_is_read() {
    let (mut a, p) = con_pannello();
    let righe = render_lines(&mut a, 100, 23);
    // all'inizio: si legge il primo periodo (le prime 21 righe di 144), il filetto e' in risalto sulla prima riga e non in fondo
    assert_eq!(colonna(&righe[1], 24), '┃');
    for i in 5..20usize {
        assert_eq!(colonna(&righe[1 + i], 24), '│', "riga {i}");
    }
    // l'intestazione ha sempre il filetto normale
    assert_eq!(colonna(&righe[0], 24), '│');
    // in fondo al file: il contrario
    a.handle_key(tasto(KeyCode::End, KeyModifiers::CONTROL));
    let righe = render_lines(&mut a, 100, 23);
    assert_eq!(colonna(&righe[1 + 19], 24), '┃');
    for i in 0..10usize {
        assert_eq!(colonna(&righe[1 + i], 24), '│', "riga {i}");
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_text_moves_aside_and_nothing_in_it_changes() {
    let (mut a, p) = con_pannello();
    let con = render_lines(&mut a, 100, 23);
    let (x_con, y_con) = a.cursor_xy().expect("cursore");
    // la stessa scheda senza il pannello, larga quanto resta al testo
    a.set_timeline(false);
    let senza = render_lines(&mut a, 75, 23);
    let (x_senza, y_senza) = a.cursor_xy().expect("cursore");
    assert_eq!((x_con, y_con), (x_senza + 25, y_senza));
    for k in 0..=20usize {
        let testo: String = con[k].chars().skip(25).collect();
        assert_eq!(testo, senza[k], "riga {k}");
    }
    // il testo comincia davvero dopo il pannello: il gutter di 5 colonne ("%4d" e il filetto) e la prima riga
    assert_eq!(
        con[0].chars().skip(25).collect::<String>(),
        "   1│2026-11-01 00:00:00 [ERROR] riga 0"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn on_the_right_the_panel_follows_the_text() {
    let (mut a, p) = con_pannello();
    a.set_timeline_right(true);
    let con = render_lines(&mut a, 100, 23);
    let xy_con = a.cursor_xy().expect("cursore");
    a.set_timeline(false);
    let senza = render_lines(&mut a, 75, 23);
    // il testo sta dov'era senza pannello, e il cursore pure
    assert_eq!(xy_con, a.cursor_xy().expect("cursore"));
    for k in 0..=20usize {
        assert_eq!(primi(&con[k], 75).trim_end(), senza[k], "riga {k}");
    }
    // il filetto e' la prima colonna del pannello (dal lato del testo), poi la data e la barra
    assert_eq!(colonna(&con[1], 75), '┃');
    let barra = "███▓▓▓▒▒▒░░░";
    assert_eq!(
        con[1].chars().skip(76).collect::<String>(),
        format!("2026-11-01  {barra}")
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_screen_too_narrow_for_the_text_has_no_panel() {
    let (mut a, p) = con_pannello();
    // 65 colonne = 25 del pannello + 40 del testo: c'e'
    a.resize(65, 23);
    let c65 = render_lines(&mut a, 65, 23);
    assert!(c65[0].starts_with(" Timeline"), "{:?}", c65[0]);
    // 64: non c'e', e il testo e' quello di una scheda senza timeline
    a.resize(64, 23);
    let c64 = render_lines(&mut a, 64, 23);
    assert!(a.timeline_on(), "resta accesa, solo non si vede");
    a.set_timeline(false);
    let spenta = render_lines(&mut a, 64, 23);
    assert_eq!(c64, spenta);
    let _ = std::fs::remove_file(p);
}

#[test]
fn before_the_levels_are_counted_the_bars_are_dim_and_in_one_shape() {
    let (mut a, p) = spenta_su(&dati());
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    // appena ci sono le tacche, prima che il giro dei livelli si raccolga
    finche(&mut a, |a| a.timeline_status() == TimelineStatus::Ready);
    assert!(!a.timeline_levels_ready());
    let righe = render_lines(&mut a, 100, 23);
    assert_eq!(primi(&righe[1], 24), "2026-11-01  ▒▒▒▒▒▒▒▒▒▒▒▒");
    pronta(&mut a);
    let righe = render_lines(&mut a, 100, 23);
    assert_eq!(primi(&righe[1], 24), "2026-11-01  ███▓▓▓▒▒▒░░░");
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_file_with_no_times_says_so_in_the_panel() {
    let (mut a, p) = spenta_su("niente\nda\nvedere\n");
    a.set_timeline(true);
    finche(&mut a, |a| a.timeline_status() == TimelineStatus::NoTimes);
    assert_eq!(a.timeline_status(), TimelineStatus::NoTimes);
    let righe = render_lines(&mut a, 100, 23);
    // "No times to show: pick a time format first." spezzato a 23 colonne, con uno spazio davanti
    assert_eq!(primi(&righe[1], 24), " No times to show: pick ");
    assert_eq!(
        primi(&righe[2], 24),
        format!("{:<24}", " a time format first.")
    );
    assert!(righe[3].chars().take(24).all(|c| c == ' '));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_document_with_unsaved_changes_says_it_is_not_available() {
    let (mut a, p) = spenta_su(&dati());
    a.handle_key(tasto(KeyCode::F(4), KeyModifiers::NONE));
    a.handle_key(tasto(KeyCode::Char('x'), KeyModifiers::NONE));
    a.set_timeline(true);
    finche(&mut a, |a| {
        a.timeline_status() == TimelineStatus::Unavailable
    });
    let righe = render_lines(&mut a, 100, 23);
    // "Not available for a document with unsaved changes." spezzato a 23 colonne
    assert_eq!(
        primi(&righe[1], 24),
        format!("{:<24}", " Not available for a")
    );
    assert_eq!(
        primi(&righe[2], 24),
        format!("{:<24}", " document with unsaved")
    );
    assert_eq!(primi(&righe[3], 24), format!("{:<24}", " changes."));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_click_on_the_panel_moves_the_cursor_to_that_point_of_the_file() {
    let (mut a, p) = con_pannello();
    // la riga `i` del corpo e' la riga di schermo 1 + i; il punto lo dice `clic_atteso` (un terzo del corpo per periodo)
    for i in [3usize, 9, 15] {
        giu(&mut a, 5, 1 + i as u16);
        su(&mut a, 5, 1 + i as u16);
        assert_eq!(a.cursor(), clic_atteso(i).0, "riga {i} del corpo");
    }
    // e il testo si e' spostato di conseguenza: la riga del cursore si vede
    let (_, riga) = clic_atteso(15);
    let righe = render_lines(&mut a, 100, 23);
    assert!(
        righe[0..=20]
            .iter()
            .any(|r| r.ends_with(&format!("[DEBUG] riga {riga}"))),
        "{righe:#?}"
    );
    assert_eq!(a.selection(), None, "un clic sul pannello non seleziona");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_panel_on_the_right_takes_its_own_clicks_and_the_text_keeps_its_own() {
    let (mut a, p) = con_pannello();
    a.set_timeline_right(true);
    let _ = render_lines(&mut a, 100, 23);
    // un clic nel testo (colonna 5 + 4: il gutter e' di 5 colonne, quindi la colonna 4; la riga di schermo 2 e' la terza riga del file,
    // che comincia a 35 + 34 = 69) e' un clic nel testo
    giu(&mut a, 9, 2);
    su(&mut a, 9, 2);
    assert_eq!(a.cursor(), 69 + 4);
    // il pannello sta alle colonne 75..=99
    giu(&mut a, 90, 1 + 9);
    su(&mut a, 90, 1 + 9);
    assert_eq!(a.cursor(), clic_atteso(9).0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn dragging_on_the_panel_keeps_following_even_outside_it_and_never_selects() {
    let (mut a, p) = con_pannello();
    giu(&mut a, 5, 1 + 3);
    assert_eq!(a.cursor(), clic_atteso(3).0);
    trascina(&mut a, 5, 1 + 9);
    assert_eq!(a.cursor(), clic_atteso(9).0);
    // il puntatore esce dal pannello, nel testo: il trascinamento e' ancora del pannello
    trascina(&mut a, 60, 1 + 15);
    assert_eq!(a.cursor(), clic_atteso(15).0);
    // sotto il corpo vale l'ultima riga (la 19), sopra la prima (la 0)
    trascina(&mut a, 60, 22);
    assert_eq!(a.cursor(), clic_atteso(19).0);
    trascina(&mut a, 60, 0);
    assert_eq!(a.cursor(), clic_atteso(0).0);
    su(&mut a, 60, 0);
    assert_eq!(a.selection(), None);
    // lasciato il tasto, un trascinamento che passa di li' non e' piu' del pannello (e' un trascinamento nel testo)
    trascina(&mut a, 60, 1 + 3);
    assert_ne!(a.cursor(), clic_atteso(3).0);
    // finito: un clic nel testo e' di nuovo un clic nel testo
    giu(&mut a, 25 + 5 + 4, 2);
    su(&mut a, 25 + 5 + 4, 2);
    assert_eq!(a.cursor(), 69 + 4);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_header_and_a_dialog_take_clicks_without_moving_the_cursor() {
    let (mut a, p) = con_pannello();
    a.handle_key(tasto(KeyCode::End, KeyModifiers::CONTROL));
    let fine = a.cursor();
    assert!(fine > 0);
    let _ = render_lines(&mut a, 100, 23);
    // l'intestazione (riga 0): niente
    giu(&mut a, 5, 0);
    su(&mut a, 5, 0);
    assert_eq!(a.cursor(), fine);
    // con un dialogo aperto un clic sul pannello non fa niente
    a.handle_key(tasto(KeyCode::Char('g'), KeyModifiers::CONTROL));
    giu(&mut a, 5, 1 + 3);
    su(&mut a, 5, 1 + 3);
    assert_eq!(a.cursor(), fine);
    let _ = std::fs::remove_file(p);
}

#[test]
fn alt_b_turns_the_timeline_on_and_off_and_remembers_it() {
    let (mut a, p) = spenta_su(&dati());
    assert!(!a.timeline_on());
    a.handle_key(alt('b'));
    assert!(a.timeline_on());
    assert!(a.timeline_pref().on);
    let righe = render_lines(&mut a, 100, 23);
    assert!(righe[0].starts_with(" Timeline"), "{:?}", righe[0]);
    // anche maiuscola (Alt+Maiusc+B)
    a.handle_key(alt('B'));
    assert!(!a.timeline_on());
    assert!(!a.timeline_pref().on);
    let righe = render_lines(&mut a, 100, 23);
    assert!(!righe[0].starts_with(" Timeline"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_workspace_tick_builds_the_timeline_of_the_terminal() {
    let p = scrivi(&dati());
    let cfg = AppConfig {
        chrome: true,
        ..AppConfig::default()
    };
    let mut ws = Workspace::new(App::open(&p, cfg.clone()).unwrap(), cfg);
    ws.resize(100, 23);
    assert!(ws.wait_idle(Duration::from_secs(20)));
    ws.handle_key(alt('b'));
    assert!(ws.active().timeline_on());
    // senza nessuno che chiami `timeline_tick` a mano: lo fa il giro del terminale
    let fine = Instant::now() + Duration::from_secs(20);
    while ws.active().timeline_status() != TimelineStatus::Ready && Instant::now() < fine {
        ws.tick();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(ws.active().timeline_status(), TimelineStatus::Ready);
    // l'intera passata (livelli compresi) arriva col solo `tick`
    while !ws.active().timeline_levels_ready() && Instant::now() < fine {
        ws.tick();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(ws.active().timeline_levels_ready());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_colors_of_the_bar_are_the_ones_of_the_theme_for_each_level() {
    let (mut a, p) = con_pannello();
    a.adopt_look(ThemeId::Night, "en");
    let t = Theme::for_id(ThemeId::Night);
    let mut terminale = Terminal::new(TestBackend::new(100, 23)).unwrap();
    terminale.draw(|f| draw(f, &mut a)).unwrap();
    let buf = terminale.backend().buffer();
    // la barra comincia alla colonna 12 (11 della data e uno spazio): tre celle per livello
    let colore = |x: u16| buf[(x, 1)].fg;
    let atteso =
        [t.log.error, t.log.warn, t.log.info, t.log.debug].map(|s| s.fg.unwrap_or(Color::Reset));
    for (k, &c) in atteso.iter().enumerate() {
        for cella in 0..3u16 {
            assert_eq!(colore(12 + 3 * k as u16 + cella), c, "livello {k}");
        }
    }
    // la legenda nell'intestazione (riga di schermo 0, da colonna 16): forma e lettera di ogni livello, nel suo colore
    for (k, &c) in atteso.iter().enumerate() {
        let x = 16 + 2 * k as u16;
        assert_eq!(buf[(x, 0)].symbol(), ["█", "▓", "▒", "░"][k]);
        assert_eq!(buf[(x + 1, 0)].symbol(), ["E", "W", "I", "D"][k]);
        assert_eq!(buf[(x, 0)].fg, c, "forma del livello {k}");
        assert_eq!(buf[(x + 1, 0)].fg, c, "lettera del livello {k}");
    }
    // i quattro colori sono diversi fra loro (se no il collaudo non distingue niente)
    for i in 0..4 {
        for j in i + 1..4 {
            assert_ne!(atteso[i], atteso[j], "{i} e {j}");
        }
    }
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_state_of_the_timeline_is_shared_through_the_preference() {
    // le scelte fatte sulla finestra (lato, barra accesa) valgono anche per il terminale: si leggono dalla preferenza
    let p = scrivi(&dati());
    let cfg = AppConfig {
        chrome: true,
        timeline: TimelinePref {
            on: true,
            right: true,
            ..TimelinePref::default()
        },
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 23);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert!(a.timeline_on());
    pronta(&mut a);
    let righe = render_lines(&mut a, 100, 23);
    // a destra: il testo da 0, il pannello da 75
    assert!(righe[0].starts_with("   1│2026-11-01"), "{:?}", righe[0]);
    assert!(righe[0]
        .chars()
        .skip(75)
        .collect::<String>()
        .starts_with("│ ~1 hour"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_selection_and_the_log_colors_follow_the_text_aside() {
    let (mut a, p) = spenta_su(&dati());
    a.adopt_look(ThemeId::Dark, "en");
    let t = Theme::for_id(ThemeId::Dark);
    a.set_log_colors(true, false);
    a.set_timeline(true);
    // tre caratteri selezionati (Maiusc+Destra) sulla prima riga: le colonne 0, 1 e 2 del testo
    for _ in 0..3 {
        a.handle_key(tasto(KeyCode::Right, KeyModifiers::SHIFT));
    }
    assert_eq!(a.selection(), Some((0, 3)));
    let mut terminale = Terminal::new(TestBackend::new(100, 23)).unwrap();
    terminale.draw(|f| draw(f, &mut a)).unwrap();
    let buf = terminale.backend().buffer();
    let stile = |x: u16, y: u16| (buf[(x, y)].fg, buf[(x, y)].bg);
    // il testo comincia alla colonna 25 e il gutter e' di 5: la prima cella di testo e' la 30
    assert_ne!(colori(&t, t.selection), colori(&t, t.fg));
    for x in [30u16, 31, 32] {
        assert_eq!(stile(x, 0), colori(&t, t.selection), "colonna {x}");
    }
    assert_ne!(stile(33, 0), colori(&t, t.selection));
    assert_ne!(stile(29, 0), colori(&t, t.selection));
    // i colori del log, sulla terza riga di testo (INFO: il resto della riga ha il colore normale): l'orario alla colonna di testo 5
    // e la parola INFO alla 22 ("2026-11-01 01:00:00 [INFO ]": il '[' e' la ventunesima cella)
    assert_ne!(colori(&t, t.log.time), colori(&t, t.fg));
    assert_ne!(colori(&t, t.log.info), colori(&t, t.fg));
    assert_eq!(stile(30 + 5, 2), colori(&t, t.log.time));
    assert_eq!(stile(30 + 22, 2), colori(&t, t.log.info));
    // e prima del pannello non c'e' niente di tutto questo
    assert_ne!(stile(5, 2), colori(&t, t.log.time));
    let _ = std::fs::remove_file(p);
}

#[test]
fn without_the_terminal_frame_there_is_no_panel_and_the_text_stays_put() {
    // la finestra grafica (chrome = false) ha la sua barra: per il motore il testo non si sposta e non c'e' niente da disegnare
    let p = scrivi(&dati());
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 23);
    assert!(a.wait_idle(Duration::from_secs(20)));
    let prima = render_lines(&mut a, 100, 23);
    let xy_prima = a.cursor_xy();
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    pronta(&mut a);
    assert_eq!(prima, render_lines(&mut a, 100, 23));
    assert_eq!(xy_prima, a.cursor_xy());
    let _ = std::fs::remove_file(p);
}

#[test]
fn waiting_for_the_background_work_waits_for_the_timeline_of_the_terminal() {
    // `--dump` e i collaudi aspettano con `wait_idle`: le tacche e i livelli sono lavori come l'indice
    let (mut a, p) = spenta_su(&dati());
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    assert!(a.wait_idle(Duration::from_secs(20)));
    assert_eq!(a.timeline_status(), TimelineStatus::Ready);
    assert!(a.timeline_levels_ready());
    // un file senza orari: finisce subito con "niente da mostrare", non aspetta il tempo massimo
    let (mut b, q) = spenta_su("niente\nda\nvedere\n");
    b.set_timeline(true);
    let inizio = Instant::now();
    assert!(b.wait_idle(Duration::from_secs(20)));
    assert!(inizio.elapsed() < Duration::from_secs(10));
    assert_eq!(b.timeline_status(), TimelineStatus::NoTimes);
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(q);
}

#[test]
fn a_dated_log_without_levels_has_plain_bars() {
    // due giorni, tre righe uguali ciascuno, nessuna con un livello: due periodi pieni, in una forma sola
    let mut testo = String::new();
    for giorno in 1..=2 {
        for ora in 10..13 {
            testo.push_str(&format!("2026-11-0{giorno} {ora}:00:00 niente da dire\n"));
        }
    }
    let (mut a, p) = spenta_su(&testo);
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    pronta(&mut a);
    let righe = render_lines(&mut a, 100, 23);
    // due periodi in 20 righe: le prime 10 e le altre 10
    assert_eq!(primi(&righe[1], 24), "2026-11-01  ▒▒▒▒▒▒▒▒▒▒▒▒");
    assert_eq!(primi(&righe[2], 24), format!("{:<11} ▒▒▒▒▒▒▒▒▒▒▒▒", ""));
    assert_eq!(primi(&righe[11], 24), "2026-11-02  ▒▒▒▒▒▒▒▒▒▒▒▒");
    let _ = std::fs::remove_file(p);
}

/// Scrive `testo` nel dialogo aperto, un tasto alla volta.
fn digita(a: &mut App, testo: &str) {
    for c in testo.chars() {
        a.handle_key(tasto(KeyCode::Char(c), KeyModifiers::NONE));
    }
}

#[test]
fn the_found_text_and_the_column_block_follow_the_text_aside() {
    let (mut a, p) = spenta_su(&dati());
    a.adopt_look(ThemeId::Dark, "en");
    let t = Theme::for_id(ThemeId::Dark);
    a.set_timeline(true);
    // il trovato: "riga 5" e' alla fine della riga 5 ("2026-11-01 02:30:00 [WARN] riga 5"), 27 caratteri dopo l'inizio; il testo
    // comincia alla colonna 25 e il gutter e' di 5: le celle 57..=62, sulla riga di schermo 5 (la riga 0 del file e' sulla 0)
    a.handle_key(tasto(KeyCode::Char('f'), KeyModifiers::CONTROL));
    digita(&mut a, "riga 5");
    a.handle_key(tasto(KeyCode::Enter, KeyModifiers::NONE));
    assert!(a.wait_idle(Duration::from_secs(20)));
    let mut terminale = Terminal::new(TestBackend::new(100, 23)).unwrap();
    terminale.draw(|f| draw(f, &mut a)).unwrap();
    let buf = terminale.backend().buffer();
    let trovato = t
        .found
        .bg
        .expect("il tema Dark colora lo sfondo del trovato");
    let colorate: Vec<u16> = (0..100u16).filter(|&x| buf[(x, 5)].bg == trovato).collect();
    let riga6: String = (0..100u16)
        .map(|x| buf[(x, 5)].symbol().to_string())
        .collect();
    assert_eq!(colorate, (57..=62u16).collect::<Vec<_>>(), "{riga6:?}");
    assert_ne!(
        buf[(33, 5)].bg,
        trovato,
        "la stessa cella senza lo spostamento di 25"
    );
    // il blocco: F8, due colonne a destra e una riga in giu': le righe 0 e 1, le colonne 0 e 1 del testo
    a.handle_key(tasto(KeyCode::Esc, KeyModifiers::NONE));
    a.handle_key(tasto(KeyCode::Home, KeyModifiers::CONTROL));
    a.handle_key(tasto(KeyCode::F(8), KeyModifiers::NONE));
    a.handle_key(tasto(KeyCode::Right, KeyModifiers::SHIFT));
    a.handle_key(tasto(KeyCode::Right, KeyModifiers::SHIFT));
    a.handle_key(tasto(KeyCode::Down, KeyModifiers::SHIFT));
    assert!(a.column_block().is_some());
    terminale.draw(|f| draw(f, &mut a)).unwrap();
    let buf = terminale.backend().buffer();
    let selezione = t
        .selection
        .bg
        .expect("il tema Dark colora lo sfondo della selezione");
    for (x, y) in [(30u16, 0u16), (31, 0), (30, 1), (31, 1)] {
        assert_eq!(buf[(x, y)].bg, selezione, "cella {x},{y}");
    }
    assert_ne!(buf[(32, 0)].bg, selezione);
    assert_ne!(
        buf[(5, 0)].bg,
        selezione,
        "la stessa cella senza lo spostamento di 25"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_long_line_scrolls_inside_the_room_the_panel_leaves() {
    // una riga di 90 caratteri: con il pannello a sinistra restano 100 - 25 - 3 (il gutter) = 72 celle di testo, quindi a fine
    // riga (colonna 90) si scorre di 90 + 1 - 72 = 19 e il cursore sta nell'ultima colonna dello schermo (25 + 3 + 71 = 99)
    let (mut a, p) = spenta_su(&format!("{}\n", "x".repeat(90)));
    a.set_timeline(true);
    a.handle_key(tasto(KeyCode::End, KeyModifiers::NONE));
    let _ = render_lines(&mut a, 100, 23);
    assert_eq!(a.cursor_xy().expect("cursore").0, 99);
    // a destra il testo comincia a 0: l'ultima colonna del testo e' la 74, prima del pannello (da 75)
    a.set_timeline_right(true);
    a.resize(100, 23);
    let _ = render_lines(&mut a, 100, 23);
    assert_eq!(a.cursor_xy().expect("cursore").0, 74);
    let _ = std::fs::remove_file(p);
}

/// Colore di primo piano e di sfondo che una cella ha quando e' disegnata con lo stile `s` sopra lo sfondo del tema.
fn colori(t: &Theme, s: ratatui::style::Style) -> (Color, Color) {
    let e = t.screen.patch(s);
    (e.fg.unwrap_or(Color::Reset), e.bg.unwrap_or(Color::Reset))
}

#[test]
fn the_label_the_rule_and_the_dim_bars_have_their_own_styles() {
    let (mut a, p) = spenta_su(&dati());
    // un tema in cui titolo, riga di stato e numeri di riga hanno colori diversi (nel Night sono uguali)
    a.adopt_look(ThemeId::Dark, "en");
    let t = Theme::for_id(ThemeId::Dark);
    assert_ne!(colori(&t, t.title), colori(&t, t.gutter));
    assert_ne!(colori(&t, t.status), colori(&t, t.gutter));
    assert_ne!(colori(&t, t.status), colori(&t, t.title));
    a.set_timeline(true);
    a.set_timeline_step(Step::Day);
    // le tacche ci sono, i livelli non ancora: le barre sono fioche
    finche(&mut a, |a| a.timeline_status() == TimelineStatus::Ready);
    assert!(!a.timeline_levels_ready());
    let mut terminale = Terminal::new(TestBackend::new(100, 23)).unwrap();
    terminale.draw(|f| draw(f, &mut a)).unwrap();
    let buf = terminale.backend().buffer();
    let stile = |x: u16, y: u16| (buf[(x, y)].fg, buf[(x, y)].bg);
    // la barra fioca (riga 0 del corpo = riga di schermo 1, colonna 12)
    assert_eq!(buf[(12, 1)].symbol(), "▒");
    assert_eq!(stile(12, 1), colori(&t, t.gutter));
    // la data e il filetto del tratto che si legge (riga 0 del corpo): in risalto; dove non si legge (riga 14 di schermo): normali
    assert_eq!(buf[(24, 1)].symbol(), "┃");
    assert_eq!(stile(0, 1), colori(&t, t.title));
    assert_eq!(stile(24, 1), colori(&t, t.title));
    assert_eq!(buf[(24, 14)].symbol(), "│");
    assert_eq!(stile(0, 14), colori(&t, t.gutter));
    assert_eq!(stile(24, 14), colori(&t, t.gutter));
    // l'intestazione ha lo stile della riga di stato
    assert_eq!(stile(1, 0), colori(&t, t.status));
    let _ = std::fs::remove_file(p);
}
