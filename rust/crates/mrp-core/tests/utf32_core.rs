//! Il nucleo con l'a-capo a 32 bit (UTF-32 LE e BE): la forma `Newline`, l'indice delle righe, il documento con
//! modifiche, la scansione per righe e la ricerca. Scritti da ARCH prima dell'implementazione. Gli attesi vengono da un
//! MODELLO scritto qui, che lavora sulle unita' u32 ottenute dal testo con `char as u32` (mai dal codice del nucleo):
//! l'inizio di una riga e' la unita' dopo una unita' 0x0000000A.
//!
//! Le trappole volute: caratteri che contengono i byte `0A` e `00` (U+0A00, U+0A0A) e coppie di unita' vicine che
//! formano `0A 00 00 00` (LE) o `00 00 00 0A` (BE) SFASATE di 1, 2 o 3 byte (U+A0000 seguito da U+100000 in LE;
//! U+10000 seguito da U+A0000 in BE).

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
        let p = std::env::temp_dir().join(format!("mrp_u32_test_{}_{}", std::process::id(), n));
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
        End::Le => Newline::Le32,
        End::Be => Newline::Be32,
    }
}

/// `s` in UTF-32 con l'endian `e`, scrivendo il valore di ogni carattere.
fn enc(e: End, s: &str) -> Vec<u8> {
    s.chars()
        .flat_map(|c| match e {
            End::Le => (c as u32).to_le_bytes(),
            End::Be => (c as u32).to_be_bytes(),
        })
        .collect()
}

/// Le unita' intere di `bytes` (i byte finali che non ne completano una non contano).
fn units(e: End, bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| match e {
            End::Le => u32::from_le_bytes([c[0], c[1], c[2], c[3]]),
            End::Be => u32::from_be_bytes([c[0], c[1], c[2], c[3]]),
        })
        .collect()
}

const ALFABETO: [char; 16] = [
    'a',
    'b',
    ' ',
    '\n',
    '\n',
    '\r',
    'é',
    '\u{0A0A}',
    '\u{0A00}',
    '\u{0100}',
    '\u{10000}',
    '\u{A0000}',
    '\u{100000}',
    '\u{1F600}',
    '€',
    '\u{0A41}',
];

fn random_text(r: &mut Lcg, n: usize) -> String {
    (0..n)
        .map(|_| ALFABETO[r.below(ALFABETO.len() as u64) as usize])
        .collect()
}

/// Un coda di 1, 2 o 3 byte che non completa una unita'.
fn coda(r: &mut Lcg) -> Vec<u8> {
    let n = 1 + r.below(3) as usize;
    [0x0A, 0x00, 0x41][..n].to_vec()
}

// ---------- la forma dell'a-capo ----------

/// Oracolo: le posizioni degli a-capo guardando una unita' alla volta, partendo da 0.
fn modello_nl(nl: Newline, hay: &[u8]) -> Vec<usize> {
    let u = nl.unit();
    let mut v = Vec::new();
    let mut i = 0;
    while i + u <= hay.len() {
        let unita = &hay[i..i + u];
        let ok = match nl {
            Newline::Le32 => unita == [0x0A, 0, 0, 0],
            Newline::Be32 => unita == [0, 0, 0, 0x0A],
            Newline::Cr => unita == [0x0D],
            Newline::Le16 => unita == [0x0A, 0],
            Newline::Be16 => unita == [0, 0x0A],
            Newline::Byte => unita == [0x0A],
        };
        if ok {
            v.push(i);
        }
        i += u;
    }
    v
}

#[test]
fn the_32_bit_forms_have_unit_four_and_their_bytes() {
    assert_eq!(Newline::Le32.unit(), 4);
    assert_eq!(Newline::Be32.unit(), 4);
    assert_eq!(Newline::Le32.bytes(), &[0x0A, 0, 0, 0]);
    assert_eq!(Newline::Be32.bytes(), &[0, 0, 0, 0x0A]);
    assert!(Newline::Le32.is_newline(&[0x0A, 0, 0, 0]));
    assert!(!Newline::Le32.is_newline(&[0, 0, 0, 0x0A]));
    assert!(Newline::Be32.is_newline(&[0, 0, 0, 0x0A]));
    assert!(!Newline::Be32.is_newline(&[0x0A, 0, 0, 0]));
}

