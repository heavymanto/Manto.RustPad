//! Il nucleo con l'a-capo a 16 bit (UTF-16 LE e BE): indice delle righe, documento con modifiche, scansione per righe
//! e ricerca. Scritti da ARCH prima dell'implementazione. Gli attesi vengono da un MODELLO scritto qui, che lavora sul
//! testo con `str::encode_utf16` (mai dal codice del nucleo): l'inizio di una riga e' la unita' dopo una unita' 0x000A.
//!
//! Le trappole volute: caratteri che contengono il byte 0x0A (U+010A, U+0A41, U+0A00) e coppie di unita' vicine che
//! formano `0A 00` (LE) o `00 0A` (BE) SFASATE di un byte (U+0A41 seguito da U+0100; U+4100 seguito da U+0A41).

use mrp_core::docsearch::SearchError;
use mrp_core::newline::Newline;
use mrp_core::scan::{next_row, prev_row_start, row_end, row_start, Next};
use mrp_core::{BuildOptions, Document, FileSource, LineIndex};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// ---------- attrezzi ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_u16_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, data: &[u8]) -> PathBuf {
        let p = self
            .0
            .join(format!("a{}.txt", COUNTER.fetch_add(1, Ordering::SeqCst)));
        std::fs::write(&p, data).unwrap();
        p
    }
    fn doc(&self, data: &[u8], nl: Newline, cap: usize) -> Document {
        let mut d = Document::open_with_chunk(&self.file(data), cap).unwrap();
        assert!(d.set_newline(nl));
        d
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn flags() -> (AtomicBool, AtomicU64) {
    (AtomicBool::new(false), AtomicU64::new(0))
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

#[derive(Clone, Copy, Debug)]
enum End {
    Le,
    Be,
}

const ENDS: [End; 2] = [End::Le, End::Be];

fn nl_of(e: End) -> Newline {
    match e {
        End::Le => Newline::Le16,
        End::Be => Newline::Be16,
    }
}

/// `s` in UTF-16 con l'endian `e`, con la libreria standard.
fn enc(e: End, s: &str) -> Vec<u8> {
    match e {
        End::Le => s.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        End::Be => s.encode_utf16().flat_map(u16::to_be_bytes).collect(),
    }
}

/// Le unita' intere di `bytes` (un byte finale spaiato non conta).
fn units(e: End, bytes: &[u8]) -> Vec<u16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| match e {
            End::Le => u16::from_le_bytes([c[0], c[1]]),
            End::Be => u16::from_be_bytes([c[0], c[1]]),
        })
        .collect()
}

const ALFABETO: [char; 14] = [
    'a',
    'b',
    ' ',
    '\n',
    '\n',
    '\r',
    'é',
    '\u{010A}',
    '\u{0A41}',
    '\u{0100}',
    '\u{4100}',
    '\u{0A00}',
    '\u{1F600}',
    '€',
];

fn random_text(r: &mut Lcg, n: usize) -> String {
    (0..n)
        .map(|_| ALFABETO[r.below(ALFABETO.len() as u64) as usize])
        .collect()
}

/// Modello: gli inizi di riga, in byte. Una riga comincia a 0 (se il documento non e' vuoto) e subito dopo ogni unita'
/// 0x000A, se la posizione e' ancora dentro il documento (un byte finale spaiato la rende un inizio).
fn model_starts(e: End, bytes: &[u8]) -> Vec<u64> {
    let mut v = Vec::new();
    if !bytes.is_empty() {
        v.push(0);
    }
    for (i, u) in units(e, bytes).iter().enumerate() {
        if *u == 0x000A && 2 * (i + 1) < bytes.len() {
            v.push(2 * (i as u64 + 1));
        }
    }
    v
}

fn model_line_at(starts: &[u64], o: u64) -> u64 {
    (starts.partition_point(|&s| s <= o) - 1) as u64
}

// ---------- indice delle righe ----------

