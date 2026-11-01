//! I caratteri speciali visibili (fase 2, 1.2): con `ViewOptions::special` lo spazio si vede come `·`, la tabulazione come
//! `→` (e il resto delle sue celle vuoto), un carattere di controllo come `¤` e la fine di una riga vera come `¶`; la
//! larghezza in celle non cambia mai (un carattere = le celle di prima), quindi colonne, mouse e scorrimento restano quelli di
//! sempre. Scritti da ARCH: gli attesi sono calcolati a mano sui byte di prova.

use mrp_core::view::{col_of, offset_at_col, rows_from, Row, ViewOptions};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use unicode_width::UnicodeWidthStr;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Guard(PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn doc(dati: &[u8]) -> (Document, Guard) {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_special_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    (Document::open(&p).unwrap(), Guard(p))
}

fn speciali() -> ViewOptions {
    ViewOptions {
        special: true,
        ..ViewOptions::default()
    }
}

fn testi(righe: &[Row]) -> Vec<&str> {
    righe.iter().map(|r| r.text.as_str()).collect()
}

fn vista(dati: &[u8], opts: &ViewOptions, hscroll: usize, cols: usize) -> Vec<String> {
    let (d, _g) = doc(dati);
    rows_from(&d, 0, 20, hscroll, cols, opts)
        .into_iter()
        .map(|r| r.text)
        .collect()
}

#[test]
fn the_default_is_off_and_nothing_changes() {
    assert!(!ViewOptions::default().special);
    assert_eq!(
        vista(b"a b\tc\n\x01\n", &ViewOptions::default(), 0, 80),
        vec!["a b c", "\u{b7}"]
    );
}

#[test]
fn spaces_tabs_and_the_end_of_line_are_drawn() {
    // 'a' ' ' 'b' = colonne 0..3; la tabulazione parte dalla colonna 3 e arriva alla 4: una cella
    assert_eq!(
        vista(b"a b\tc\n", &speciali(), 0, 80),
        vec!["a\u{b7}b\u{2192}c\u{b6}"]
    );
    // la tabulazione dalla colonna 2 occupa 2 celle: la freccia e uno spazio
    assert_eq!(
        vista(b"ab\tc\n", &speciali(), 0, 80),
        vec!["ab\u{2192} c\u{b6}"]
    );
    // dalla colonna 0 ne occupa 4
    assert_eq!(
        vista(b"\tx\n", &speciali(), 0, 80),
        vec!["\u{2192}   x\u{b6}"]
    );
}

#[test]
fn empty_lines_show_only_the_marker_and_the_last_line_without_a_newline_none() {
    assert_eq!(
        vista(b"\n\nx", &speciali(), 0, 80),
        vec!["\u{b6}", "\u{b6}", "x"]
    );
}

#[test]
fn crlf_shows_one_marker_and_a_lone_cr_in_the_middle_is_a_control_character() {
    assert_eq!(
        vista(b"x\r\ny\r\n", &speciali(), 0, 80),
        vec!["x\u{b6}", "y\u{b6}"]
    );
    // un \r in mezzo a una riga e' un carattere di controllo, e lo si distingue dallo spazio (che e' `·`)
    assert_eq!(vista(b"a\rb\n", &speciali(), 0, 80), vec!["a\u{a4}b\u{b6}"]);
    assert_eq!(
        vista(b"a\x01 b\n", &speciali(), 0, 80),
        vec!["a\u{a4}\u{b7}b\u{b6}"]
    );
}

#[test]
fn wide_characters_keep_their_width_before_the_marker() {
    // due caratteri larghi = 4 celle, il segno sta alla colonna 4
    assert_eq!(
        vista("日本\n".as_bytes(), &speciali(), 0, 80),
        vec!["日本\u{b6}"]
    );
    assert_eq!(
        vista("日本\n".as_bytes(), &speciali(), 0, 4),
        vec!["日本"],
        "il segno e' fuori dalla finestra"
    );
    assert_eq!(
        vista("日本\n".as_bytes(), &speciali(), 0, 5),
        vec!["日本\u{b6}"]
    );
}

#[test]
fn the_marker_follows_the_horizontal_window() {
    let d = b"abcdef\n";
    // il segno e' alla colonna 6
    assert_eq!(vista(d, &speciali(), 3, 10), vec!["def\u{b6}"]);
    assert_eq!(vista(d, &speciali(), 3, 4), vec!["def\u{b6}"]);
    assert_eq!(vista(d, &speciali(), 3, 3), vec!["def"]);
    assert_eq!(vista(d, &speciali(), 6, 5), vec!["\u{b6}"]);
    assert_eq!(vista(d, &speciali(), 7, 5), vec![""]);
}

#[test]
fn a_tab_cut_by_the_window_is_blank_cells_not_an_arrow() {
    // la tabulazione occupa le colonne 0..4; la finestra parte alla 1: restano 3 celle vuote
    assert_eq!(vista(b"\tx\n", &speciali(), 1, 10), vec!["   x\u{b6}"]);
    // la finestra di 2 celle taglia la tabulazione a meta': due spazi, niente segno
    assert_eq!(vista(b"\tx\n", &speciali(), 0, 2), vec!["  "]);
}

#[test]
fn the_marker_never_changes_more_right_or_the_row_extent() {
    let (d, _g) = doc(b"abcdef\n");
    let a = rows_from(&d, 0, 5, 0, 6, &speciali());
    let b = rows_from(&d, 0, 5, 0, 6, &ViewOptions::default());
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!((x.start, x.end), (y.start, y.end));
        assert_eq!(x.more_left, y.more_left);
        assert_eq!(x.more_right, y.more_right);
        assert_eq!(x.starts_line, y.starts_line);
    }
}

