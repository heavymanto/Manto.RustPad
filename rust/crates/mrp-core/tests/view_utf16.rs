//! La vista e il blocco a colonne con UTF-16 (`ViewOptions::encoding`): gli stessi byte letti a unita' di 2 byte. Il
//! documento deve avere l'a-capo giusto (`Document::set_newline`): la vista lo trova con la scansione per righe.
//! Attesi scritti a mano da ARCH.

use mrp_core::block::{block_spans, ColumnBlock, RowSpan};
use mrp_core::encoding::Encoding;
use mrp_core::view::{col_of, offset_at_col, rows_from, ViewOptions};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_view16_test_{}_{}.txt", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn be(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

fn open(data: &[u8], e: Encoding) -> (TempFile, Document, ViewOptions) {
    let f = TempFile::new(data);
    let mut d = Document::open(&f.0).unwrap();
    assert!(d.set_newline(e.newline()));
    let o = ViewOptions {
        tab_width: 4,
        encoding: e,
        ..ViewOptions::default()
    };
    (f, d, o)
}

const LE16: Encoding = Encoding::Utf16Le;
const BE16: Encoding = Encoding::Utf16Be;

#[test]
fn rows_of_utf16_text_in_both_endians() {
    for (e, bytes) in [(LE16, le("ab\tc\nxy")), (BE16, be("ab\tc\nxy"))] {
        let (_f, d, o) = open(&bytes, e);
        let r = rows_from(&d, 0, 3, 0, 80, &o);
        assert_eq!(r.len(), 2, "{}", e.name());
        assert_eq!((r[0].start, r[0].end, r[0].text.as_str()), (0, 8, "ab  c"));
        assert!(r[0].starts_line);
        assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (10, 14, "xy"));
        assert!(r[1].starts_line);
    }
}

#[test]
fn characters_with_the_byte_0a_do_not_break_rows() {
    // LE: U+010A e' `0A 01`; U+0A15 U+0100 sono `15 0A 00 01` (contiene `0A 00` sfasato)
    let testo = "A\u{010A}\u{0A15}\u{0100}\nB";
    let (_f, d, o) = open(&le(testo), LE16);
    let r = rows_from(&d, 0, 3, 0, 80, &o);
    assert_eq!(r.len(), 2);
    assert_eq!((r[0].start, r[0].end), (0, 8));
    assert_eq!(r[0].text, "A\u{010A}\u{0A15}\u{0100}");
    assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (10, 12, "B"));
    // BE: U+4100 U+0A15 sono `41 00 0A 15` (contiene `00 0A` sfasato)
    let testo = "\u{4100}\u{0A15}\nB";
    let (_g, d, o) = open(&be(testo), BE16);
    let r = rows_from(&d, 0, 3, 0, 80, &o);
    assert_eq!(r.len(), 2);
    assert_eq!((r[0].start, r[0].end), (0, 4));
    assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (6, 8, "B"));
}

#[test]
fn surrogate_pairs_are_one_character_of_two_cells() {
    // "a", "é", "😀" (2 celle), "b": byte 0, 2, 4, 8, fine 10
    let (_f, d, o) = open(&le("a\u{e9}\u{1f600}b\n"), LE16);
    let r = rows_from(&d, 0, 1, 0, 80, &o).remove(0);
    assert_eq!(r.text, "a\u{e9}\u{1f600}b");
    assert_eq!((r.start, r.end), (0, 10));
    assert_eq!(col_of(&d, 0, 0, &o), 0);
    assert_eq!(col_of(&d, 0, 2, &o), 1);
    assert_eq!(col_of(&d, 0, 4, &o), 2);
    assert_eq!(col_of(&d, 0, 8, &o), 4);
    assert_eq!(col_of(&d, 0, 10, &o), 5);
    assert_eq!(offset_at_col(&d, 0, 0, &o), 0);
    assert_eq!(offset_at_col(&d, 0, 1, &o), 2);
    assert_eq!(offset_at_col(&d, 0, 2, &o), 4);
    assert_eq!(offset_at_col(&d, 0, 4, &o), 8);
    assert_eq!(offset_at_col(&d, 0, 9, &o), 10);
}