#[test]
fn plain_utf32_text_and_the_wrong_form() {
    // "a\nb\n"
    let le = enc(End::Le, "a\nb\n");
    let be = enc(End::Be, "a\nb\n");
    assert_eq!(
        Newline::Le32.positions(&le).collect::<Vec<_>>(),
        vec![4, 12]
    );
    assert_eq!(
        Newline::Be32.positions(&be).collect::<Vec<_>>(),
        vec![4, 12]
    );
    assert_eq!(Newline::Le32.count(&le), 2);
    assert_eq!(Newline::Le32.find(&le), Some(4));
    assert_eq!(Newline::Le32.rfind(&le), Some(12));
    assert_eq!(Newline::Be32.rfind(&be), Some(12));
    assert!(Newline::Le32.ends_with_newline(&le));
    assert!(Newline::Be32.ends_with_newline(&be));
    assert_eq!(Newline::Be32.count(&le), 0);
    assert_eq!(Newline::Le32.count(&be), 0);
}

#[test]
fn the_shifted_newline_inside_two_characters_is_not_one() {
    // LE: U+A0000 = 00 00 0A 00, poi U+100000 = 00 00 10 00: i byte 2..6 sono `0A 00 00 00`, sfasati di 2
    let le = enc(End::Le, "\u{A0000}\u{100000}");
    assert_eq!(&le[2..6], &[0x0A, 0, 0, 0]);
    assert_eq!(Newline::Le32.count(&le), 0);
    assert_eq!(Newline::Le32.find(&le), None);
    assert_eq!(Newline::Le32.rfind(&le), None);
    // BE: U+10000 = 00 01 00 00, poi U+A0000 = 00 0A 00 00: i byte 2..6 sono `00 00 00 0A`, sfasati di 2
    let be = enc(End::Be, "\u{10000}\u{A0000}");
    assert_eq!(&be[2..6], &[0, 0, 0, 0x0A]);
    assert_eq!(Newline::Be32.count(&be), 0);
    assert_eq!(Newline::Be32.find(&be), None);
    // sfasati di 1: LE `?? 0A 00 00 | 00 ...` non e' un a-capo; il carattere U+0A00 e' `00 0A 00 00`
    let le = enc(End::Le, "\u{0A00}\u{0100}");
    assert_eq!(Newline::Le32.count(&le), 0);
    // un a-capo vero subito dopo resta trovato, alla sua posizione allineata
    let le = enc(End::Le, "\u{A0000}\u{100000}\n");
    assert_eq!(Newline::Le32.positions(&le).collect::<Vec<_>>(), vec![8]);
    let be = enc(End::Be, "\u{10000}\u{A0000}\n");
    assert_eq!(Newline::Be32.positions(&be).collect::<Vec<_>>(), vec![8]);
}

#[test]
fn a_tail_that_does_not_complete_a_unit_is_never_a_newline() {
    // LE: `0A 00 00` in coda non e' un a-capo (manca un byte)
    assert_eq!(Newline::Le32.count(&[0x61, 0, 0, 0, 0x0A, 0, 0]), 0);
    assert!(!Newline::Le32.ends_with_newline(&[0x61, 0, 0, 0, 0x0A, 0, 0]));
    // BE: `00 00 0A` in coda
    assert_eq!(Newline::Be32.count(&[0, 0, 0, 0x61, 0, 0, 0x0A]), 0);
    // un a-capo vero seguito da byte spaiati
    for extra in 1..4 {
        let mut v = vec![0x0A, 0, 0, 0];
        v.extend(vec![0x7A; extra]);
        assert_eq!(Newline::Le32.positions(&v).collect::<Vec<_>>(), vec![0]);
        assert!(!Newline::Le32.ends_with_newline(&v));
    }
    for nl in [Newline::Le32, Newline::Be32] {
        assert_eq!(nl.count(&[]), 0);
        assert_eq!(nl.count(&[0x0A, 0, 0]), 0);
        assert_eq!(nl.find(&[]), None);
        assert!(!nl.ends_with_newline(&[]));
        assert!(!nl.ends_with_newline(&[0x0A]));
    }
}

