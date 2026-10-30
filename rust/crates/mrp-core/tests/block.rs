//! Test della modalita' colonna del core (E16). Scritti da ARCH prima dell'implementazione. L'oracolo e' ingenuo e
//! indipendente: spezza il testo (un `String`) in righe, calcola per ogni carattere colonna di inizio e larghezza con una
//! tabella di tre casi (tabulazione fino al multiplo di 4, 'caratteri larghi' = U+3040..U+9FFF largo 2, tutto il resto
//! largo 1) e applica la regola dei confini del modulo. I test usano solo testo valido UTF-8.

use mrp_core::block::{
    block_spans, block_text, column_block, delete_block, insert_block, nth_row, paste_block,
    BlockError, ColumnBlock, RowSpan,
};
use mrp_core::view::ViewOptions;
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_block_test_{}_{}.txt", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
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

fn opts() -> ViewOptions {
    ViewOptions {
        tab_width: 4,
        ..ViewOptions::default()
    }
}

/// Larghezza di un carattere alla colonna `col` (oracolo).
fn width(c: char, col: u64) -> u64 {
    if c == '\t' {
        4 - col % 4
    } else if ('\u{3040}'..='\u{9FFF}').contains(&c) {
        2
    } else {
        1
    }
}

/// Una riga dell'oracolo: i caratteri con (byte di inizio nel documento, lunghezza in byte, colonna di inizio,
/// larghezza), il byte di inizio riga, la fine "da blocco" (prima di `\n` e di un `\r` nascosto) e la larghezza.
struct NRow {
    start: u64,
    end: u64,
    width: u64,
    chars: Vec<(u64, u64, u64, u64)>,
}

/// Le righe del testo come le camminerebbe `scan::next_row` con `max_scan` enorme: una per `\n`, e l'ultima solo se
/// non vuota. Un `\r` subito prima di un `\n` e' nascosto (larghezza 0, fuori dal blocco).
fn naive_rows(text: &str) -> Vec<NRow> {
    let mut rows = Vec::new();
    let mut pos = 0u64;
    let mut resto = text;
    while !resto.is_empty() {
        let (riga, tolto, ha_nl) = match resto.find('\n') {
            Some(i) => (&resto[..i], i + 1, true),
            None => (resto, resto.len(), false),
        };
        let mut corpo = riga;
        if ha_nl && corpo.ends_with('\r') {
            corpo = &corpo[..corpo.len() - 1];
        }
        let mut chars = Vec::new();
        let mut col = 0u64;
        let mut off = pos;
        for c in corpo.chars() {
            let w = width(c, col);
            chars.push((off, c.len_utf8() as u64, col, w));
            col += w;
            off += c.len_utf8() as u64;
        }
        rows.push(NRow {
            start: pos,
            end: pos + corpo.len() as u64,
            width: col,
            chars,
        });
        pos += tolto as u64;
        resto = &resto[tolto..];
    }
    rows
}

/// Cio' che il blocco copre su una riga dell'oracolo.
fn naive_span(r: &NRow, col_a: u64, col_b: u64) -> RowSpan {
    let start = r.chars.iter().find(|c| c.2 >= col_a).map_or(r.end, |c| c.0);
    let end = r.chars.iter().find(|c| c.2 >= col_b).map_or(r.end, |c| c.0);
    RowSpan {
        row_start: r.start,
        start,
        end,
        width: r.width,
        reaches: col_a <= r.width,
    }
}

/// Tutti gli span attesi per un blocco dato da offset di riga e colonne.
fn naive_spans(text: &str, first: u64, last: u64, col_a: u64, col_b: u64) -> Vec<RowSpan> {
    naive_rows(text)
        .iter()
        .filter(|r| r.start >= first && r.start <= last)
        .map(|r| naive_span(r, col_a, col_b))
        .collect()
}

/// Colonna (oracolo) del byte `off`, confine di carattere, nella sua riga.
fn naive_col(text: &str, off: u64) -> (u64, u64) {
    let rows = naive_rows(text);
    // l'ultima riga che inizia a `off` o prima
    let r = rows
        .iter()
        .rev()
        .find(|r| r.start <= off)
        .expect("offset in una riga");
    let col = r.chars.iter().find(|c| c.0 >= off).map_or(r.width, |c| c.2);
    (r.start, col)
}