#[test]
fn lone_surrogates_and_a_stray_byte_are_replacement_characters() {
    // LE: alto spaiato D83D, "a", basso spaiato DE00, poi un byte spaiato 0x62
    let bytes = vec![0x3D, 0xD8, 0x61, 0x00, 0x00, 0xDE, 0x62];
    let (_f, d, o) = open(&bytes, LE16);
    let r = rows_from(&d, 0, 1, 0, 80, &o).remove(0);
    assert_eq!(r.text, "\u{fffd}a\u{fffd}\u{fffd}");
    assert_eq!((r.start, r.end), (0, 7));
    assert_eq!(col_of(&d, 0, 6, &o), 3);
}

#[test]
fn a_long_utf16_row_is_cut_at_an_aligned_position() {
    // 100 "a" senza a-capo e un max_scan dispari nelle opzioni di default: la riga si taglia, e il taglio e' pari
    let (_f, d, o) = open(&le(&"a".repeat(100)), LE16);
    let r = rows_from(
        &d,
        0,
        2,
        0,
        400,
        &ViewOptions {
            max_scan: 51,
            ..o.clone()
        },
    );
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].start % 2, 0);
    assert_eq!(r[1].start % 2, 0);
    assert_eq!(r[0].end, r[1].start);
    assert!(r[0].more_right);
}

#[test]
fn a_cut_row_does_not_start_a_line_even_if_the_previous_byte_is_0a() {
    // LE: U+0A00 e' `00 0A` (ultimo byte 0A); BE: U+010A e' `01 0A`. Il taglio a 20 byte cade fra due di questi caratteri
    for (e, bytes) in [
        (LE16, le(&"\u{0A00}".repeat(40))),
        (BE16, be(&"\u{010A}".repeat(40))),
    ] {
        let (_f, d, o) = open(&bytes, e);
        let o = ViewOptions { max_scan: 20, ..o };
        let r = rows_from(&d, 0, 3, 0, 400, &o);
        assert_eq!(r.len(), 3, "{}", e.name());
        assert!(r[0].starts_line);
        assert_eq!(r[1].start, 20);
        assert!(!r[1].starts_line, "{}", e.name());
        assert!(!r[2].starts_line, "{}", e.name());
    }
}

#[test]
fn a_column_block_counts_utf16_characters() {
    // "ab\n" poi "😀cd\n". La riga 0 sta in 0..6 (a 0..2, b 2..4, a-capo 4..6); la riga 1 comincia a 6: la faccina
    // (2 celle, colonne 0-1) 6..10, 'c' (colonna 2) 10..12, 'd' (colonna 3) 12..14.
    // Blocco colonne [1,3): comincia al primo carattere con colonna di inizio >= 1 e finisce al primo con colonna >= 3.
    let (_f, d, o) = open(&le("ab\n\u{1f600}cd\n"), LE16);
    let blocco = ColumnBlock {
        first_row: 0,
        last_row: 8,
        col_a: 1,
        col_b: 3,
    };
    let s = block_spans(&d, &blocco, &o, 10).unwrap();
    assert_eq!(
        s,
        vec![
            RowSpan {
                row_start: 0,
                start: 2,
                end: 4,
                width: 2,
                reaches: true
            },
            RowSpan {
                row_start: 6,
                start: 10,
                end: 12,
                width: 4,
                reaches: true
            },
        ]
    );
}

#[test]
fn crlf_in_utf16_hides_the_carriage_return() {
    for (e, bytes) in [(LE16, le("ab\r\ncd\r\n")), (BE16, be("ab\r\ncd\r\n"))] {
        let (_f, d, o) = open(&bytes, e);
        let r = rows_from(&d, 0, 5, 0, 80, &o);
        assert_eq!(r.len(), 2, "{}", e.name());
        assert_eq!((r[0].start, r[0].end, r[0].text.as_str()), (0, 6, "ab"));
        assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (8, 14, "cd"));
    }
}

