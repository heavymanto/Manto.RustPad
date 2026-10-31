//! La vista con una codifica diversa da UTF-8 (`ViewOptions::encoding`): gli stessi byte, caratteri diversi. Attesi a mano.

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
            std::env::temp_dir().join(format!("mrp_viewenc_test_{}_{}.txt", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn open(data: &[u8]) -> (TempFile, Document) {
    let f = TempFile::new(data);
    let d = Document::open(&f.0).unwrap();
    (f, d)
}

fn opts(name: &str) -> ViewOptions {
    ViewOptions {
        tab_width: 4,
        encoding: Encoding::by_name(name).unwrap(),
        ..ViewOptions::default()
    }
}

fn first_row_text(d: &Document, o: &ViewOptions) -> String {
    rows_from(d, 0, 1, 0, 80, o).remove(0).text
}

#[test]
fn the_same_bytes_read_differently_in_utf8_and_in_windows_1252() {
    let (_f, d) = open(b"caff\xe8\t\x80\n");
    assert_eq!(
        first_row_text(&d, &opts("windows-1252")),
        "caff\u{e8}   \u{20ac}"
    );
    assert_eq!(
        first_row_text(&d, &opts("utf-8")),
        "caff\u{fffd}   \u{fffd}"
    );
    let r = rows_from(&d, 0, 1, 0, 80, &opts("windows-1252")).remove(0);
    assert_eq!((r.start, r.end), (0, 7));
}

#[test]
fn utf8_bytes_read_as_a_code_page_are_two_characters() {
    // "é" in UTF-8 sono i due byte C3 A9: in windows-1252 sono "Ã" e "©"
    let (_f, d) = open(b"\xc3\xa9x\n");
    assert_eq!(first_row_text(&d, &opts("windows-1252")), "\u{c3}\u{a9}x");
    assert_eq!(first_row_text(&d, &opts("utf-8")), "\u{e9}x");
    assert_eq!(col_of(&d, 0, 2, &opts("windows-1252")), 2);
    assert_eq!(col_of(&d, 0, 2, &opts("utf-8")), 1);
    assert_eq!(offset_at_col(&d, 0, 1, &opts("windows-1252")), 1);
    assert_eq!(offset_at_col(&d, 0, 2, &opts("windows-1252")), 2);
    assert_eq!(offset_at_col(&d, 0, 1, &opts("utf-8")), 2);
}

#[test]
fn undefined_and_control_bytes_in_a_code_page() {
    // 0x81 non e' definito in windows-1252 e vale il controllo C1 U+0081 (come in Windows), che la vista mostra come il
    // punto mediano, mentre 0x85 e' "…"; in ISO-8859-1 sono entrambi controlli C1
    let (_f, d) = open(b"a\x81b\x85c\n");
    assert_eq!(
        first_row_text(&d, &opts("windows-1252")),
        "a\u{b7}b\u{2026}c"
    );
    assert_eq!(first_row_text(&d, &opts("iso-8859-1")), "a\u{b7}b\u{b7}c");
}

#[test]
fn dos_box_drawing_in_code_page_437() {
    let (_f, d) = open(b"\xc9\xcd\xbb\n\xba \xba\n");
    let righe = rows_from(&d, 0, 2, 0, 80, &opts("ibm437"));
    assert_eq!(righe[0].text, "\u{2554}\u{2550}\u{2557}");
    assert_eq!(righe[1].text, "\u{2551} \u{2551}");
}

#[test]
fn hidden_carriage_return_and_window_with_a_code_page() {
    let (_f, d) = open(b"abc\xe9\r\n");
    let r = rows_from(&d, 0, 1, 0, 80, &opts("windows-1252")).remove(0);
    assert_eq!(r.text, "abc\u{e9}");
    assert_eq!(col_of(&d, 0, 5, &opts("windows-1252")), 4);
    // finestra a meta' riga: 100 volte "é" (0xE9), colonne 5..15
    let mut dati = vec![0xe9u8; 100];
    dati.push(b'\n');
    let (_g, d2) = open(&dati);
    let r = rows_from(&d2, 0, 1, 5, 10, &opts("windows-1252")).remove(0);
    assert_eq!(r.text, "\u{e9}".repeat(10));
    assert!(r.more_left && r.more_right);
}

#[test]
fn a_column_block_counts_code_page_characters_one_cell_each() {
    // C3 A9 'x' \n  'a' 'b' 'c' \n ; blocco colonne [2,3)
    let (_f, d) = open(b"\xc3\xa9x\nabc\n");
    let blocco = ColumnBlock {
        first_row: 0,
        last_row: 4,
        col_a: 2,
        col_b: 3,
    };
    let s = block_spans(&d, &blocco, &opts("windows-1252"), 10).unwrap();
    assert_eq!(
        s,
        vec![
            RowSpan {
                row_start: 0,
                start: 2,
                end: 3,
                width: 3,
                reaches: true
            },
            RowSpan {
                row_start: 4,
                start: 6,
                end: 7,
                width: 3,
                reaches: true
            },
        ]
    );
    // in UTF-8 la prima riga ha solo due celle ("é", "x"): il blocco [2,3) non copre niente
    let u = block_spans(&d, &blocco, &opts("utf-8"), 10).unwrap();
    assert_eq!(u[0].width, 2);
    assert_eq!((u[0].start, u[0].end), (3, 3));
}