fn open(text: &str) -> (TempFile, Document) {
    let f = TempFile::new(text.as_bytes());
    let d = Document::open(&f.0).unwrap();
    (f, d)
}

fn whole(d: &Document) -> String {
    String::from_utf8(d.read_range(0, u64::MAX)).unwrap()
}

/// Testo di prova: ASCII, tabulazioni, 'e' accentata, caratteri larghi, righe corte e vuote, nessun `\n` finale.
const SAMPLE: &str = "abcdefghij\n\tx\u{e9}z\n\u{65e5}\u{672c}\u{8a9e}abc\n\nshort\nwide\u{65e5}\u{672c}end\n0123456789";

// ---------- esempi a mano ----------

#[test]
fn spans_of_a_plain_ascii_block_match_hand_computed_values() {
    // righe: "abcdefghij" (0..10), "klmnop" (11..17), "qr" (18..20); blocco colonne [3,6) sulle tre righe
    let (_f, d) = open("abcdefghij\nklmnop\nqr");
    let b = ColumnBlock {
        first_row: 0,
        last_row: 18,
        col_a: 3,
        col_b: 6,
    };
    let s = block_spans(&d, &b, &opts(), 100).unwrap();
    assert_eq!(
        s,
        vec![
            RowSpan {
                row_start: 0,
                start: 3,
                end: 6,
                width: 10,
                reaches: true
            },
            RowSpan {
                row_start: 11,
                start: 14,
                end: 17,
                width: 6,
                reaches: true
            },
            // "qr" e' piu' corta di col_a: il blocco e' vuoto a fine riga e non "arriva"
            RowSpan {
                row_start: 18,
                start: 20,
                end: 20,
                width: 2,
                reaches: false
            },
        ]
    );
    assert_eq!(block_text(&d, &s, 1000).unwrap(), b"def\nnop\n".to_vec());
}

#[test]
fn a_row_exactly_as_wide_as_col_a_reaches_with_an_empty_span_at_its_end() {
    let (_f, d) = open("abc\nabcd");
    let b = ColumnBlock {
        first_row: 0,
        last_row: 4,
        col_a: 3,
        col_b: 6,
    };
    let s = block_spans(&d, &b, &opts(), 100).unwrap();
    assert_eq!((s[0].start, s[0].end, s[0].reaches), (3, 3, true));
    assert_eq!((s[1].start, s[1].end, s[1].reaches), (7, 8, true));
    assert_eq!(block_text(&d, &s, 100).unwrap(), b"\nd".to_vec());
}

#[test]
fn tabs_and_wide_chars_follow_the_start_column_rule() {
    // riga "\tx<e'>z": tab colonne 0..4, x a 4, e' a 5, z a 6.   riga "<nichi><hon><go>abc": 0,2,4 poi a 6,7,8
    let (_f, d) = open("\txéz\n日本語abc");
    let la = "\txéz\n".len() as u64;
    // [1,5): il tab inizia a 0 (< 1) quindi e' fuori; x (inizia a 4) e' dentro; e' (5) fuori
    let b = ColumnBlock {
        first_row: 0,
        last_row: la,
        col_a: 1,
        col_b: 5,
    };
    let s = block_spans(&d, &b, &opts(), 10).unwrap();
    // tab = byte 0, x = byte 1, e' = byte 2..4, z = byte 4
    assert_eq!((s[0].start, s[0].end), (1, 2));
    // seconda riga: caratteri a colonne 0,2,4 (3 byte ciascuno), a=6; [1,5) prende il 2o e il 3o carattere largo
    assert_eq!((s[1].start - la, s[1].end - la), (3, 9));
    assert_eq!(block_text(&d, &s, 100).unwrap(), "x\n本語".as_bytes());
    // [3,3): punto di inserimento dentro il primo carattere largo (colonne 0..2)? no: colonna 3 e' dentro il
    // secondo (colonne 2..4); il primo carattere con inizio >= 3 e' il terzo (colonna 4)
    let p = ColumnBlock {
        first_row: la,
        last_row: la,
        col_a: 3,
        col_b: 3,
    };
    let s = block_spans(&d, &p, &opts(), 10).unwrap();
    assert_eq!((s[0].start - la, s[0].end - la), (6, 6));
    assert!(s[0].reaches);
}

