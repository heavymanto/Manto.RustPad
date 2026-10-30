//! Test del modello di visualizzazione (`view`): da un documento a righe di celle di terminale. Scritti da ARCH prima
//! dell'implementazione: gli attesi sono a mano o vengono da un oracolo ingenuo scritto qui, mai dal codice.

use mrp_core::scan::DEFAULT_MAX_SCAN;
use mrp_core::view::{col_of, offset_at_col, rows_from, Row, ViewOptions};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use unicode_width::UnicodeWidthChar;

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_view_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn open(&self, data: &[u8]) -> Document {
        // un nome nuovo a ogni apertura: su Windows un file ancora mappato non si puo' troncare
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = self.0.join(format!("a{n}.txt"));
        std::fs::write(&p, data).unwrap();
        Document::open(&p).unwrap()
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn opts(tab: usize, max_scan: u64) -> ViewOptions {
    ViewOptions {
        tab_width: tab,
        max_scan,
        show_eof_row: false,
        ..ViewOptions::default()
    }
}

fn texts(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(|r| r.text.as_str()).collect()
}

// ---------- a mano ----------

#[test]
fn plain_rows() {
    let dir = TestDir::new();
    let d = dir.open(b"ab\ncd\n");
    let rows = rows_from(&d, 0, 5, 0, 10, &opts(4, DEFAULT_MAX_SCAN));
    assert_eq!(
        texts(&rows),
        vec!["ab", "cd"],
        "nessuna riga fantasma dopo l'ultimo a-capo"
    );
    assert_eq!((rows[0].start, rows[0].end), (0, 2));
    assert_eq!((rows[1].start, rows[1].end), (3, 5));
    assert!(rows
        .iter()
        .all(|r| r.starts_line && !r.more_left && !r.more_right));
}

#[test]
fn count_limits_the_rows_and_top_can_be_in_the_middle() {
    let dir = TestDir::new();
    let d = dir.open(b"aa\nbb\ncc\ndd");
    assert_eq!(
        texts(&rows_from(&d, 0, 2, 0, 10, &opts(4, 1000))),
        vec!["aa", "bb"]
    );
    assert_eq!(
        texts(&rows_from(&d, 6, 10, 0, 10, &opts(4, 1000))),
        vec!["cc", "dd"]
    );
    assert!(rows_from(&d, 0, 0, 0, 10, &opts(4, 1000)).is_empty());
    assert!(
        rows_from(&d, 11, 3, 0, 10, &opts(4, 1000)).is_empty(),
        "top == len: nessuna riga"
    );
    assert!(rows_from(&d, 99, 3, 0, 10, &opts(4, 1000)).is_empty());
}

#[test]
fn empty_document_and_the_eof_row() {
    let dir = TestDir::new();
    let d = dir.open(b"");
    assert!(rows_from(&d, 0, 3, 0, 10, &opts(4, 1000)).is_empty());
    let mut o = opts(4, 1000);
    o.show_eof_row = true;
    let rows = rows_from(&d, 0, 3, 0, 10, &o);
    assert_eq!(texts(&rows), vec![""]);
    assert_eq!((rows[0].start, rows[0].end), (0, 0));
    let d = dir.open(b"ab\n");
    assert_eq!(
        texts(&rows_from(&d, 0, 3, 0, 10, &o)),
        vec!["ab", ""],
        "con l'a-capo finale c'e' la riga vuota per il cursore"
    );
    let d = dir.open(b"ab");
    assert_eq!(
        texts(&rows_from(&d, 0, 3, 0, 10, &o)),
        vec!["ab"],
        "senza a-capo finale non c'e'"
    );
    let d = dir.open(b"ab\n");
    assert_eq!(
        texts(&rows_from(&d, 0, 1, 0, 10, &o)),
        vec!["ab"],
        "la riga fantasma rispetta count"
    );
}

#[test]
fn tabs_expand_to_the_next_multiple() {
    let dir = TestDir::new();
    let d = dir.open(b"a\tb\n\tx");
    let r = rows_from(&d, 0, 5, 0, 20, &opts(4, 1000));
    assert_eq!(texts(&r), vec!["a   b", "    x"]);
    let r = rows_from(&d, 0, 5, 0, 20, &opts(8, 1000));
    assert_eq!(texts(&r), vec!["a       b", "        x"]);
    let r = rows_from(&d, 0, 1, 0, 20, &opts(1, 1000));
    assert_eq!(texts(&r), vec!["a b"]);
}

#[test]
fn control_characters_invalid_bytes_and_carriage_returns() {
    let dir = TestDir::new();
    let d = dir.open(b"a\x01b\x7f\nx\xffy\nab\r\ncd\ra\r");
    let r = rows_from(&d, 0, 9, 0, 20, &opts(4, 1000));
    assert_eq!(
        texts(&r),
        vec!["a\u{b7}b\u{b7}", "x\u{fffd}y", "ab", "cd\u{b7}a\u{b7}"],
        "controlli e DEL -> U+00B7; byte non UTF-8 -> U+FFFD; \\r prima di \\n nascosto; \\r altrove e in fondo al file visibile"
    );
    // il \r nascosto fa parte della riga (end esclude solo l'a-capo)
    assert_eq!((r[2].start, r[2].end), (9, 12));
}

#[test]
fn each_invalid_byte_is_one_cell() {
    let dir = TestDir::new();
    let d = dir.open(b"\xe6\x97\xe6\x97\xa5\xc3");
    // E6 97 (troncato) / E6 97 A5 = U+65E5 (valido, largo 2) / C3 (troncato in fondo)
    let r = rows_from(&d, 0, 1, 0, 20, &opts(4, 1000));
    assert_eq!(texts(&r), vec!["\u{fffd}\u{fffd}\u{65e5}\u{fffd}"]);
}

#[test]
fn wide_characters_and_the_window_edges() {
    let dir = TestDir::new();
    let d = dir.open("日本語x".as_bytes());
    let t = |h: usize, c: usize| rows_from(&d, 0, 1, h, c, &opts(4, 1000)).remove(0).text;
    assert_eq!(t(0, 4), "日本");
    assert_eq!(
        t(0, 3),
        "日 ",
        "un carattere largo che non entra intero lascia uno spazio"
    );
    assert_eq!(
        t(1, 3),
        " 本",
        "il carattere largo tagliato a sinistra lascia uno spazio"
    );
    assert_eq!(t(2, 4), "本語");
    assert_eq!(t(0, 20), "日本語x");
    assert_eq!(t(6, 5), "x");
    assert_eq!(t(7, 5), "");
}

#[test]
fn horizontal_scroll_flags() {
    let dir = TestDir::new();
    let d = dir.open(b"abcdef\n\nxy");
    let o = opts(4, 1000);
    let r = rows_from(&d, 0, 3, 0, 3, &o);
    assert_eq!(texts(&r), vec!["abc", "", "xy"]);
    assert!(!r[0].more_left && r[0].more_right);
    assert!(!r[1].more_left && !r[1].more_right);
    assert!(!r[2].more_left && !r[2].more_right);
    let r = rows_from(&d, 0, 3, 2, 3, &o);
    assert_eq!(texts(&r), vec!["cde", "", ""]);
    assert!(r[0].more_left && r[0].more_right);
    assert!(!r[1].more_left, "riga vuota: niente freccia");
    assert!(
        r[2].more_left && !r[2].more_right,
        "xy e' tutta a sinistra della finestra? no: 'xy' ha 2 celle, hscroll 2 -> finestra vuota"
    );
    let r = rows_from(&d, 0, 1, 3, 3, &o);
    assert_eq!(texts(&r), vec!["def"]);
    assert!(r[0].more_left && !r[0].more_right, "esattamente finita");
}

#[test]
fn long_lines_are_cut_into_rows() {
    let dir = TestDir::new();
    let d = dir.open(b"abcdefghij\nxy");
    let r = rows_from(&d, 0, 10, 0, 50, &opts(4, 4));
    assert_eq!(texts(&r), vec!["abcd", "efgh", "ij", "xy"]);
    assert_eq!(
        r.iter().map(|x| x.start).collect::<Vec<_>>(),
        vec![0, 4, 8, 11]
    );
    assert_eq!(
        r.iter().map(|x| x.end).collect::<Vec<_>>(),
        vec![4, 8, 10, 13]
    );
    assert_eq!(
        r.iter().map(|x| x.starts_line).collect::<Vec<_>>(),
        vec![true, false, false, true]
    );
    assert!(
        r[0].more_right && r[1].more_right,
        "una riga tagliata continua nella successiva"
    );
    assert!(!r[2].more_right);
}

#[test]
fn a_row_far_longer_than_the_window_reads_only_what_it_needs() {
    // 3 MiB su una sola riga: rows_from deve rispondere subito e giusto (non legge/decodifica tutta la riga)
    let dir = TestDir::new();
    let mut data = vec![b'a'; 3 * 1024 * 1024];
    data.extend_from_slice(b"\nz");
    let d = dir.open(&data);
    let t0 = std::time::Instant::now();
    let r = rows_from(&d, 0, 2, 100, 40, &opts(4, 8 * 1024 * 1024));
    assert!(t0.elapsed().as_secs() < 2);
    assert_eq!(r[0].text, "a".repeat(40));
    assert!(r[0].more_left && r[0].more_right);
    assert_eq!(r[0].end, 3 * 1024 * 1024, "end e' la vera fine della riga");
    // la seconda riga ("z") sta tutta a sinistra della finestra (hscroll 100): niente testo, ma il segno a sinistra
    assert_eq!(r[1].start, 3 * 1024 * 1024 + 1);
    assert_eq!(r[1].text, "");
    assert!(r[1].more_left && !r[1].more_right);
}

#[test]
fn col_of_and_offset_at_col_by_hand() {
    let dir = TestDir::new();
    let d = dir.open(b"a\tb\n\xe6\x97\xa5\xe6\x9c\xac\nq\xffr");
    let o = opts(4, 1000);
    // riga 0: 'a' @0, tab @1 (colonne 1-3), 'b' @2? no: b'a', b'\t', b'b' -> offset 0,1,2; fine riga offset 3
    assert_eq!(col_of(&d, 0, 0, &o), 0);
    assert_eq!(col_of(&d, 0, 1, &o), 1);
    assert_eq!(col_of(&d, 0, 2, &o), 4);
    assert_eq!(
        col_of(&d, 0, 3, &o),
        5,
        "il cursore a fine riga sta dopo l'ultimo carattere"
    );
    assert_eq!(offset_at_col(&d, 0, 0, &o), 0);
    assert_eq!(offset_at_col(&d, 0, 1, &o), 1);
    assert_eq!(
        offset_at_col(&d, 0, 3, &o),
        1,
        "una colonna dentro la tabulazione e' la tabulazione"
    );
    assert_eq!(offset_at_col(&d, 0, 4, &o), 2);
    assert_eq!(
        offset_at_col(&d, 0, 5, &o),
        3,
        "oltre la fine: fine riga (prima dell'a-capo)"
    );
    assert_eq!(offset_at_col(&d, 0, 99, &o), 3);
    // riga 1: due caratteri larghi (3 byte ciascuno)
    assert_eq!(col_of(&d, 4, 4, &o), 0);
    assert_eq!(col_of(&d, 4, 7, &o), 2);
    assert_eq!(col_of(&d, 4, 10, &o), 4);
    assert_eq!(
        offset_at_col(&d, 4, 1, &o),
        4,
        "la seconda cella del carattere largo e' il carattere"
    );
    assert_eq!(offset_at_col(&d, 4, 2, &o), 7);
    assert_eq!(offset_at_col(&d, 4, 4, &o), 10);
    // riga 2: byte non valido = una cella
    assert_eq!(col_of(&d, 11, 12, &o), 1);
    assert_eq!(col_of(&d, 11, 13, &o), 2);
    assert_eq!(offset_at_col(&d, 11, 2, &o), 13);
}

// ---------- casuale contro l'oracolo ----------

/// Oracolo ingenuo di una riga: per ogni carattere (offset nella riga, larghezza, glifo).
struct Cell {
    off: usize,
    width: usize,
    glyph: String,
}

fn decode_one(b: &[u8]) -> (char, usize) {
    // un carattere valido o un byte non valido (= U+FFFD, 1 byte)
    for n in 1..=4.min(b.len()) {
        if let Ok(s) = std::str::from_utf8(&b[..n]) {
            if let Some(c) = s.chars().next() {
                return (c, n);
            }
        }
    }
    ('\u{fffd}', 1)
}

fn o_cells(row: &[u8], tab: usize, followed_by_nl: bool) -> (Vec<Cell>, usize) {
    let mut cells = Vec::new();
    let mut col = 0usize;
    let mut i = 0usize;
    while i < row.len() {
        let (c, n) = decode_one(&row[i..]);
        let (w, g) = if c == '\t' {
            let w = tab - col % tab;
            (w, " ".repeat(w))
        } else if c == '\r' && i + 1 == row.len() && followed_by_nl {
            (0, String::new())
        } else if c.is_control() {
            (1, "\u{b7}".to_string())
        } else {
            (c.width().unwrap_or(0), c.to_string())
        };
        cells.push(Cell {
            off: i,
            width: w,
            glyph: g,
        });
        col += w;
        i += n;
    }
    (cells, col)
}

/// La finestra [h, h+cols) come la dice la specifica: caratteri interi, uno spazio per ogni cella di un carattere largo tagliato.
fn o_window(cells: &[Cell], h: usize, cols: usize) -> String {
    let mut out = String::new();
    let mut col = 0usize;
    for c in cells {
        let (a, b) = (col, col + c.width);
        col = b;
        if c.width == 0 {
            // larghezza zero: solo \r nascosto negli usi di questo test, niente da mostrare
            out.push_str(&c.glyph);
            continue;
        }
        if a >= h && b <= h + cols {
            out.push_str(&c.glyph);
        } else {
            let da = a.max(h);
            let fino = b.min(h + cols);
            if fino > da {
                out.push_str(&" ".repeat(fino - da));
            }
        }
    }
    out
}

fn random_row_bytes(r: &mut Lcg, n: usize) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..n {
        match r.below(14) {
            0 => v.push(b'\t'),
            1 => v.push(0x01),
            2 => v.push(0xff),
            3 => v.extend_from_slice("é".as_bytes()),
            4 => v.extend_from_slice("日".as_bytes()),
            5 => v.extend_from_slice("😀".as_bytes()),
            6 => v.push(b'\r'),
            _ => v.push(b'a' + r.below(5) as u8),
        }
    }
    v
}