#[test]
fn a_row_cut_by_the_scan_limit_has_no_marker_but_the_real_end_has() {
    let (d, _g) = doc(b"abcdefghij\n");
    let o = ViewOptions {
        max_scan: 5,
        ..speciali()
    };
    let r = rows_from(&d, 0, 10, 0, 80, &o);
    assert_eq!(testi(&r), vec!["abcde", "fghij", "\u{b6}"]);
}

#[test]
fn the_empty_row_of_the_end_of_file_has_no_marker() {
    let (d, _g) = doc(b"a\n");
    let o = ViewOptions {
        show_eof_row: true,
        ..speciali()
    };
    let r = rows_from(&d, 0, 10, 0, 80, &o);
    assert_eq!(testi(&r), vec!["a\u{b6}", ""]);
}

#[test]
fn columns_and_clicks_are_the_same_with_or_without_the_option() {
    let (d, _g) = doc(b"a\tb c\x01d\n\tx\n");
    let normale = ViewOptions::default();
    let sp = speciali();
    for row_start in [0u64, 8] {
        for off in row_start..=(row_start + 8).min(d.len()) {
            assert_eq!(
                col_of(&d, row_start, off, &normale),
                col_of(&d, row_start, off, &sp),
                "{row_start} {off}"
            );
        }
        for col in 0..12u64 {
            assert_eq!(
                offset_at_col(&d, row_start, col, &normale),
                offset_at_col(&d, row_start, col, &sp),
                "{row_start} {col}"
            );
        }
    }
}

#[test]
fn the_text_never_exceeds_the_window_width() {
    let dati = "x\ty 日 z\t\t\u{1}\n\n \t \nlungo lungo lungo lungo\n".as_bytes();
    for cols in 1..24usize {
        for hscroll in 0..6usize {
            for r in vista(dati, &speciali(), hscroll, cols) {
                assert!(
                    UnicodeWidthStr::width(r.as_str()) <= cols,
                    "{hscroll} {cols} {r:?}"
                );
            }
        }
    }
}

#[test]
fn a_row_read_only_in_part_never_gets_a_marker_mid_line() {
    // una riga di segni combinanti (larghezza 0): la lettura si ferma al limite di sicurezza molto prima della fine,
    // con la colonna ancora dentro la finestra; il segno di fine riga non deve comparire a meta' della riga
    let mut dati = b"a".to_vec();
    for _ in 0..200 {
        dati.extend_from_slice("\u{301}".as_bytes());
    }
    dati.push(b'\n');
    let r = vista(&dati, &speciali(), 0, 4);
    assert_eq!(r.len(), 1);
    assert!(!r[0].contains('\u{b6}'), "{:?}", r[0]);
}