#[test]
fn crlf_keeps_the_hidden_cr_out_of_the_block() {
    let (_f, d) = open("ab\r\ncd\r\nef");
    let b = ColumnBlock {
        first_row: 0,
        last_row: 8,
        col_a: 1,
        col_b: 9,
    };
    let s = block_spans(&d, &b, &opts(), 10).unwrap();
    // "ab\r": il \r e' nascosto, la riga finisce a 2; larghezza 2
    assert_eq!((s[0].start, s[0].end, s[0].width), (1, 2, 2));
    assert_eq!((s[1].start, s[1].end, s[1].width), (5, 6, 2));
    assert_eq!((s[2].start, s[2].end, s[2].width), (9, 10, 2));
    assert_eq!(block_text(&d, &s, 100).unwrap(), b"b\nd\nf".to_vec());
}

#[test]
fn delete_insert_and_paste_by_hand() {
    let (_f, mut d) = open("abcdef\nghijkl\nmn");
    let b = ColumnBlock {
        first_row: 0,
        last_row: 14,
        col_a: 2,
        col_b: 4,
    };
    let s = block_spans(&d, &b, &opts(), 10).unwrap();
    assert_eq!(delete_block(&mut d, &s).unwrap(), 4);
    assert_eq!(whole(&d), "abef\nghkl\nmn");
    // dopo la cancellazione il blocco e' largo zero a colonna 2
    let b0 = ColumnBlock { col_b: 2, ..b };
    let s = block_spans(&d, &b0, &opts(), 10).unwrap();
    assert_eq!(insert_block(&mut d, &s, 2, b"-", false).unwrap(), 3);
    assert_eq!(whole(&d), "ab-ef\ngh-kl\nmn-");
    // "mn" arriva esattamente alla colonna 2: ci si inserisce; ora una riga corta senza e con riempimento
    let (_f2, mut d2) = open("abcd\nx\nefgh");
    let b2 = ColumnBlock {
        first_row: 0,
        last_row: 7,
        col_a: 3,
        col_b: 3,
    };
    let s2 = block_spans(&d2, &b2, &opts(), 10).unwrap();
    assert_eq!(insert_block(&mut d2, &s2, 3, b"#", false).unwrap(), 2);
    assert_eq!(whole(&d2), "abc#d\nx\nefg#h");
    let (_f3, mut d3) = open("abcd\nx\nefgh");
    let s3 = block_spans(&d3, &b2, &opts(), 10).unwrap();
    assert_eq!(insert_block(&mut d3, &s3, 3, b"#", true).unwrap(), 3);
    assert_eq!(whole(&d3), "abc#d\nx  #\nefg#h");
    // incolla righe diverse
    let (_f4, mut d4) = open("0123\n4567\n89ab");
    let b4 = ColumnBlock {
        first_row: 0,
        last_row: 10,
        col_a: 1,
        col_b: 1,
    };
    let s4 = block_spans(&d4, &b4, &opts(), 10).unwrap();
    let righe: Vec<&[u8]> = vec![b"AA", b"B"];
    assert_eq!(paste_block(&mut d4, &s4, 1, &righe, false).unwrap(), 2);
    assert_eq!(whole(&d4), "0AA123\n4B567\n89ab");
}

#[test]
fn errors_change_nothing() {
    let (_f, mut d) = open("aaaa\nbbbb\ncccc");
    let b = ColumnBlock {
        first_row: 0,
        last_row: 10,
        col_a: 1,
        col_b: 3,
    };
    assert_eq!(
        block_spans(&d, &b, &opts(), 2),
        Err(BlockError::TooManyRows)
    );
    assert!(block_spans(&d, &b, &opts(), 3).is_ok());
    let s = block_spans(&d, &b, &opts(), 3).unwrap();
    assert_eq!(block_text(&d, &s, 7), Err(BlockError::TooManyBytes));
    assert_eq!(block_text(&d, &s, 8).unwrap(), b"aa\nbb\ncc".to_vec());
    let tre: Vec<&[u8]> = vec![b"1", b"2", b"3", b"4"];
    assert_eq!(
        paste_block(&mut d, &s, 1, &tre, false),
        Err(BlockError::NotEnoughRows)
    );
    assert_eq!(whole(&d), "aaaa\nbbbb\ncccc");
    assert!(!d.is_modified());
}