#[test]
fn random_rows_agree_with_the_oracle() {
    let dir = TestDir::new();
    let mut r = Lcg(2026);
    for round in 0..60 {
        // un documento di qualche riga, con modifiche a pezzi
        let mut model: Vec<u8> = Vec::new();
        for _ in 0..(1 + r.below(6)) {
            let n = r.below(40) as usize;
            model.extend(random_row_bytes(&mut r, n));
            model.push(b'\n');
        }
        if r.below(2) == 0 {
            model.pop(); // senza a-capo finale
        }
        let mut d = dir.open(&model);
        for _ in 0..3 {
            let pos = r.below(model.len() as u64 + 1) as usize;
            let k = r.below(10) as usize;
            let ins = random_row_bytes(&mut r, k);
            d.insert(pos as u64, &ins).unwrap();
            model.splice(pos..pos, ins.iter().copied());
        }
        let tab = 1 + r.below(8) as usize;
        let o = opts(tab, DEFAULT_MAX_SCAN);
        let h = r.below(30) as usize;
        let cols = 1 + r.below(40) as usize;
        let rows = rows_from(&d, 0, 100, h, cols, &o);
        // righe attese dal modello
        let mut attese: Vec<(usize, usize, bool)> = Vec::new(); // (start, end, followed_by_nl)
        let mut s = 0usize;
        while s < model.len() {
            match model[s..].iter().position(|&b| b == b'\n') {
                Some(p) => {
                    attese.push((s, s + p, true));
                    s += p + 1;
                }
                None => {
                    attese.push((s, model.len(), false));
                    s = model.len();
                }
            }
        }
        assert_eq!(rows.len(), attese.len(), "round {round}: numero di righe");
        for (row, &(start, end, nl)) in rows.iter().zip(&attese) {
            let (cells, width) = o_cells(&model[start..end], tab, nl);
            assert_eq!(
                (row.start as usize, row.end as usize),
                (start, end),
                "round {round}"
            );
            assert_eq!(
                row.text,
                o_window(&cells, h, cols),
                "round {round} riga @{start} h {h} cols {cols} tab {tab}"
            );
            assert_eq!(
                row.more_left,
                h > 0 && width > 0,
                "round {round} more_left @{start}"
            );
            assert_eq!(
                row.more_right,
                width > h + cols,
                "round {round} more_right @{start}"
            );
            assert!(row.starts_line);
            // col_of / offset_at_col contro l'oracolo, su ogni offset della riga
            for c in &cells {
                let colonna: usize = cells
                    .iter()
                    .take_while(|x| x.off < c.off)
                    .map(|x| x.width)
                    .sum();
                assert_eq!(
                    col_of(&d, start as u64, (start + c.off) as u64, &o),
                    colonna,
                    "round {round} col_of"
                );
                if c.width > 0 {
                    for dentro in 0..c.width {
                        assert_eq!(
                            offset_at_col(&d, start as u64, (colonna + dentro) as u64, &o),
                            (start + c.off) as u64,
                            "round {round} offset_at_col({})",
                            colonna + dentro
                        );
                    }
                }
            }
            assert_eq!(
                col_of(&d, start as u64, end as u64, &o),
                width,
                "round {round}: col_of a fine riga"
            );
            assert_eq!(
                offset_at_col(&d, start as u64, width as u64, &o),
                end as u64
            );
            assert_eq!(
                offset_at_col(&d, start as u64, width as u64 + 50, &o),
                end as u64
            );
        }
    }
}
