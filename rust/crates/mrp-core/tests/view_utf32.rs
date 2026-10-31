//! La vista e il blocco a colonne con UTF-32 (`ViewOptions::encoding`): gli stessi byte letti a unita' di 4 byte. Il
//! documento deve avere l'a-capo giusto (`Document::set_newline`): la vista lo trova con la scansione per righe.
//! Attesi scritti a mano da ARCH (le posizioni sono multipli di 4).

use mrp_core::block::{block_spans, block_text, insert_block, BlockError, ColumnBlock, RowSpan};
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
            std::env::temp_dir().join(format!("mrp_view32_test_{}_{}.txt", std::process::id(), n));
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
    s.chars().flat_map(|c| (c as u32).to_le_bytes()).collect()
}

fn be(s: &str) -> Vec<u8> {
    s.chars().flat_map(|c| (c as u32).to_be_bytes()).collect()
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

const LE32: Encoding = Encoding::Utf32Le;
const BE32: Encoding = Encoding::Utf32Be;

#[test]
fn rows_of_utf32_text_in_both_endians() {
    for (e, bytes) in [(LE32, le("ab\tc\nxy")), (BE32, be("ab\tc\nxy"))] {
        let (_f, d, o) = open(&bytes, e);
        let r = rows_from(&d, 0, 3, 0, 80, &o);
        assert_eq!(r.len(), 2, "{}", e.name());
        assert_eq!((r[0].start, r[0].end, r[0].text.as_str()), (0, 16, "ab  c"));
        assert!(r[0].starts_line);
        assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (20, 28, "xy"));
        assert!(r[1].starts_line);
    }
}

#[test]
fn characters_with_the_byte_0a_do_not_break_rows() {
    // LE: U+0A0A e' `0A 0A 00 00`; U+A0000 U+100000 sono `00 00 0A 00 | 00 00 10 00` (contengono `0A 00 00 00` sfasato)
    let (_f, d, o) = open(&le("A\u{0A0A}\u{A0000}\u{100000}\nB"), LE32);
    let r = rows_from(&d, 0, 3, 0, 80, &o);
    assert_eq!(r.len(), 2);
    assert_eq!((r[0].start, r[0].end), (0, 16));
    assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (20, 24, "B"));
    // BE: U+10000 U+A0000 sono `00 01 00 00 | 00 0A 00 00` (contengono `00 00 00 0A` sfasato)
    let (_g, d, o) = open(&be("\u{10000}\u{A0000}\nB"), BE32);
    let r = rows_from(&d, 0, 3, 0, 80, &o);
    assert_eq!(r.len(), 2);
    assert_eq!((r[0].start, r[0].end), (0, 8));
    assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (12, 16, "B"));
}

#[test]
fn a_character_outside_the_bmp_is_one_character_of_two_cells() {
    // "a", "é", "😀" (2 celle), "b": byte 0, 4, 8, 12, a-capo a 16
    for (e, bytes) in [
        (LE32, le("a\u{e9}\u{1f600}b\n")),
        (BE32, be("a\u{e9}\u{1f600}b\n")),
    ] {
        let (_f, d, o) = open(&bytes, e);
        let r = rows_from(&d, 0, 1, 0, 80, &o).remove(0);
        assert_eq!(r.text, "a\u{e9}\u{1f600}b", "{}", e.name());
        assert_eq!((r.start, r.end), (0, 16));
        assert_eq!(col_of(&d, 0, 0, &o), 0);
        assert_eq!(col_of(&d, 0, 4, &o), 1);
        assert_eq!(col_of(&d, 0, 8, &o), 2);
        assert_eq!(col_of(&d, 0, 12, &o), 4);
        assert_eq!(col_of(&d, 0, 16, &o), 5);
        assert_eq!(offset_at_col(&d, 0, 0, &o), 0);
        assert_eq!(offset_at_col(&d, 0, 1, &o), 4);
        assert_eq!(offset_at_col(&d, 0, 2, &o), 8);
        assert_eq!(offset_at_col(&d, 0, 4, &o), 12);
        assert_eq!(offset_at_col(&d, 0, 9, &o), 16);
    }
}