fn check_index(idx: &LineIndex, src: &FileSource, bytes: &[u8], e: End, ctx: &str) {
    let starts = model_starts(e, bytes);
    assert_eq!(idx.line_count(), starts.len() as u64, "{ctx}: righe");
    for (k, &s) in starts.iter().enumerate() {
        assert_eq!(
            idx.offset_of_line(src, k as u64),
            Some(s),
            "{ctx}: riga {k}"
        );
    }
    assert_eq!(idx.offset_of_line(src, starts.len() as u64), None, "{ctx}");
    for o in 0..bytes.len() as u64 {
        assert_eq!(
            idx.line_at_offset(src, o),
            Some(model_line_at(&starts, o)),
            "{ctx}: offset {o}"
        );
    }
    assert_eq!(idx.line_at_offset(src, bytes.len() as u64), None, "{ctx}");
}

#[test]
fn the_index_matches_the_model_for_both_endians_and_odd_lengths() {
    let dir = TestDir::new();
    let mut r = Lcg(7);
    for round in 0..30 {
        let n = r.below(100) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            for dispari in [false, true] {
                let mut bytes = enc(e, &text);
                if dispari {
                    bytes.push(0x41);
                }
                let src = FileSource::open(&dir.file(&bytes)).unwrap();
                for chunk in [1usize, 2, 3, 4, 5, 8, 100, 4096] {
                    for every in [1u32, 2, 7, 256] {
                        let opts = BuildOptions {
                            chunk_bytes: chunk,
                            checkpoint_every: every,
                        };
                        let (c, p) = flags();
                        let idx = LineIndex::build_newline(&src, nl_of(e), &opts, &c, &p).unwrap();
                        assert_eq!(idx.newline(), nl_of(e));
                        assert_eq!(p.load(Ordering::Relaxed), bytes.len() as u64);
                        let ctx = format!(
                            "giro {round} {e:?} dispari={dispari} chunk={chunk} ogni={every}"
                        );
                        check_index(&idx, &src, &bytes, e, &ctx);
                    }
                }
            }
        }
    }
}

#[test]
fn the_index_with_byte_newlines_is_the_old_one() {
    let dir = TestDir::new();
    let data = b"uno\ndue\n\ntre\r\nquattro";
    let src = FileSource::open(&dir.file(data)).unwrap();
    let (c, p) = flags();
    let nuovo = LineIndex::build_for(&src, Newline::Byte, &c, &p).unwrap();
    let (c, p) = flags();
    let vecchio = LineIndex::build(&src, &c, &p).unwrap();
    assert_eq!(nuovo.newline(), Newline::Byte);
    assert_eq!(vecchio.newline(), Newline::Byte);
    assert_eq!(nuovo.line_count(), 5);
    assert_eq!(nuovo.line_count(), vecchio.line_count());
    for k in 0..5 {
        assert_eq!(
            nuovo.offset_of_line(&src, k),
            vecchio.offset_of_line(&src, k)
        );
    }
    assert_eq!(nuovo.offset_of_line(&src, 3), Some(9));
}

#[test]
fn an_empty_file_has_no_lines_and_cancel_still_wins() {
    let dir = TestDir::new();
    let src = FileSource::open(&dir.file(b"")).unwrap();
    let (c, p) = flags();
    let idx = LineIndex::build_for(&src, Newline::Le16, &c, &p).unwrap();
    assert_eq!(idx.line_count(), 0);
    assert_eq!(idx.newline(), Newline::Le16);
    c.store(true, Ordering::Relaxed);
    assert!(LineIndex::build_for(&src, Newline::Le16, &c, &p).is_err());
}

#[test]
fn known_utf16_lines_with_the_trap_characters() {
    // LE: "A" U+0A41 U+0100 "\n" "B": i byte 1-2 sono `0A 00` ma sfasati: NON e' un a-capo. L'unico a-capo e' dopo U+0100.
    let dir = TestDir::new();
    let bytes = enc(End::Le, "A\u{0A41}\u{0100}\nB");
    assert_eq!(bytes.len(), 10);
    let src = FileSource::open(&dir.file(&bytes)).unwrap();
    let (c, p) = flags();
    let idx = LineIndex::build_for(&src, Newline::Le16, &c, &p).unwrap();
    assert_eq!(idx.line_count(), 2);
    assert_eq!(idx.offset_of_line(&src, 1), Some(8));
    // BE: U+4100 U+0A41 "\n" "B"
    let bytes = enc(End::Be, "\u{4100}\u{0A41}\nB");
    let src = FileSource::open(&dir.file(&bytes)).unwrap();
    let idx = LineIndex::build_for(&src, Newline::Be16, &c, &p).unwrap();
    assert_eq!(idx.line_count(), 2);
    assert_eq!(idx.offset_of_line(&src, 1), Some(6));
    // la forma sbagliata trova un'altra cosa: gli stessi byte letti come Byte hanno piu' righe
    let (c, p) = flags();
    let sbagliato = LineIndex::build_for(&src, Newline::Byte, &c, &p).unwrap();
    assert_eq!(sbagliato.line_count(), 3);
}

