//! Lo stile di fine riga (E29): `LineEnding` (riconoscimento, byte scritti da Invio, forma del documento) e
//! `Newline::Cr` (il `\r` da solo come a-capo: posizioni, indice delle righe, scansione, vista). Scritti da ARCH.
//! Gli attesi sono scritti a mano sui byte di prova; il modello dell'indice conta i `\r` con un ciclo semplice.

use mrp_core::encoding::Encoding;
use mrp_core::lineending::LineEnding;
use mrp_core::newline::Newline;
use mrp_core::scan::{next_row, row_end, row_start, Next};
use mrp_core::view::{rows_from, ViewOptions};
use mrp_core::{BuildOptions, Document, FileSource, LineIndex};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn tmp(data: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_eol_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, data).unwrap();
    p
}

struct Guard(PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn enc(nome: &str) -> Encoding {
    Encoding::by_name(nome).unwrap()
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
}

// ---------- riconoscimento ----------

#[test]
fn detect_in_a_one_byte_encoding() {
    use LineEnding::*;
    let casi: &[(&[u8], LineEnding)] = &[
        (b"a\nb\nc\n", Lf),
        (b"a\r\nb\r\nc\r\n", CrLf),
        (b"a\rb\rc\r", Cr),
        (b"a\rb\rc", Cr),
        (b"a\r\nb\nc\n", Lf),
        (b"a\r\nb\r\nc\n", CrLf),
        (b"a\r\nb\rc", CrLf),
        (b"a\rb\nc", Lf),
        (b"", Lf),
        (b"nessuna fine riga", Lf),
        // un \r all'ultimo byte potrebbe essere il primo di un \r\n tagliato: non conta
        (b"abc\r", Lf),
        (b"a\r\nb\r", CrLf),
        (b"\n\n\n", Lf),
        (b"\r\n", CrLf),
    ];
    for &(dati, atteso) in casi {
        for nome in ["utf-8", "windows-1252", "shift-jis", "gb18030"] {
            assert_eq!(
                LineEnding::detect(dati, enc(nome)),
                atteso,
                "{nome} {dati:?}"
            );
        }
    }
}

#[test]
fn detect_in_utf16_and_utf32_never_gives_a_lone_cr() {
    use LineEnding::*;
    let le16 = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(u16::to_le_bytes).collect() };
    let be32 = |s: &str| -> Vec<u8> { s.chars().flat_map(|c| (c as u32).to_be_bytes()).collect() };
    assert_eq!(
        LineEnding::detect(&le16("a\r\nb\r\nc\r\n"), enc("utf-16le")),
        CrLf
    );
    assert_eq!(LineEnding::detect(&le16("a\nb\nc\n"), enc("utf-16le")), Lf);
    // il CR da solo non esiste in UTF-16: vale LF (quindi niente a-capo riconosciuti, ma lo stile e' LF)
    assert_eq!(LineEnding::detect(&le16("a\rb\rc\r"), enc("utf-16le")), Lf);
    // U+0D0A sono i byte 0A 0D: non e' ne' un CR ne' un LF
    assert_eq!(
        LineEnding::detect(&le16("x\u{0D0A}\u{0D0A}y"), enc("utf-16le")),
        Lf
    );
    assert_eq!(
        LineEnding::detect(&be32("a\r\nb\r\n"), enc("utf-32be")),
        CrLf
    );
    assert_eq!(LineEnding::detect(&be32("a\r\nb\n"), enc("utf-32be")), Lf);
    // le unita' sfasate di 1 byte non contano: `0D 0A` dentro due unita' LE
    assert_eq!(
        LineEnding::detect(&[0x41, 0x0D, 0x0A, 0x00, 0x42, 0x00], enc("utf-16le")),
        Lf
    );
}