#[test]
fn invalid_units_and_a_short_tail_are_replacement_characters() {
    // LE: D83D (surrogato), "a", DE00 (surrogato), 0x00110000 (sopra U+10FFFF), poi un byte spaiato 0x62
    let mut bytes = vec![0x3D, 0xD8, 0x00, 0x00];
    bytes.extend(le("a"));
    bytes.extend([0x00, 0xDE, 0x00, 0x00]);
    bytes.extend([0x00, 0x00, 0x11, 0x00]);
    bytes.push(0x62);
    let (_f, d, o) = open(&bytes, LE32);
    let r = rows_from(&d, 0, 1, 0, 80, &o).remove(0);
    assert_eq!(r.text, "\u{fffd}a\u{fffd}\u{fffd}\u{fffd}");
    assert_eq!((r.start, r.end), (0, 17));
    // la coda e' un carattere solo, anche se lunga 3 byte
    assert_eq!(col_of(&d, 0, 16, &o), 4);
    let (_g, d, o) = open(&[le("a"), vec![1, 2, 3]].concat(), LE32);
    let r = rows_from(&d, 0, 1, 0, 80, &o).remove(0);
    assert_eq!(r.text, "a\u{fffd}");
    assert_eq!((r.start, r.end), (0, 7));
}

#[test]
fn a_long_utf32_row_is_cut_at_an_aligned_position() {
    // 100 "a" senza a-capo e un max_scan che non e' multiplo di 4: la riga si taglia, e il taglio e' multiplo di 4
    let (_f, d, o) = open(&le(&"a".repeat(100)), LE32);
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
    assert_eq!(r[0].start % 4, 0);
    assert_eq!(r[1].start % 4, 0);
    assert_eq!(r[0].end, r[1].start);
    assert!(r[0].more_right);
}

#[test]
fn a_cut_row_does_not_start_a_line_even_if_the_previous_byte_is_0a() {
    // BE: U+010A e' `00 00 01 0A` (ultimo byte 0A); LE: U+0A00 e' `00 0A 00 00`. Il taglio a 20 byte cade fra due di questi
    for (e, bytes) in [
        (LE32, le(&"\u{0A00}".repeat(40))),
        (BE32, be(&"\u{010A}".repeat(40))),
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
fn a_column_block_counts_utf32_characters() {
    // "ab\n" poi "😀cd\n". La riga 0 sta in 0..12 (a 0..4, b 4..8, a-capo 8..12); la riga 1 comincia a 12: la faccina
    // (2 celle, colonne 0-1) 12..16, 'c' (colonna 2) 16..20, 'd' (colonna 3) 20..24.
    // Blocco colonne [1,3): comincia al primo carattere con colonna di inizio >= 1 e finisce al primo con colonna >= 3.
    let (_f, d, o) = open(&le("ab\n\u{1f600}cd\n"), LE32);
    let blocco = ColumnBlock {
        first_row: 0,
        last_row: 12,
        col_a: 1,
        col_b: 3,
    };
    let s = block_spans(&d, &blocco, &o, 10).unwrap();
    assert_eq!(
        s,
        vec![
            RowSpan {
                row_start: 0,
                start: 4,
                end: 8,
                width: 2,
                reaches: true
            },
            RowSpan {
                row_start: 12,
                start: 16,
                end: 20,
                width: 4,
                reaches: true
            },
        ]
    );
}

#[test]
fn crlf_in_utf32_hides_the_carriage_return() {
    for (e, bytes) in [(LE32, le("ab\r\ncd\r\n")), (BE32, be("ab\r\ncd\r\n"))] {
        let (_f, d, o) = open(&bytes, e);
        let r = rows_from(&d, 0, 5, 0, 80, &o);
        assert_eq!(r.len(), 2, "{}", e.name());
        assert_eq!((r[0].start, r[0].end, r[0].text.as_str()), (0, 12, "ab"));
        assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (16, 28, "cd"));
    }
}

#[test]
fn the_eof_row_appears_after_a_final_utf32_newline() {
    let (_f, d, mut o) = open(&le("ab\n"), LE32);
    o.show_eof_row = true;
    let r = rows_from(&d, 0, 5, 0, 80, &o);
    assert_eq!(r.len(), 2);
    assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (12, 12, ""));
    // senza a-capo finale, no
    let (_g, d, mut o) = open(&le("ab"), LE32);
    o.show_eof_row = true;
    assert_eq!(rows_from(&d, 0, 5, 0, 80, &o).len(), 1);
    // un carattere che contiene il byte 0A (LE: U+0A00 = 00 0A 00 00) non e' un a-capo finale
    let (_h, d, mut o) = open(&le("a\u{0A00}"), LE32);
    o.show_eof_row = true;
    assert_eq!(rows_from(&d, 0, 5, 0, 80, &o).len(), 1);
    // e `0A 00 00` in coda (tre byte) non e' un a-capo
    let (_i, d, mut o) = open(&[le("ab"), vec![0x0A, 0, 0]].concat(), LE32);
    o.show_eof_row = true;
    assert_eq!(rows_from(&d, 0, 5, 0, 80, &o).len(), 1);
}