#[test]
fn nth_row_walks_rows_and_stops_at_the_end() {
    let (_f, d) = open("ab\ncd\nef");
    assert_eq!(nth_row(&d, 0, 0, &opts()), Some(0));
    assert_eq!(nth_row(&d, 0, 1, &opts()), Some(3));
    assert_eq!(nth_row(&d, 0, 2, &opts()), Some(6));
    assert_eq!(nth_row(&d, 0, 3, &opts()), None);
    assert_eq!(nth_row(&d, 3, 1, &opts()), Some(6));
    // una riga lunga tagliata da max_scan conta come piu' righe visuali
    let (_g, d2) = open("abcdefgh\nxy");
    let tagliata = ViewOptions {
        max_scan: 3,
        ..opts()
    };
    assert_eq!(nth_row(&d2, 0, 1, &tagliata), Some(3));
    assert_eq!(nth_row(&d2, 0, 3, &tagliata), Some(9));
}

#[test]
fn column_block_uses_the_rows_of_anchor_and_cursor_and_the_virtual_column() {
    let (_f, d) = open("abcdefghij\nklm\nnopqrs");
    let o = opts();
    // ancora in riga 0 colonna 6, cursore in riga 2 colonna 2: righe 0..=15, colonne [2,6)
    let b = column_block(&d, 6, 15 + 2, None, &o);
    assert_eq!(
        b,
        ColumnBlock {
            first_row: 0,
            last_row: 15,
            col_a: 2,
            col_b: 6
        }
    );
    // ordine invertito: stesso blocco
    assert_eq!(column_block(&d, 17, 6, None, &o), b);
    // colonna virtuale del cursore oltre la fine della riga corta (riga 1 ha larghezza 3)
    let v = column_block(&d, 2, 11 + 3, Some(8), &o);
    assert_eq!(
        v,
        ColumnBlock {
            first_row: 0,
            last_row: 11,
            col_a: 2,
            col_b: 8
        }
    );
    // ancora == cursore: larghezza zero, una riga
    let z = column_block(&d, 4, 4, None, &o);
    assert_eq!(
        z,
        ColumnBlock {
            first_row: 0,
            last_row: 0,
            col_a: 4,
            col_b: 4
        }
    );
}

// ---------- proprieta' contro l'oracolo ----------

/// Testo casuale di caratteri fra ASCII, tab, 'e' accentata e caratteri larghi, a righe di lunghezza varia; ogni tanto
/// una riga con `\r` prima del `\n`; l'ultima riga con o senza `\n`.
fn random_text(r: &mut Lcg) -> String {
    const ALFABETO: [char; 12] = [
        'a', 'b', 'c', 'd', 'e', 'f', '\t', '\u{e9}', '\u{65e5}', '\u{672c}', 'x', ' ',
    ];
    let righe = 1 + r.below(8);
    let mut s = String::new();
    for i in 0..righe {
        let n = r.below(14);
        for _ in 0..n {
            s.push(ALFABETO[r.below(ALFABETO.len() as u64) as usize]);
        }
        let ultima = i + 1 == righe;
        if !ultima || r.below(2) == 0 {
            if r.below(5) == 0 {
                s.push('\r');
            }
            s.push('\n');
        }
    }
    s
}

/// Offset (confine di carattere) casuale nel testo, anche `len`.
fn random_offset(r: &mut Lcg, text: &str) -> u64 {
    let confini: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect();
    confini[r.below(confini.len() as u64) as usize] as u64
}

#[test]
fn random_blocks_match_the_naive_oracle_for_spans_and_text() {
    let mut r = Lcg(2026);
    let o = opts();
    let mut visti_non_vuoti = 0;
    for caso in 0..400 {
        let text = random_text(&mut r);
        if text.is_empty() {
            continue;
        }
        let (_f, d) = open(&text);
        let righe = naive_rows(&text);
        if righe.is_empty() {
            continue;
        }
        // blocco fra due righe e due colonne casuali
        let a = righe[r.below(righe.len() as u64) as usize].start;
        let c = righe[r.below(righe.len() as u64) as usize].start;
        let (first, last) = (a.min(c), a.max(c));
        let ca = r.below(16);
        let cb = ca + r.below(8);
        let b = ColumnBlock {
            first_row: first,
            last_row: last,
            col_a: ca,
            col_b: cb,
        };
        let atteso = naive_spans(&text, first, last, ca, cb);
        let s = block_spans(&d, &b, &o, 1000).unwrap();
        assert_eq!(s, atteso, "caso {caso} {text:?} blocco {b:?}");
        // testo del blocco
        let mut t = Vec::new();
        for (i, sp) in atteso.iter().enumerate() {
            if i > 0 {
                t.push(b'\n');
            }
            t.extend_from_slice(&text.as_bytes()[sp.start as usize..sp.end as usize]);
        }
        assert_eq!(block_text(&d, &s, 1 << 20).unwrap(), t, "caso {caso}");
        if t.iter().any(|b| *b != b'\n') {
            visti_non_vuoti += 1;
        }
    }
    assert!(visti_non_vuoti > 100, "il test deve vedere blocchi veri");
}

