//! La riga di stato porta anche il nome del file, la dimensione e il modo (prima stavano in una riga del titolo, in cima, che non c'e'
//! piu': il testo comincia alla prima riga). Scritti da ARCH.
//! Ordine a destra: lavoro in corso, nome, dimensione, codifica, fine riga, modo. Tre spazi almeno dalla sinistra; se manca il posto
//! cade, nell'ordine, il nome (prima accorciato, tenendo la fine), la dimensione con codifica e fine riga, il modo.

use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn app(nome: &str, testo: &str, w: u16, h: u16) -> (App, PathBuf) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("mrp_stato_{}_{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(nome);
    std::fs::write(&p, testo).unwrap();
    let mut a = App::open(&p, AppConfig::default()).unwrap();
    a.resize(w, h);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, p)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

/// La riga di stato (la penultima), senza gli spazi in fondo.
fn stato(a: &mut App, w: u16, h: u16) -> String {
    render_lines(a, w, h)[usize::from(h) - 2]
        .trim_end()
        .to_string()
}

fn pulisci(p: PathBuf) {
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
}

#[test]
fn there_is_no_title_row_the_text_starts_on_the_first_row() {
    let (mut a, p) = app("a.txt", "uno\ndue\n", 60, 6);
    let righe = render_lines(&mut a, 60, 6);
    assert_eq!(righe[0], "1│uno");
    assert_eq!(righe[1], "2│due");
    assert!(righe[4].starts_with(" Ln 1"), "{:?}", righe[4]);
    assert!(righe[5].starts_with(" Alt+V View"), "{:?}", righe[5]);
    pulisci(p);
}

#[test]
fn the_right_side_has_the_name_the_size_and_the_mode_in_that_order() {
    let (mut a, p) = app("note.txt", "uno\ndue\n", 80, 6);
    let s = stato(&mut a, 80, 6);
    assert!(s.starts_with(" Ln 1  Col 1  byte 0  0%"), "{s:?}");
    assert!(s.ends_with("note.txt  8 B  VIEW"), "{s:?}");
    // fra la posizione e il nome ci sono almeno tre spazi
    assert!(s.contains("0%   "), "{s:?}");
    // il modo cambia con F4 e il segno di modificato segue il nome
    a.handle_key(k(KeyCode::F(4)));
    a.handle_key(k(KeyCode::Char('x')));
    let s = stato(&mut a, 80, 6);
    assert!(s.ends_with("note.txt *  9 B  EDIT"), "{s:?}");
    pulisci(p);
}

#[test]
fn the_column_mode_is_said_after_the_mode() {
    let (mut a, p) = app("note.txt", "uno\ndue\n", 80, 6);
    a.handle_key(k(KeyCode::F(8)));
    let s = stato(&mut a, 80, 6);
    assert!(s.ends_with("8 B  VIEW COL"), "{s:?}");
    pulisci(p);
}

#[test]
fn a_long_name_is_shortened_keeping_its_end() {
    let nome = "un_nome_di_file_molto_lungo_che_non_ci_sta_nella_riga.txt";
    let (mut a, p) = app(nome, "uno\n", 70, 6);
    let s = stato(&mut a, 70, 6);
    let coda: String = s.split("  4 B  VIEW").next().unwrap().to_string();
    // il nome e' accorciato davanti con "…" e finisce come il nome vero
    let accorciato = coda.rsplit("   ").next().unwrap();
    assert!(accorciato.starts_with('…'), "{s:?}");
    assert!(nome.ends_with(&accorciato['…'.len_utf8()..]), "{s:?}");
    assert!(accorciato.chars().count() >= 8, "{s:?}");
    assert!(s.chars().count() <= 70, "{s:?}");
    assert!(s.ends_with("4 B  VIEW"), "{s:?}");
    pulisci(p);
}