// ---------- documento ----------

fn check_doc(doc: &mut Document, model: &[u8], e: End, ctx: &str) {
    assert_eq!(doc.len(), model.len() as u64, "{ctx}");
    assert_eq!(doc.read_range(0, model.len() as u64), model, "{ctx}");
    let starts = model_starts(e, model);
    assert_eq!(doc.line_count(), Some(starts.len() as u64), "{ctx}: righe");
    for (k, &s) in starts.iter().enumerate() {
        assert_eq!(doc.offset_of_line(k as u64), Some(s), "{ctx}: riga {k}");
    }
    assert_eq!(doc.offset_of_line(starts.len() as u64), None, "{ctx}");
    for o in 0..model.len() as u64 {
        assert_eq!(
            doc.line_at_offset(o),
            Some(model_line_at(&starts, o)),
            "{ctx}: offset {o}"
        );
    }
    assert_eq!(doc.line_at_offset(model.len() as u64), None, "{ctx}");
}

fn build(doc: &mut Document, chunk: usize, every: u32) {
    let (c, p) = flags();
    let opts = BuildOptions {
        chunk_bytes: chunk,
        checkpoint_every: every,
    };
    doc.build_lines_with(&opts, &c, &p).unwrap();
}

#[test]
fn the_document_lines_match_the_model_without_edits_odd_tail_included() {
    let dir = TestDir::new();
    let mut r = Lcg(11);
    for _ in 0..25 {
        let n = r.below(80) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            for dispari in [false, true] {
                let mut bytes = enc(e, &text);
                if dispari {
                    bytes.push(0x7A);
                }
                let mut d = dir.doc(&bytes, nl_of(e), 3);
                assert_eq!(d.newline(), nl_of(e));
                build(&mut d, 6, 2);
                check_doc(&mut d, &bytes, e, &format!("{e:?} dispari={dispari}"));
            }
        }
    }
}

#[test]
fn the_document_lines_follow_aligned_edits() {
    let dir = TestDir::new();
    let mut r = Lcg(99);
    for round in 0..20 {
        let n = r.below(60) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            for cap in [2usize, 3, 8] {
                let mut model = enc(e, &text);
                let mut d = dir.doc(&model, nl_of(e), cap);
                build(&mut d, 4, 2);
                check_doc(
                    &mut d,
                    &model,
                    e,
                    &format!("giro {round} {e:?} cap={cap} inizio"),
                );
                for passo in 0..25 {
                    let len = model.len();
                    if r.below(3) != 0 || len == 0 {
                        // inserisce a una posizione pari
                        let pos = 2 * r.below(len as u64 / 2 + 1) as usize;
                        let k = r.below(6) as usize;
                        let dati = enc(e, &random_text(&mut r, k));
                        d.insert(pos as u64, &dati).unwrap();
                        model.splice(pos..pos, dati);
                    } else {
                        // cancella un tratto pari, da una posizione pari
                        let pos = 2 * r.below(len as u64 / 2) as usize;
                        let max = (len - pos) / 2;
                        let quanto = 2 * (r.below(max as u64 + 1) as usize).min(5);
                        d.delete(pos as u64, quanto as u64).unwrap();
                        model.drain(pos..pos + quanto);
                    }
                    if r.below(4) == 0 {
                        d.seal();
                    }
                    let ctx = format!("giro {round} {e:?} cap={cap} passo {passo}");
                    check_doc(&mut d, &model, e, &ctx);
                }
                // annulla tutto: si torna al testo di partenza
                while d.undo().is_some() {}
                let ctx = format!("giro {round} {e:?} cap={cap} dopo undo");
                check_doc(&mut d, &enc(e, &text), e, &ctx);
            }
        }
    }
}