#[test]
fn the_eof_row_appears_after_a_final_utf16_newline() {
    let (_f, d, mut o) = open(&le("ab\n"), LE16);
    o.show_eof_row = true;
    let r = rows_from(&d, 0, 5, 0, 80, &o);
    assert_eq!(r.len(), 2);
    assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (6, 6, ""));
    // senza a-capo finale, no
    let (_g, d, mut o) = open(&le("ab"), LE16);
    o.show_eof_row = true;
    assert_eq!(rows_from(&d, 0, 5, 0, 80, &o).len(), 1);
    // un carattere che finisce con il byte 0A (LE: U+0A00 = 00 0A) non e' un a-capo finale
    let (_h, d, mut o) = open(&le("a\u{0A00}"), LE16);
    o.show_eof_row = true;
    assert_eq!(rows_from(&d, 0, 5, 0, 80, &o).len(), 1);
}

#[test]
fn block_text_and_insert_use_the_encoded_newline_and_space() {
    // "ab\ncd\n": blocco colonne [1,2): 'b' (byte 2..4) e 'd' (byte 8..10); la riga 1 comincia a 6
    let blocco = ColumnBlock {
        first_row: 0,
        last_row: 6,
        col_a: 1,
        col_b: 2,
    };
    let (_f, d, o) = open(&le("ab\ncd\n"), LE16);
    let s = block_spans(&d, &blocco, &o, 10).unwrap();
    assert_eq!((s[0].start, s[0].end, s[1].start, s[1].end), (2, 4, 8, 10));
    use mrp_core::block::{block_text, insert_block, BlockError};
    assert_eq!(
        block_text(&d, &s, 100).unwrap(),
        vec![0x62, 0x00, 0x0A, 0x00, 0x64, 0x00]
    );
    assert_eq!(block_text(&d, &s, 6).unwrap().len(), 6);
    assert!(matches!(
        block_text(&d, &s, 5),
        Err(BlockError::TooManyBytes)
    ));
    let (_g, d, o) = open(&be("ab\ncd\n"), BE16);
    let s = block_spans(&d, &blocco, &o, 10).unwrap();
    assert_eq!(
        block_text(&d, &s, 100).unwrap(),
        vec![0x00, 0x62, 0x00, 0x0A, 0x00, 0x64]
    );
    // inserimento con riempimento: "a\nbcd\n", colonna 3: la riga corta ("a") si riempie con due spazi UTF-16
    for (e, codifica) in [
        (LE16, le as fn(&str) -> Vec<u8>),
        (BE16, be as fn(&str) -> Vec<u8>),
    ] {
        let (_h, mut d, o) = open(&codifica("a\nbcd\n"), e);
        let punto = ColumnBlock {
            first_row: 0,
            last_row: 4,
            col_a: 3,
            col_b: 3,
        };
        let s = block_spans(&d, &punto, &o, 10).unwrap();
        assert_eq!(
            insert_block(&mut d, &s, 3, &codifica("X"), true).unwrap(),
            2
        );
        let len = d.len();
        assert_eq!(
            d.read_range(0, len),
            codifica("a  X\nbcdX\n"),
            "{}",
            e.name()
        );
    }
}

#[test]
fn a_surrogate_pair_across_the_read_block_boundary_is_one_character() {
    // "a" e 20000 faccine: le coppie stanno a 2, 6, ...; quella a 65534 attraversa il confine dei 64 KiB di lettura
    let testo = format!("a{}\n", "\u{1f600}".repeat(20_000));
    for (e, bytes) in [(LE16, le(&testo)), (BE16, be(&testo))] {
        let (_f, d, o) = open(&bytes, e);
        let r = rows_from(&d, 0, 1, 0, 100_000, &o).remove(0);
        assert_eq!(
            r.text,
            format!("a{}", "\u{1f600}".repeat(20_000)),
            "{}",
            e.name()
        );
        assert_eq!(r.end, 2 + 4 * 20_000);
    }
}