#[test]
fn what_does_not_fit_goes_away_in_order_name_then_size_then_mode() {
    let (mut a, p) = app("documento.txt", "uno\n", 120, 6);
    let mut visto_modo = false;
    let mut visto_misura = false;
    // dal largo allo stretto: ogni cosa, una volta sparita, non torna piu'; il nome non c'e' senza la misura e questa senza il modo
    let mut prima = (true, true, true);
    for w in (28..=120u16).rev() {
        a.resize(w, 6);
        let s = stato(&mut a, w, 6);
        assert!(s.chars().count() <= usize::from(w), "w={w}: {s:?}");
        assert!(s.starts_with(" Ln 1  Col 1  byte 0  0%"), "w={w}: {s:?}");
        let ora = (
            s.contains("documento.txt") || s.contains('…'),
            s.contains("4 B"),
            s.contains("VIEW"),
        );
        assert!(
            !ora.0 || ora.1,
            "w={w}: il nome c'e' ma non la misura: {s:?}"
        );
        assert!(
            !ora.1 || ora.2,
            "w={w}: la misura c'e' ma non il modo: {s:?}"
        );
        assert!(
            (!ora.0 || prima.0) && (!ora.1 || prima.1) && (!ora.2 || prima.2),
            "w={w}: una cosa e' tornata: {s:?}"
        );
        visto_modo |= ora.2;
        visto_misura |= ora.1;
        prima = ora;
    }
    assert!(visto_modo && visto_misura);
    // la posizione sta sempre per intero, anche quando a destra non resta niente
    a.resize(28, 6);
    assert_eq!(stato(&mut a, 28, 6), " Ln 1  Col 1  byte 0  0%");
    pulisci(p);
}

#[test]
fn a_message_on_the_left_is_never_cut_by_the_right_side() {
    let (mut a, p) = app("note.txt", "uno\n", 50, 6);
    a.handle_key(k(KeyCode::Char('x'))); // in lettura: il messaggio "read-only: press F4 to edit"
    let s = stato(&mut a, 50, 6);
    assert!(s.starts_with(" read-only: press F4 to edit"), "{s:?}");
    // a 30 colonne il messaggio c'e' tutto e a destra non resta posto
    a.resize(30, 6);
    a.handle_key(k(KeyCode::Char('x')));
    let s = stato(&mut a, 30, 6);
    assert_eq!(s, " read-only: press F4 to edit");
    pulisci(p);
}

#[test]
fn encoding_and_line_ending_sit_between_the_size_and_the_mode() {
    let (mut a, p) = app("win.txt", "uno\r\ndue\r\n", 90, 6);
    let s = stato(&mut a, 90, 6);
    assert!(s.ends_with("win.txt  10 B  CRLF  VIEW"), "{s:?}");
    pulisci(p);
}

// ---------- la parte destra, con i numeri esatti ----------

use mrp_tui::ui::destra_stato;

/// `destra_stato` con una sinistra di 25 celle (" Ln 1  Col 1  byte 0  0%") su uno schermo largo `w`, con la misura e il modo "VIEW".
fn destra(w: usize, nome: &str) -> (String, Option<String>) {
    destra_stato(
        w,
        25,
        Vec::new(),
        nome.to_string(),
        vec!["4 B".to_string()],
        "VIEW".to_string(),
    )
}

#[test]
fn the_pieces_are_in_order_work_name_size_encoding_line_ending_then_the_mode() {
    let (d, m) = destra_stato(
        100,
        25,
        vec!["saving 5%".to_string()],
        "doc.txt".to_string(),
        vec![
            "4 B".to_string(),
            "windows-1252".to_string(),
            "CRLF".to_string(),
        ],
        "VIEW".to_string(),
    );
    assert_eq!(d, "saving 5%  doc.txt  4 B  windows-1252  CRLF  ");
    assert_eq!(m.as_deref(), Some("VIEW"));
}

#[test]
fn the_name_is_whole_while_it_fits_then_shortened_keeping_the_end() {
    // fuori dal nome restano la misura (3 + 2) e il modo (4 + 1), due spazi dopo il nome, tre dalla sinistra: 28 + 12 = 40 + il nome
    assert_eq!(destra(50, "abcdefghij").0, "abcdefghij  4 B  ");
    // una cella in meno: "…" e le ultime 8 lettere
    assert_eq!(destra(49, "abcdefghij").0, "…cdefghij  4 B  ");
    // e cosi' fino a 8 celle ("…" e 7 lettere), sotto il nome sparisce
    assert_eq!(destra(48, "abcdefghij").0, "…defghij  4 B  ");
    assert_eq!(destra(47, "abcdefghij").0, "4 B  ");
    // un nome corto non si accorcia mai
    assert_eq!(destra(100, "ab").0, "ab  4 B  ");
}