#[test]
fn set_newline_rules() {
    let dir = TestDir::new();
    let bytes = enc(End::Le, "a\nb\n");
    let mut d = Document::open(&dir.file(&bytes)).unwrap();
    assert_eq!(d.newline(), Newline::Byte);
    assert!(d.set_newline(Newline::Byte));
    build(&mut d, 4, 2);
    assert!(d.has_lines());
    // stessa forma: nulla cambia, l'indice resta
    assert!(d.set_newline(Newline::Byte));
    assert!(d.has_lines());
    // forma diversa: cambia, l'indice si butta
    assert!(d.set_newline(Newline::Le16));
    assert_eq!(d.newline(), Newline::Le16);
    assert!(!d.has_lines());
    // un indice costruito per un'altra forma non si installa
    let src = FileSource::open(&dir.file(&bytes)).unwrap();
    let (c, p) = flags();
    d.install_lines(LineIndex::build_for(&src, Newline::Byte, &c, &p).unwrap());
    assert!(!d.has_lines());
    d.install_lines(LineIndex::build_for(&src, Newline::Be16, &c, &p).unwrap());
    assert!(!d.has_lines());
    // uno della forma giusta si
    d.install_lines(LineIndex::build_for(&src, Newline::Le16, &c, &p).unwrap());
    assert!(d.has_lines());
    assert_eq!(d.line_count(), Some(2));
    // con una modifica la forma non si cambia piu'
    d.insert(0, &[0x61, 0x00]).unwrap();
    assert!(!d.set_newline(Newline::Be16));
    assert_eq!(d.newline(), Newline::Le16);
    assert!(d.has_lines());
    assert!(d.set_newline(Newline::Le16));
    // annullata, c'e' ancora qualcosa da ripetere: ancora no
    d.undo();
    assert!(!d.is_modified());
    assert!(!d.set_newline(Newline::Be16));
    assert_eq!(d.newline(), Newline::Le16);
    // ripetuta e riannullata fino in fondo, la storia resta: ancora no
    assert!(d.redo().is_some());
    assert!(!d.set_newline(Newline::Byte));
}

// ---------- scansione per righe ----------

#[derive(Debug, PartialEq)]
enum N {
    Line(usize),
    Cut(usize),
    End,
}

fn m_primo(u: &[u16], da: usize, fine: usize) -> Option<usize> {
    (da..fine.min(u.len())).find(|&i| u[i] == 0x000A)
}

fn m_ultimo(u: &[u16], da: usize, fine: usize) -> Option<usize> {
    (da..fine.min(u.len())).rev().find(|&i| u[i] == 0x000A)
}

/// Le stesse regole di `scan.rs` ma in unita': `pos` e `max` sono indici di unita' (`max >= 1`).
fn m_next(u: &[u16], pos: usize, max: usize) -> N {
    let len = u.len();
    if pos >= len {
        return N::End;
    }
    let fine = len.min(pos + max);
    match m_primo(u, pos, fine) {
        Some(i) if i + 1 < len => N::Line(i + 1),
        Some(_) => N::End,
        None if pos + max < len => N::Cut(pos + max),
        None => N::End,
    }
}

fn m_end(u: &[u16], pos: usize, max: usize) -> usize {
    let len = u.len();
    let fine = len.min(pos + max);
    let inizio = pos.min(len);
    m_primo(u, inizio, fine).unwrap_or(fine.max(inizio))
}

fn m_start(u: &[u16], pos: usize, max: usize) -> (usize, bool) {
    let pos = pos.min(u.len());
    let da = pos.saturating_sub(max);
    match m_ultimo(u, da, pos) {
        Some(i) => (i + 1, true),
        None if pos <= max => (0, true),
        None => (pos - max, false),
    }
}

fn m_prev(u: &[u16], pos: usize, max: usize) -> Option<usize> {
    let pos = pos.min(u.len());
    if pos == 0 {
        None
    } else {
        Some(m_start(u, pos - 1, max).0)
    }
}