#[test]
fn column_block_matches_the_oracle_columns() {
    let mut r = Lcg(77);
    let o = opts();
    for caso in 0..300 {
        let text = random_text(&mut r);
        if text.is_empty() {
            continue;
        }
        let (_f, d) = open(&text);
        let anchor = random_offset(&mut r, &text);
        let cursor = random_offset(&mut r, &text);
        // la riga di un offset a fine documento dopo un `\n` finale non esiste nell'oracolo: salta
        let (ra, ca) = naive_col(&text, anchor);
        let (rc, cc) = naive_col(&text, cursor);
        // a fine documento dopo un `\n` finale c'e' una riga fantasma che l'oracolo non ha
        let fantasma = |off: u64| off == text.len() as u64 && text.ends_with('\n');
        if fantasma(anchor) || fantasma(cursor) {
            continue;
        }
        let virt = if r.below(3) == 0 {
            Some(cc + r.below(5))
        } else {
            None
        };
        let colonna_cursore = virt.unwrap_or(cc);
        let atteso = ColumnBlock {
            first_row: ra.min(rc),
            last_row: ra.max(rc),
            col_a: ca.min(colonna_cursore),
            col_b: ca.max(colonna_cursore),
        };
        assert_eq!(
            column_block(&d, anchor, cursor, virt, &o),
            atteso,
            "caso {caso} {text:?} ancora {anchor} cursore {cursor}"
        );
    }
}

/// Applica a una copia del testo l'operazione `f` sugli span (dall'ultimo al primo) e restituisce il testo atteso.
fn naive_apply(
    text: &str,
    spans: &[RowSpan],
    f: impl Fn(usize, &RowSpan) -> Option<(usize, usize, Vec<u8>)>,
) -> String {
    let mut v = text.as_bytes().to_vec();
    for (i, sp) in spans.iter().enumerate().rev() {
        if let Some((da, a, nuovo)) = f(i, sp) {
            v.splice(da..a, nuovo);
        }
    }
    String::from_utf8(v).unwrap()
}