#[test]
fn bytes_written_by_enter() {
    let b = |nome: &str, e: LineEnding| e.bytes(enc(nome));
    assert_eq!(b("utf-8", LineEnding::Lf), b"\n");
    assert_eq!(b("utf-8", LineEnding::CrLf), b"\r\n");
    assert_eq!(b("utf-8", LineEnding::Cr), b"\r");
    assert_eq!(b("shift-jis", LineEnding::CrLf), b"\r\n");
    assert_eq!(b("windows-1252", LineEnding::Cr), b"\r");
    assert_eq!(b("utf-16le", LineEnding::CrLf), [0x0D, 0, 0x0A, 0]);
    assert_eq!(b("utf-16be", LineEnding::Lf), [0, 0x0A]);
    assert_eq!(
        b("utf-32be", LineEnding::CrLf),
        [0, 0, 0, 0x0D, 0, 0, 0, 0x0A]
    );
    // il CR da solo non c'e' a 16 e 32 bit: si scrive un LF
    assert_eq!(b("utf-16le", LineEnding::Cr), [0x0A, 0]);
    assert_eq!(b("utf-32le", LineEnding::Cr), [0x0A, 0, 0, 0]);
}

#[test]
fn document_form_and_labels() {
    assert_eq!(LineEnding::Cr.newline_for(enc("utf-8")), Newline::Cr);
    assert_eq!(LineEnding::Cr.newline_for(enc("shift-jis")), Newline::Cr);
    assert_eq!(LineEnding::Cr.newline_for(enc("utf-16le")), Newline::Le16);
    assert_eq!(LineEnding::Cr.newline_for(enc("utf-32be")), Newline::Be32);
    assert_eq!(LineEnding::CrLf.newline_for(enc("utf-8")), Newline::Byte);
    assert_eq!(LineEnding::CrLf.newline_for(enc("utf-16be")), Newline::Be16);
    assert_eq!(
        LineEnding::Lf.newline_for(enc("windows-1252")),
        Newline::Byte
    );
    assert_eq!(LineEnding::Cr.for_encoding(enc("utf-16le")), LineEnding::Lf);
    assert_eq!(LineEnding::Cr.for_encoding(enc("utf-8")), LineEnding::Cr);
    assert_eq!(
        LineEnding::CrLf.for_encoding(enc("utf-16le")),
        LineEnding::CrLf
    );
    assert_eq!(LineEnding::default(), LineEnding::Lf);
    assert_eq!(
        [LineEnding::Lf, LineEnding::CrLf, LineEnding::Cr].map(LineEnding::label),
        ["LF", "CRLF", "CR"]
    );
}

// ---------- Newline::Cr ----------

#[test]
fn cr_form_basics() {
    assert_eq!(Newline::Cr.unit(), 1);
    assert_eq!(Newline::Cr.bytes(), b"\r");
    assert!(Newline::Cr.is_newline(b"\r"));
    assert!(!Newline::Cr.is_newline(b"\n"));
    let h = b"a\rb\r\nc";
    assert_eq!(Newline::Cr.positions(h).collect::<Vec<_>>(), vec![1, 3]);
    assert_eq!(
        Newline::Cr.positions(h).rev().collect::<Vec<_>>(),
        vec![3, 1]
    );
    assert_eq!(Newline::Cr.count(h), 2);
    assert_eq!(Newline::Cr.find(h), Some(1));
    assert_eq!(Newline::Cr.rfind(h), Some(3));
    assert_eq!(Newline::Cr.find(b"a\nb"), None);
    assert!(Newline::Cr.ends_with_newline(b"ab\r"));
    assert!(!Newline::Cr.ends_with_newline(b"ab\r\n"));
    assert!(!Newline::Cr.ends_with_newline(b""));
    // la forma di sempre non guarda i \r
    assert_eq!(Newline::Byte.count(h), 1);
}