#[test]
fn row_scanning_matches_the_unit_model() {
    let dir = TestDir::new();
    let mut r = Lcg(5);
    for round in 0..25 {
        let n = r.below(70) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            let bytes = enc(e, &text);
            let d = dir.doc(&bytes, nl_of(e), 3);
            let u = units(e, &bytes);
            // max_scan in byte: il nucleo lo porta a un multiplo di 2, almeno 2
            for max_scan in [0u64, 1, 2, 3, 4, 5, 6, 7, 8, 16, 33, 1000] {
                let max = ((max_scan / 2) as usize).max(1);
                for p in 0..=u.len() + 2 {
                    let pb = 2 * p as u64;
                    let ctx = format!("giro {round} {e:?} max_scan={max_scan} pos={pb}");
                    let atteso = match m_next(&u, p, max) {
                        N::Line(i) => Next::Line(2 * i as u64),
                        N::Cut(i) => Next::Cut(2 * i as u64),
                        N::End => Next::End,
                    };
                    assert_eq!(next_row(&d, pb, max_scan), atteso, "next_row {ctx}");
                    assert_eq!(
                        row_end(&d, pb, max_scan),
                        2 * m_end(&u, p, max) as u64,
                        "row_end {ctx}"
                    );
                    let (s, vera) = m_start(&u, p, max);
                    assert_eq!(
                        row_start(&d, pb, max_scan),
                        (2 * s as u64, vera),
                        "row_start {ctx}"
                    );
                    assert_eq!(
                        prev_row_start(&d, pb, max_scan),
                        m_prev(&u, p, max).map(|i| 2 * i as u64),
                        "prev_row_start {ctx}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_scanning_with_an_odd_tail_byte() {
    let dir = TestDir::new();
    // LE: "a\n" e poi un byte spaiato 'b' (0x62)
    let d = dir.doc(&[0x61, 0x00, 0x0A, 0x00, 0x62], Newline::Le16, 3);
    assert_eq!(next_row(&d, 0, 100), Next::Line(4));
    assert_eq!(row_end(&d, 0, 100), 2);
    assert_eq!(row_end(&d, 4, 100), 5);
    assert_eq!(next_row(&d, 4, 100), Next::End);
    assert_eq!(row_start(&d, 5, 100), (4, true));
    assert_eq!(prev_row_start(&d, 4, 100), Some(0));
}

#[test]
fn byte_documents_scan_as_before() {
    let dir = TestDir::new();
    let d = Document::open(&dir.file(b"ab\ncd\n")).unwrap();
    assert_eq!(next_row(&d, 0, 100), Next::Line(3));
    assert_eq!(row_end(&d, 3, 100), 5);
    assert_eq!(row_start(&d, 4, 100), (3, true));
    assert_eq!(prev_row_start(&d, 3, 100), Some(0));
    assert_eq!(next_row(&d, 3, 100), Next::End);
}

// ---------- ricerca ----------

/// Modello: la prima posizione `s >= from`, multipla di `align`, dove `needle` sta per intero.
fn m_find_next(bytes: &[u8], needle: &[u8], from: u64, align: usize) -> Option<u64> {
    let n = needle.len();
    if n == 0 || bytes.len() < n {
        return None;
    }
    (from as usize..=bytes.len() - n)
        .find(|&s| s % align == 0 && &bytes[s..s + n] == needle)
        .map(|s| s as u64)
}

/// Modello: l'ultima posizione `s < before`, multipla di `align`, dove `needle` sta per intero.
fn m_find_prev(bytes: &[u8], needle: &[u8], before: u64, align: usize) -> Option<u64> {
    let n = needle.len();
    if n == 0 || bytes.len() < n {
        return None;
    }
    let fino = (before as usize).min(bytes.len() - n + 1);
    (0..fino)
        .rev()
        .find(|&s| s % align == 0 && &bytes[s..s + n] == needle)
        .map(|s| s as u64)
}

#[test]
fn search_ignores_misaligned_matches() {
    let dir = TestDir::new();
    let (c, p) = flags();
    // LE: 'A' = `41 00`; "U+4100 U+4200" sono `00 41 00 42`: `41 00` compare al byte 1, sfasato
    let d = dir.doc(&enc(End::Le, "\u{4100}\u{4200}"), Newline::Le16, 3);
    let s = d.snapshot().unwrap();
    assert_eq!(s.find_next(&enc(End::Le, "A"), 0, &c, &p).unwrap(), None);
    assert_eq!(s.find_prev(&enc(End::Le, "A"), 100, &c, &p).unwrap(), None);
    let d = dir.doc(&enc(End::Le, "\u{4100}\u{4200}A"), Newline::Le16, 3);
    let s = d.snapshot().unwrap();
    assert_eq!(s.find_next(&enc(End::Le, "A"), 0, &c, &p).unwrap(), Some(4));
    assert_eq!(
        s.find_prev(&enc(End::Le, "A"), 100, &c, &p).unwrap(),
        Some(4)
    );
    // BE: 'A' = `00 41`; "U+4100 U+4142" sono `41 00 41 42`: `00 41` al byte 1
    let d = dir.doc(&enc(End::Be, "\u{4100}\u{4142}"), Newline::Be16, 3);
    let s = d.snapshot().unwrap();
    assert_eq!(s.find_next(&enc(End::Be, "A"), 0, &c, &p).unwrap(), None);
    assert_eq!(s.find_prev(&enc(End::Be, "A"), 100, &c, &p).unwrap(), None);
    // un documento a byte la trova invece: l'allineamento e' solo per UTF-16
    let b = Document::open(&dir.file(&[0x00, 0x41, 0x00, 0x42])).unwrap();
    let s = b.snapshot().unwrap();
    assert_eq!(s.find_next(&[0x41, 0x00], 0, &c, &p).unwrap(), Some(1));
}

#[test]
fn search_matches_the_aligned_model_for_every_start_and_chunk() {
    let dir = TestDir::new();
    let mut r = Lcg(31);
    let (c, p) = flags();
    for round in 0..12 {
        let n = 1 + r.below(30) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            let bytes = enc(e, &text);
            let mut d = dir.doc(&bytes, nl_of(e), 3);
            // anche con una modifica allineata, perche' la fotografia legge dai pezzi
            let mut model = bytes.clone();
            let pos = 2 * r.below(model.len() as u64 / 2 + 1) as usize;
            let extra = enc(e, "A\u{0A41}\u{0100}");
            d.insert(pos as u64, &extra).unwrap();
            model.splice(pos..pos, extra);
            let aghi: Vec<Vec<u8>> = vec![
                enc(e, "A"),
                enc(e, "a"),
                enc(e, "\n"),
                enc(e, "\u{0A41}\u{0100}"),
                enc(e, "ab"),
                enc(e, &random_text(&mut r, 2)),
                vec![0x41],
                vec![0x00, 0x0A, 0x00],
            ];
            for ago in &aghi {
                for chunk in [1usize, 2, 3, 5, 64] {
                    let s = d.snapshot().unwrap().with_chunk(chunk);
                    for da in 0..=model.len() as u64 + 1 {
                        let ctx = format!("giro {round} {e:?} ago={ago:?} chunk={chunk} da={da}");
                        assert_eq!(
                            s.find_next(ago, da, &c, &p).unwrap(),
                            m_find_next(&model, ago, da, 2),
                            "find_next {ctx}"
                        );
                        assert_eq!(
                            s.find_prev(ago, da, &c, &p).unwrap(),
                            m_find_prev(&model, ago, da, 2),
                            "find_prev {ctx}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn search_cancel_is_still_reported() {
    let dir = TestDir::new();
    let d = dir.doc(&enc(End::Le, "abc"), Newline::Le16, 3);
    let s = d.snapshot().unwrap();
    let (c, p) = flags();
    c.store(true, Ordering::Relaxed);
    assert!(matches!(
        s.find_next(&enc(End::Le, "b"), 0, &c, &p),
        Err(SearchError::Cancelled)
    ));
}

// ---------- casi aggiunti dopo le mutazioni ----------

#[test]
fn blocks_made_of_several_read_segments_keep_the_line_starts() {
    // Blocchi di 600000 byte, letti a segmenti di 256 KiB (262144 byte): gli a-capo cadono proprio sui confini dei
    // segmenti (l'unita' finisce a 262144 e 524288, e un'altra comincia a 262144), con trappole sfasate sparse.
    let dir = TestDir::new();
    let mut unita: Vec<u16> = vec![0x61; 786_432];
    for i in [
        131_071usize,
        131_072,
        262_143,
        262_144,
        393_215,
        393_216,
        524_287,
        786_431,
    ] {
        unita[i] = 0x0A;
    }
    let mut r = Lcg(77);
    for _ in 0..3000 {
        let i = r.below(786_000) as usize;
        if r.below(2) == 0 {
            unita[i] = 0x0A;
        } else {
            unita[i] = 0x0A41;
            unita[i + 1] = 0x0100;
        }
    }
    for e in ENDS {
        let bytes: Vec<u8> = unita
            .iter()
            .flat_map(|u| match e {
                End::Le => u.to_le_bytes(),
                End::Be => u.to_be_bytes(),
            })
            .collect();
        let src = FileSource::open(&dir.file(&bytes)).unwrap();
        let starts = model_starts(e, &bytes);
        for chunk in [600_000usize, 1 << 20] {
            let opts = BuildOptions {
                chunk_bytes: chunk,
                checkpoint_every: 256,
            };
            let (c, p) = flags();
            let idx = LineIndex::build_newline(&src, nl_of(e), &opts, &c, &p).unwrap();
            assert_eq!(idx.line_count(), starts.len() as u64, "{e:?} chunk={chunk}");
            for (k, &s) in starts.iter().enumerate() {
                assert_eq!(
                    idx.offset_of_line(&src, k as u64),
                    Some(s),
                    "{e:?} riga {k}"
                );
            }
            for o in [
                0u64, 262_141, 262_142, 262_143, 262_144, 262_145, 524_286, 524_287, 524_288,
            ] {
                assert_eq!(
                    idx.line_at_offset(&src, o),
                    Some(model_line_at(&starts, o)),
                    "{e:?} offset {o}"
                );
            }
        }
    }
}

#[test]
fn an_odd_tail_that_looks_like_a_newline_is_not_one() {
    let dir = TestDir::new();
    // LE: unita' U+0A41 (`41 0A`) e poi il byte 00: gli ultimi due byte sono `0A 00`, ma sfasati
    let mut d = dir.doc(&[0x41, 0x0A, 0x00], Newline::Le16, 3);
    build(&mut d, 4, 2);
    assert_eq!(d.line_count(), Some(1));
    // BE: U+4100 (`41 00`) e poi il byte 0A: gli ultimi due byte sono `00 0A`, ma sfasati
    let mut d = dir.doc(&[0x41, 0x00, 0x0A], Newline::Be16, 3);
    build(&mut d, 4, 2);
    assert_eq!(d.line_count(), Some(1));
}

#[test]
fn a_long_row_in_an_odd_length_document_is_scanned_aligned() {
    // 40000 'a', poi U+0A41 U+0100 (contiene `0A 00` sfasato di uno), poi 100 'a' e un byte spaiato: lunghezza dispari.
    // Nessun a-capo: la riga che contiene la fine comincia a 0. Le letture all'indietro sono a blocchi da 64 KiB: se
    // partissero da una posizione dispari vedrebbero l'a-capo finto.
    let dir = TestDir::new();
    let mut testo = "a".repeat(40_000);
    testo.push('\u{0A41}');
    testo.push('\u{0100}');
    testo.push_str(&"a".repeat(100));
    let mut bytes = enc(End::Le, &testo);
    bytes.push(0x61);
    assert_eq!(bytes.len() % 2, 1);
    let d = dir.doc(&bytes, Newline::Le16, 3);
    let len = bytes.len() as u64;
    assert_eq!(row_start(&d, len, 1 << 20), (0, true));
    assert_eq!(row_start(&d, len - 1, 1 << 20), (0, true));
    assert_eq!(prev_row_start(&d, len, 1 << 20), Some(0));
}

#[test]
fn a_misaligned_match_does_not_hide_an_overlapping_aligned_one() {
    let dir = TestDir::new();
    let (c, p) = flags();
    // LE: `41 41 41 00`; l'ago `41 41` sta a 0 (allineato) e a 1 (sfasato): all'indietro prima si vede quello a 1
    let d = dir.doc(&[0x41, 0x41, 0x41, 0x00], Newline::Le16, 3);
    let s = d.snapshot().unwrap();
    assert_eq!(s.find_prev(&[0x41, 0x41], 100, &c, &p).unwrap(), Some(0));
    // `00 41 41 41`; l'ago `41 41` sta a 1 (sfasato) e a 2 (allineato): in avanti prima si vede quello a 1
    let d = dir.doc(&[0x00, 0x41, 0x41, 0x41], Newline::Le16, 3);
    let s = d.snapshot().unwrap();
    assert_eq!(s.find_next(&[0x41, 0x41], 0, &c, &p).unwrap(), Some(2));
    for chunk in [1usize, 2, 3] {
        let s = d.snapshot().unwrap().with_chunk(chunk);
        assert_eq!(s.find_next(&[0x41, 0x41], 0, &c, &p).unwrap(), Some(2));
    }
}