#[test]
fn random_delete_insert_and_paste_match_the_oracle_and_undo_restores() {
    let mut r = Lcg(4242);
    let o = opts();
    for caso in 0..300 {
        let text = random_text(&mut r);
        let righe = naive_rows(&text);
        if righe.is_empty() {
            continue;
        }
        let a = righe[r.below(righe.len() as u64) as usize].start;
        let c = righe[r.below(righe.len() as u64) as usize].start;
        let (first, last) = (a.min(c), a.max(c));
        let ca = r.below(14);
        let cb = ca + r.below(7);
        let b = ColumnBlock {
            first_row: first,
            last_row: last,
            col_a: ca,
            col_b: cb,
        };
        let spans = naive_spans(&text, first, last, ca, cb);

        // cancellazione
        let (_f, mut d) = open(&text);
        let s = block_spans(&d, &b, &o, 1000).unwrap();
        let tolti = delete_block(&mut d, &s).unwrap();
        let atteso = naive_apply(&text, &spans, |_, sp| {
            Some((sp.start as usize, sp.end as usize, vec![]))
        });
        assert_eq!(whole(&d), atteso, "cancella, caso {caso} {text:?} {b:?}");
        assert_eq!(tolti, spans.iter().map(|x| x.end - x.start).sum::<u64>());
        d.seal();
        if tolti > 0 {
            assert!(d.undo().is_some(), "una sola operazione, un solo passo");
            assert_eq!(whole(&d), text, "annulla, caso {caso}");
        }

        // inserimento con e senza riempimento
        for pad in [false, true] {
            let (_g, mut d) = open(&text);
            let bz = ColumnBlock { col_b: ca, ..b };
            let s = block_spans(&d, &bz, &o, 1000).unwrap();
            let n = insert_block(&mut d, &s, ca, b"<>", pad).unwrap();
            let spans_z = naive_spans(&text, first, last, ca, ca);
            let atteso = naive_apply(&text, &spans_z, |_, sp| {
                if sp.reaches {
                    Some((sp.start as usize, sp.start as usize, b"<>".to_vec()))
                } else if pad {
                    let mut nuovo = vec![b' '; (ca - sp.width) as usize];
                    nuovo.extend_from_slice(b"<>");
                    Some((sp.start as usize, sp.start as usize, nuovo))
                } else {
                    None
                }
            });
            assert_eq!(
                whole(&d),
                atteso,
                "inserisci pad={pad}, caso {caso} {text:?} {b:?}"
            );
            let modificate = spans_z.iter().filter(|x| x.reaches || pad).count() as u64;
            assert_eq!(n, modificate);
        }

        // incolla: tante righe quante ne ha il blocco (o meno)
        let (_h, mut d) = open(&text);
        let bz = ColumnBlock { col_b: ca, ..b };
        let s = block_spans(&d, &bz, &o, 1000).unwrap();
        let quante = 1 + r.below(s.len() as u64) as usize;
        let testi: Vec<Vec<u8>> = (0..quante).map(|i| format!("P{i}").into_bytes()).collect();
        let riferimenti: Vec<&[u8]> = testi.iter().map(|t| t.as_slice()).collect();
        let n = paste_block(&mut d, &s, ca, &riferimenti, false).unwrap();
        let spans_z = naive_spans(&text, first, last, ca, ca);
        let atteso = naive_apply(&text, &spans_z, |i, sp| {
            if i < quante && sp.reaches {
                Some((sp.start as usize, sp.start as usize, testi[i].clone()))
            } else {
                None
            }
        });
        assert_eq!(whole(&d), atteso, "incolla, caso {caso} {text:?} {b:?}");
        let fatte = spans_z.iter().take(quante).filter(|x| x.reaches).count() as u64;
        assert_eq!(n, fatte);
    }
}

#[test]
fn many_rows_stay_exact_and_fast_enough() {
    // 20.000 righe: cancella e reinserisci una colonna; vale come prova di costo (il test finisce in frazioni di secondo)
    let mut testo = String::new();
    for i in 0..20_000 {
        testo.push_str(&format!("{i:05}|abcdefghij|{}\n", i % 10));
    }
    let (_f, mut d) = open(&testo);
    let b = ColumnBlock {
        first_row: 0,
        // inizio dell'ultima riga (il testo finisce con `\n`)
        last_row: testo[..testo.len() - 1]
            .rfind('\n')
            .map_or(0, |i| i as u64 + 1),
        col_a: 6,
        col_b: 16,
    };
    let s = block_spans(&d, &b, &opts(), 100_000).unwrap();
    assert_eq!(s.len(), 20_000);
    assert_eq!(delete_block(&mut d, &s).unwrap(), 200_000);
    d.seal();
    let atteso: String = (0..20_000)
        .map(|i| format!("{i:05}||{}\n", i % 10))
        .collect();
    assert_eq!(whole(&d), atteso);
    let b0 = ColumnBlock { col_b: 6, ..b };
    let s = block_spans(&d, &b0, &opts(), 100_000).unwrap();
    assert_eq!(
        insert_block(&mut d, &s, 6, b"abcdefghij", false).unwrap(),
        20_000
    );
    assert_eq!(whole(&d), testo);
    // il documento e' tornato com'era ma resta "modificato" fino al salvataggio; un annulla per volta lo riporta indietro
    d.seal();
    assert!(d.undo().is_some());
    assert_eq!(whole(&d), atteso);
}

#[test]
fn sample_text_block_over_all_rows() {
    let (_f, d) = open(SAMPLE);
    let ultima = naive_rows(SAMPLE).last().unwrap().start;
    let b = ColumnBlock {
        first_row: 0,
        last_row: ultima,
        col_a: 2,
        col_b: 7,
    };
    let s = block_spans(&d, &b, &opts(), 100).unwrap();
    assert_eq!(s, naive_spans(SAMPLE, 0, ultima, 2, 7));
    assert_eq!(s.len(), 7);
}