#[test]
fn the_size_goes_before_the_mode_and_the_mode_before_the_work() {
    let lungo = "un_nome_che_non_ci_sta_mai.txt";
    // senza nome restano misura (5) e modo (5) in 10 celle: a 38 colonne ci sono tutti e due, a 37 solo il modo
    let (d, m) = destra(38, lungo);
    assert_eq!((d.as_str(), m.as_deref()), ("4 B  ", Some("VIEW")));
    let (d, m) = destra(37, lungo);
    assert_eq!((d.as_str(), m.as_deref()), ("", Some("VIEW")));
    // il modo ci sta in 5 celle: a 33 colonne c'e', a 32 no
    let (d, m) = destra(33, lungo);
    assert_eq!((d.as_str(), m.as_deref()), ("", Some("VIEW")));
    let (d, m) = destra(32, lungo);
    assert_eq!((d.as_str(), m.as_deref()), ("", None));
    // il lavoro in corso (9 + 2) e' l'ultimo a cadere: con 11 celle e il modo (5) in 10 cade tutto, con 16 c'e' tutto tranne la misura
    let con_lavoro = |w: usize| {
        destra_stato(
            w,
            25,
            vec!["saving 5%".to_string()],
            lungo.to_string(),
            vec!["4 B".to_string()],
            "VIEW".to_string(),
        )
    };
    let (d, m) = con_lavoro(44);
    assert_eq!((d.as_str(), m.as_deref()), ("saving 5%  ", Some("VIEW")));
    let (d, m) = con_lavoro(38);
    assert_eq!((d.as_str(), m.as_deref()), ("", None));
}

#[test]
fn the_work_in_progress_falls_last_exactly_when_it_does_not_fit() {
    let lungo = "un_nome_che_non_ci_sta_mai.txt";
    let con_lavoro = |w: usize| {
        destra_stato(
            w,
            25,
            vec!["saving 5%".to_string()],
            lungo.to_string(),
            vec!["4 B".to_string()],
            "VIEW".to_string(),
        )
    };
    // il lavoro (9 + 2 celle) senza il modo: a 39 colonne ci sta giusto, a 38 no
    let (d, m) = con_lavoro(39);
    assert_eq!((d.as_str(), m.as_deref()), ("saving 5%  ", None));
    let (d, m) = con_lavoro(38);
    assert_eq!((d.as_str(), m.as_deref()), ("", None));
}

#[test]
fn the_mode_has_its_own_colour_and_a_three_row_screen_still_draws() {
    use mrp_tui::theme::{Theme, ThemeId};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let (mut a, p) = app("note.txt", "uno\n", 80, 6);
    a.adopt_look(ThemeId::Dark, "en");
    let t = Theme::for_id(ThemeId::Dark);
    assert_ne!(t.mode_view, t.mode_edit);
    let colore = |a: &mut App| {
        let mut term = Terminal::new(TestBackend::new(80, 6)).unwrap();
        term.draw(|f| mrp_tui::ui::draw(f, a)).unwrap();
        let buf = term.backend().buffer().clone();
        // il modo e' l'ultima parola della riga di stato (riga 4), prima dello spazio finale: colonna 79 - 4
        buf[(79 - 4, 4)].style()
    };
    let vista = colore(&mut a);
    assert_eq!(vista.fg, t.mode_view.fg);
    a.handle_key(k(KeyCode::F(4)));
    let modifica = colore(&mut a);
    assert_eq!(modifica.fg, t.mode_edit.fg);
    assert_ne!(vista.fg, modifica.fg);
    // tre righe bastano: testo, stato, tasti
    let (mut b, q) = app("note.txt", "uno\n", 60, 3);
    let righe = render_lines(&mut b, 60, 3);
    assert_eq!(righe.len(), 3);
    assert_eq!(righe[0], "1│uno");
    assert!(righe[1].starts_with(" Ln 1"), "{:?}", righe[1]);
    pulisci(p);
    pulisci(q);
}