fn model_starts(dati: &[u8]) -> Vec<u64> {
    let mut v = Vec::new();
    if !dati.is_empty() {
        v.push(0);
    }
    for (i, &b) in dati.iter().enumerate() {
        if b == b'\r' && i + 1 < dati.len() {
            v.push(i as u64 + 1);
        }
    }
    v
}

#[test]
fn the_index_with_cr_matches_the_model() {
    let mut r = Lcg(5);
    let alfabeto = [b'a', b'b', b'\r', b'\r', b'\n', b' ', 0xE9];
    for round in 0..60 {
        let n = (r.next() % 80) as usize;
        let dati: Vec<u8> = (0..n)
            .map(|_| alfabeto[(r.next() % alfabeto.len() as u64) as usize])
            .collect();
        let g = Guard(tmp(&dati));
        let src = FileSource::open(&g.0).unwrap();
        let starts = model_starts(&dati);
        for chunk in [1usize, 2, 3, 7, 64, 4096] {
            for every in [1u32, 3, 256] {
                let opts = BuildOptions {
                    chunk_bytes: chunk,
                    checkpoint_every: every,
                };
                let c = AtomicBool::new(false);
                let p = AtomicU64::new(0);
                let idx = LineIndex::build_newline(&src, Newline::Cr, &opts, &c, &p).unwrap();
                assert_eq!(idx.newline(), Newline::Cr);
                let ctx = format!("giro {round} chunk={chunk} ogni={every} {dati:?}");
                assert_eq!(idx.line_count(), starts.len() as u64, "{ctx}");
                for (k, &s) in starts.iter().enumerate() {
                    assert_eq!(
                        idx.offset_of_line(&src, k as u64),
                        Some(s),
                        "{ctx} riga {k}"
                    );
                }
                for o in 0..dati.len() as u64 {
                    let atteso = (starts.partition_point(|&s| s <= o) - 1) as u64;
                    assert_eq!(
                        idx.line_at_offset(&src, o),
                        Some(atteso),
                        "{ctx} offset {o}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_cr_only_document_scans_and_shows_rows() {
    let g = Guard(tmp(b"ab\rcde\r\rf"));
    let mut d = Document::open(&g.0).unwrap();
    assert!(d.set_newline(Newline::Cr));
    assert_eq!(d.newline(), Newline::Cr);
    // scansione: righe a 0, 3, 7, 8
    assert_eq!(next_row(&d, 0, 100), Next::Line(3));
    assert_eq!(next_row(&d, 3, 100), Next::Line(7));
    assert_eq!(next_row(&d, 7, 100), Next::Line(8));
    assert_eq!(next_row(&d, 8, 100), Next::End);
    assert_eq!(row_end(&d, 3, 100), 6);
    assert_eq!(row_start(&d, 5, 100), (3, true));
    // vista: lo stesso, senza mostrare i \r
    let o = ViewOptions::default();
    let r = rows_from(&d, 0, 4, 0, 80, &o);
    let testo: Vec<&str> = r.iter().map(|x| x.text.as_str()).collect();
    assert_eq!(testo, vec!["ab", "cde", "", "f"]);
    assert_eq!(
        r.iter().map(|x| (x.start, x.end)).collect::<Vec<_>>(),
        vec![(0, 2), (3, 6), (7, 7), (8, 9)]
    );
    // la forma di Cr si cambia come le altre, senza storia
    assert!(d.set_newline(Newline::Byte));
    assert_eq!(rows_from(&d, 0, 4, 0, 80, &o).len(), 1);
}

#[test]
fn crlf_files_keep_working_with_the_byte_form() {
    // un file CRLF: la riga finisce al \n e il \r e' nascosto
    let g = Guard(tmp(b"ab\r\ncd\r\n"));
    let d = Document::open(&g.0).unwrap();
    let r = rows_from(&d, 0, 4, 0, 80, &ViewOptions::default());
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].text, "ab");
    assert_eq!((r[0].start, r[0].end), (0, 3));
    assert_eq!(r[1].text, "cd");
}