#[test]
fn block_text_and_insert_use_the_encoded_newline_and_space() {
    // "ab\ncd\n": blocco colonne [1,2): 'b' (byte 4..8) e 'd' (byte 16..20); la riga 1 comincia a 12
    let blocco = ColumnBlock {
        first_row: 0,
        last_row: 12,
        col_a: 1,
        col_b: 2,
    };
    let (_f, d, o) = open(&le("ab\ncd\n"), LE32);
    let s = block_spans(&d, &blocco, &o, 10).unwrap();
    assert_eq!((s[0].start, s[0].end, s[1].start, s[1].end), (4, 8, 16, 20));
    assert_eq!(
        block_text(&d, &s, 100).unwrap(),
        vec![0x62, 0, 0, 0, 0x0A, 0, 0, 0, 0x64, 0, 0, 0]
    );
    assert_eq!(block_text(&d, &s, 12).unwrap().len(), 12);
    assert!(matches!(
        block_text(&d, &s, 11),
        Err(BlockError::TooManyBytes)
    ));
    let (_g, d, o) = open(&be("ab\ncd\n"), BE32);
    let s = block_spans(&d, &blocco, &o, 10).unwrap();
    assert_eq!(
        block_text(&d, &s, 100).unwrap(),
        vec![0, 0, 0, 0x62, 0, 0, 0, 0x0A, 0, 0, 0, 0x64]
    );
    // inserimento con riempimento: "a\nbcd\n", colonna 3: la riga corta ("a") si riempie con due spazi UTF-32
    for (e, codifica) in [
        (LE32, le as fn(&str) -> Vec<u8>),
        (BE32, be as fn(&str) -> Vec<u8>),
    ] {
        let (_h, mut d, o) = open(&codifica("a\nbcd\n"), e);
        let punto = ColumnBlock {
            first_row: 0,
            last_row: 8,
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
fn a_character_across_the_read_block_boundary_is_one_character() {
    // "a" e 20000 faccine: i caratteri stanno a 4, 8, ...; il confine dei 64 KiB (65536) cade su un confine di unita',
    // ma la lettura riparte da `pos` e una finestra di 4 byte non deve mai spezzare un carattere
    let testo = format!("a{}\n", "\u{1f600}".repeat(20_000));
    for (e, bytes) in [(LE32, le(&testo)), (BE32, be(&testo))] {
        let (_f, d, o) = open(&bytes, e);
        let r = rows_from(&d, 0, 1, 0, 100_000, &o).remove(0);
        assert_eq!(
            r.text,
            format!("a{}", "\u{1f600}".repeat(20_000)),
            "{}",
            e.name()
        );
        assert_eq!(r.end, 4 + 4 * 20_000);
    }
}