#[test]
fn random_bytes_agree_with_the_unit_model_for_every_form() {
    let mut r = Lcg(4242);
    let alfabeto = [0x00u8, 0x0A, 0x01, 0x41, 0x00, 0x0A, 0x00];
    for len in 0..70 {
        for _ in 0..40 {
            let hay: Vec<u8> = (0..len)
                .map(|_| alfabeto[r.below(alfabeto.len() as u64) as usize])
                .collect();
            for nl in [
                Newline::Byte,
                Newline::Le16,
                Newline::Be16,
                Newline::Le32,
                Newline::Be32,
            ] {
                let atteso = modello_nl(nl, &hay);
                let trovati: Vec<usize> = nl.positions(&hay).collect();
                assert_eq!(trovati, atteso, "{nl:?} {hay:?}");
                let mut indietro: Vec<usize> = nl.positions(&hay).rev().collect();
                indietro.reverse();
                assert_eq!(indietro, atteso, "{nl:?} all'indietro {hay:?}");
                assert_eq!(nl.count(&hay), atteso.len() as u64, "{nl:?} {hay:?}");
                assert_eq!(nl.find(&hay), atteso.first().copied());
                assert_eq!(nl.rfind(&hay), atteso.last().copied());
                let finisce = !hay.is_empty()
                    && hay.len().is_multiple_of(nl.unit())
                    && atteso.last() == Some(&(hay.len() - nl.unit()));
                assert_eq!(nl.ends_with_newline(&hay), finisce, "{nl:?} {hay:?}");
            }
        }
    }
}

// ---------- indice delle righe ----------

/// Modello: gli inizi di riga, in byte. Una riga comincia a 0 (se il documento non e' vuoto) e subito dopo ogni unita'
/// 0x0000000A, se la posizione e' ancora dentro il documento (byte finali spaiati la rendono un inizio).
fn model_starts(e: End, bytes: &[u8]) -> Vec<u64> {
    let mut v = Vec::new();
    if !bytes.is_empty() {
        v.push(0);
    }
    for (i, u) in units(e, bytes).iter().enumerate() {
        if *u == 0x0A && 4 * (i + 1) < bytes.len() {
            v.push(4 * (i as u64 + 1));
        }
    }
    v
}

fn model_line_at(starts: &[u64], o: u64) -> u64 {
    (starts.partition_point(|&s| s <= o) - 1) as u64
}

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
fn the_index_matches_the_model_for_both_endians_and_tails() {
    let dir = TestDir::new();
    let mut r = Lcg(7);
    for round in 0..20 {
        let n = r.below(80) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            for con_coda in [false, true] {
                let mut bytes = enc(e, &text);
                if con_coda {
                    bytes.extend(coda(&mut r));
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
                            "giro {round} {e:?} coda={con_coda} chunk={chunk} ogni={every}"
                        );
                        check_index(&idx, &src, &bytes, e, &ctx);
                    }
                }
            }
        }
    }
}

#[test]
fn known_utf32_lines_with_the_trap_characters() {
    let dir = TestDir::new();
    // LE: "A" U+A0000 U+100000 "\n" "B": l'a-capo sfasato dentro i due caratteri NON conta; l'unico vero e' dopo
    let bytes = enc(End::Le, "A\u{A0000}\u{100000}\nB");
    assert_eq!(bytes.len(), 20);
    let src = FileSource::open(&dir.file(&bytes)).unwrap();
    let (c, p) = flags();
    let idx = LineIndex::build_for(&src, Newline::Le32, &c, &p).unwrap();
    assert_eq!(idx.line_count(), 2);
    assert_eq!(idx.offset_of_line(&src, 1), Some(16));
    // BE: U+10000 U+A0000 "\n" "B"
    let bytes = enc(End::Be, "\u{10000}\u{A0000}\nB");
    let src = FileSource::open(&dir.file(&bytes)).unwrap();
    let idx = LineIndex::build_for(&src, Newline::Be32, &c, &p).unwrap();
    assert_eq!(idx.line_count(), 2);
    assert_eq!(idx.offset_of_line(&src, 1), Some(12));
    // gli stessi byte letti come a-capo a byte: piu' righe (c'e' lo 0x0A di U+A0000 e quello vero)
    let (c, p) = flags();
    let sbagliato = LineIndex::build_for(&src, Newline::Byte, &c, &p).unwrap();
    assert_eq!(sbagliato.line_count(), 3);
    // un file vuoto non ha righe
    let src = FileSource::open(&dir.file(b"")).unwrap();
    let idx = LineIndex::build_for(&src, Newline::Le32, &c, &p).unwrap();
    assert_eq!(idx.line_count(), 0);
    assert_eq!(idx.newline(), Newline::Le32);
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
fn the_document_lines_match_the_model_without_edits_tails_included() {
    let dir = TestDir::new();
    let mut r = Lcg(11);
    for _ in 0..20 {
        let n = r.below(60) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            for con_coda in [false, true] {
                let mut bytes = enc(e, &text);
                if con_coda {
                    bytes.extend(coda(&mut r));
                }
                let mut d = dir.doc(&bytes, nl_of(e), 3);
                assert_eq!(d.newline(), nl_of(e));
                build(&mut d, 8, 2);
                check_doc(&mut d, &bytes, e, &format!("{e:?} coda={con_coda}"));
            }
        }
    }
}

#[test]
fn the_document_lines_follow_aligned_edits() {
    let dir = TestDir::new();
    let mut r = Lcg(99);
    for round in 0..15 {
        let n = r.below(50) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            for cap in [2usize, 3, 8] {
                let mut model = enc(e, &text);
                let mut d = dir.doc(&model, nl_of(e), cap);
                build(&mut d, 8, 2);
                check_doc(
                    &mut d,
                    &model,
                    e,
                    &format!("giro {round} {e:?} cap={cap} inizio"),
                );
                for passo in 0..20 {
                    let len = model.len();
                    if r.below(3) != 0 || len == 0 {
                        // inserisce a una posizione multipla di 4
                        let pos = 4 * r.below(len as u64 / 4 + 1) as usize;
                        let k = r.below(5) as usize;
                        let dati = enc(e, &random_text(&mut r, k));
                        d.insert(pos as u64, &dati).unwrap();
                        model.splice(pos..pos, dati);
                    } else {
                        // cancella un tratto multiplo di 4, da una posizione multipla di 4
                        let pos = 4 * r.below(len as u64 / 4) as usize;
                        let max = (len - pos) / 4;
                        let quanto = 4 * (r.below(max as u64 + 1) as usize).min(3);
                        d.delete(pos as u64, quanto as u64).unwrap();
                        model.drain(pos..pos + quanto);
                    }
                    if r.below(4) == 0 {
                        d.seal();
                    }
                    let ctx = format!("giro {round} {e:?} cap={cap} passo {passo}");
                    check_doc(&mut d, &model, e, &ctx);
                }
                while d.undo().is_some() {}
                let ctx = format!("giro {round} {e:?} cap={cap} dopo undo");
                check_doc(&mut d, &enc(e, &text), e, &ctx);
            }
        }
    }
}

#[test]
fn set_newline_rules_for_the_32_bit_forms() {
    let dir = TestDir::new();
    let bytes = enc(End::Le, "a\nb\n");
    let mut d = Document::open(&dir.file(&bytes)).unwrap();
    assert_eq!(d.newline(), Newline::Byte);
    assert!(d.set_newline(Newline::Le32));
    assert_eq!(d.newline(), Newline::Le32);
    build(&mut d, 8, 2);
    assert!(d.has_lines());
    assert_eq!(d.line_count(), Some(2));
    // stessa forma: l'indice resta
    assert!(d.set_newline(Newline::Le32));
    assert!(d.has_lines());
    // forma diversa senza storia: cambia e l'indice si butta
    assert!(d.set_newline(Newline::Be32));
    assert!(!d.has_lines());
    // un indice costruito per un'altra forma non si installa
    let src = FileSource::open(&dir.file(&bytes)).unwrap();
    let (c, p) = flags();
    d.install_lines(LineIndex::build_for(&src, Newline::Le32, &c, &p).unwrap());
    assert!(!d.has_lines());
    d.install_lines(LineIndex::build_for(&src, Newline::Be32, &c, &p).unwrap());
    assert!(d.has_lines());
    // con una modifica la forma non si cambia piu'
    d.insert(0, &[0, 0, 0, 0x61]).unwrap();
    assert!(!d.set_newline(Newline::Le32));
    assert!(!d.set_newline(Newline::Byte));
    assert_eq!(d.newline(), Newline::Be32);
}

// ---------- scansione per righe ----------

#[derive(Debug, PartialEq)]
enum N {
    Line(usize),
    Cut(usize),
    End,
}

fn m_primo(u: &[u32], da: usize, fine: usize) -> Option<usize> {
    (da..fine.min(u.len())).find(|&i| u[i] == 0x0A)
}

fn m_ultimo(u: &[u32], da: usize, fine: usize) -> Option<usize> {
    (da..fine.min(u.len())).rev().find(|&i| u[i] == 0x0A)
}

/// Le stesse regole di `scan.rs` ma in unita': `pos` e `max` sono indici di unita' (`max >= 1`).
fn m_next(u: &[u32], pos: usize, max: usize) -> N {
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

fn m_end(u: &[u32], pos: usize, max: usize) -> usize {
    let len = u.len();
    let fine = len.min(pos + max);
    let inizio = pos.min(len);
    m_primo(u, inizio, fine).unwrap_or(fine.max(inizio))
}

fn m_start(u: &[u32], pos: usize, max: usize) -> (usize, bool) {
    let pos = pos.min(u.len());
    let da = pos.saturating_sub(max);
    match m_ultimo(u, da, pos) {
        Some(i) => (i + 1, true),
        None if pos <= max => (0, true),
        None => (pos - max, false),
    }
}

fn m_prev(u: &[u32], pos: usize, max: usize) -> Option<usize> {
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
    for round in 0..15 {
        let n = r.below(50) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            let bytes = enc(e, &text);
            let d = dir.doc(&bytes, nl_of(e), 3);
            let u = units(e, &bytes);
            // max_scan in byte: il nucleo lo porta a un multiplo di 4 (per difetto), almeno 4
            for max_scan in [0u64, 1, 3, 4, 5, 7, 8, 9, 16, 33, 1000] {
                let max = ((max_scan / 4) as usize).max(1);
                for p in 0..=u.len() + 2 {
                    let pb = 4 * p as u64;
                    let ctx = format!("giro {round} {e:?} max_scan={max_scan} pos={pb}");
                    let atteso = match m_next(&u, p, max) {
                        N::Line(i) => Next::Line(4 * i as u64),
                        N::Cut(i) => Next::Cut(4 * i as u64),
                        N::End => Next::End,
                    };
                    assert_eq!(next_row(&d, pb, max_scan), atteso, "next_row {ctx}");
                    assert_eq!(
                        row_end(&d, pb, max_scan),
                        4 * m_end(&u, p, max) as u64,
                        "row_end {ctx}"
                    );
                    let (s, vera) = m_start(&u, p, max);
                    assert_eq!(
                        row_start(&d, pb, max_scan),
                        (4 * s as u64, vera),
                        "row_start {ctx}"
                    );
                    assert_eq!(
                        prev_row_start(&d, pb, max_scan),
                        m_prev(&u, p, max).map(|i| 4 * i as u64),
                        "prev_row_start {ctx}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_scanning_with_a_tail_that_looks_like_a_newline() {
    let dir = TestDir::new();
    // LE: "a\n" e poi `0A 00 00`: tre byte che non sono una unita'
    let mut bytes = enc(End::Le, "a\n");
    bytes.extend([0x0A, 0, 0]);
    let d = dir.doc(&bytes, Newline::Le32, 3);
    assert_eq!(next_row(&d, 0, 100), Next::Line(8));
    assert_eq!(row_end(&d, 0, 100), 4);
    assert_eq!(row_end(&d, 8, 100), 11);
    assert_eq!(next_row(&d, 8, 100), Next::End);
    assert_eq!(row_start(&d, 11, 100), (8, true));
    assert_eq!(prev_row_start(&d, 8, 100), Some(0));
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
    // LE: U+A0000 U+100000 sono `00 00 0A 00 | 00 00 10 00`: `0A 00 00 00` compare al byte 2, sfasato
    let d = dir.doc(&enc(End::Le, "\u{A0000}\u{100000}"), Newline::Le32, 3);
    let s = d.snapshot().unwrap();
    assert_eq!(s.find_next(&enc(End::Le, "\n"), 0, &c, &p).unwrap(), None);
    assert_eq!(s.find_prev(&enc(End::Le, "\n"), 100, &c, &p).unwrap(), None);
    let d = dir.doc(&enc(End::Le, "\u{A0000}\u{100000}\n"), Newline::Le32, 3);
    let s = d.snapshot().unwrap();
    assert_eq!(
        s.find_next(&enc(End::Le, "\n"), 0, &c, &p).unwrap(),
        Some(8)
    );
    assert_eq!(
        s.find_prev(&enc(End::Le, "\n"), 100, &c, &p).unwrap(),
        Some(8)
    );
    // BE: U+10000 U+A0000 ha `00 00 00 0A` al byte 2
    let d = dir.doc(&enc(End::Be, "\u{10000}\u{A0000}"), Newline::Be32, 3);
    let s = d.snapshot().unwrap();
    assert_eq!(s.find_next(&enc(End::Be, "\n"), 0, &c, &p).unwrap(), None);
    // un documento a byte la trova invece: l'allineamento e' solo per le forme larghe
    let b = Document::open(&dir.file(&enc(End::Be, "\u{10000}\u{A0000}"))).unwrap();
    let s = b.snapshot().unwrap();
    assert_eq!(s.find_next(&[0, 0, 0, 0x0A], 0, &c, &p).unwrap(), Some(2));
}

#[test]
fn search_matches_the_aligned_model_for_every_start_and_chunk() {
    let dir = TestDir::new();
    let mut r = Lcg(31);
    let (c, p) = flags();
    for round in 0..8 {
        let n = 1 + r.below(24) as usize;
        let text = random_text(&mut r, n);
        for e in ENDS {
            let bytes = enc(e, &text);
            let mut d = dir.doc(&bytes, nl_of(e), 3);
            // anche con una modifica allineata, perche' la fotografia legge dai pezzi
            let mut model = bytes.clone();
            let pos = 4 * r.below(model.len() as u64 / 4 + 1) as usize;
            let extra = enc(e, "A\u{A0000}\u{100000}");
            d.insert(pos as u64, &extra).unwrap();
            model.splice(pos..pos, extra);
            let aghi: Vec<Vec<u8>> = vec![
                enc(e, "A"),
                enc(e, "a"),
                enc(e, "\n"),
                enc(e, "\u{A0000}\u{100000}"),
                enc(e, "ab"),
                enc(e, &random_text(&mut r, 2)),
                vec![0x41],
                vec![0x00, 0x00, 0x0A],
            ];
            for ago in &aghi {
                for chunk in [1usize, 3, 4, 5, 64] {
                    let s = d.snapshot().unwrap().with_chunk(chunk);
                    for da in 0..=model.len() as u64 + 1 {
                        let ctx = format!("giro {round} {e:?} ago={ago:?} chunk={chunk} da={da}");
                        assert_eq!(
                            s.find_next(ago, da, &c, &p).unwrap(),
                            m_find_next(&model, ago, da, 4),
                            "find_next {ctx}"
                        );
                        assert_eq!(
                            s.find_prev(ago, da, &c, &p).unwrap(),
                            m_find_prev(&model, ago, da, 4),
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
    let d = dir.doc(&enc(End::Le, "abc"), Newline::Le32, 3);
    let s = d.snapshot().unwrap();
    let (c, p) = flags();
    c.store(true, Ordering::Relaxed);
    assert!(matches!(
        s.find_next(&enc(End::Le, "b"), 0, &c, &p),
        Err(SearchError::Cancelled)
    ));
}
