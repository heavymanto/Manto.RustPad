//! La vista e la ricerca con le codifiche multibyte asiatiche (E28). Scritti da ARCH. I byte dei testi di prova si
//! ottengono con `Encoding::encode_str` (la scrittura e' verificata contro Python e .NET in multibyte.rs); gli attesi
//! delle posizioni vengono da un MODELLO scritto qui (le lunghezze dei caratteri sono quelle di `encode`, mai le
//! regole di lunghezza di `first_char` ne' la ricerca all'indietro).

use mrp_core::docsearch::SearchError;
use mrp_core::encoding::Encoding;
use mrp_core::view::{col_of, offset_at_col, rows_from, ViewOptions};
use mrp_core::Document;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_mbview_test_{}_{}.txt", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn enc(nome: &str) -> Encoding {
    Encoding::by_name(nome).unwrap()
}

fn bytes(nome: &str, s: &str) -> Vec<u8> {
    enc(nome)
        .encode_str(s)
        .unwrap_or_else(|c| panic!("{nome}: {c:?} non si scrive"))
}

fn open(data: &[u8], e: Encoding) -> (TempFile, Document, ViewOptions) {
    let f = TempFile::new(data);
    let d = Document::open(&f.0).unwrap();
    let o = ViewOptions {
        tab_width: 4,
        encoding: e,
        ..ViewOptions::default()
    };
    (f, d, o)
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
}

// ---------- vista ----------

#[test]
fn rows_of_shift_jis_text() {
    let e = enc("shift-jis");
    // 日本語 (2 byte l'uno, 2 celle l'uno), tab fino alla colonna 8, 'A', a-capo; poi ア (2 byte)
    let (_f, d, o) = open(&bytes("shift-jis", "日本語\tA\nア"), e);
    let r = rows_from(&d, 0, 3, 0, 80, &o);
    assert_eq!(r.len(), 2);
    assert_eq!((r[0].start, r[0].end), (0, 8));
    assert_eq!(r[0].text, "日本語  A");
    assert!(r[0].starts_line);
    assert_eq!((r[1].start, r[1].end, r[1].text.as_str()), (9, 11, "ア"));
}

#[test]
fn a_trail_byte_equal_to_a_backslash_or_a_letter_stays_inside_its_character() {
    // 表 = 95 5C, ソ = 83 5C, ア = 83 41: i secondi byte sono '\', '\' e 'A'
    let e = enc("shift-jis");
    let (_f, d, o) = open(&bytes("shift-jis", "表ソア\\A\n"), e);
    let r = rows_from(&d, 0, 1, 0, 80, &o).remove(0);
    assert_eq!(r.text, "表ソア\\A");
    assert_eq!((r.start, r.end), (0, 8));
    // i confini: ogni carattere largo 2 celle e lungo 2 byte
    for (byte, col) in [(0u64, 0usize), (2, 2), (4, 4), (6, 6), (7, 7), (8, 8)] {
        assert_eq!(col_of(&d, 0, byte, &o), col, "byte {byte}");
    }
    for (col, byte) in [
        (0u64, 0u64),
        (1, 0),
        (2, 2),
        (3, 2),
        (4, 4),
        (6, 6),
        (7, 7),
        (8, 8),
    ] {
        assert_eq!(offset_at_col(&d, 0, col, &o), byte, "colonna {col}");
    }
}

#[test]
fn half_width_katakana_is_one_byte_and_one_cell() {
    // ｱ = B1 in Shift_JIS, 8E B1 in EUC-JP
    for (nome, atteso_byte) in [("shift-jis", 1u64), ("euc-jp", 2)] {
        let e = enc(nome);
        let (_f, d, o) = open(&bytes(nome, "a\u{FF71}b\n"), e);
        let r = rows_from(&d, 0, 1, 0, 80, &o).remove(0);
        assert_eq!(r.text, "a\u{FF71}b", "{nome}");
        assert_eq!(col_of(&d, 0, 1 + atteso_byte, &o), 2, "{nome}");
        assert_eq!(r.end, 2 + atteso_byte, "{nome}");
    }
}

#[test]
fn invalid_and_truncated_bytes_are_replacement_characters() {
    let e = enc("shift-jis");
    // primo byte da solo in fondo al file
    let (_f, d, o) = open(&[b'a', b'b', 0x81], e);
    let r = rows_from(&d, 0, 1, 0, 80, &o).remove(0);
    assert_eq!(r.text, "ab\u{fffd}");
    assert_eq!(r.end, 3);
    // primo byte seguito da un ASCII che non e' un secondo byte valido (lo spazio; 'x' = 0x78 lo sarebbe: 81 78 e' un carattere)
    let (_g, d, o) = open(&[0x81, b' ', b'\n'], e);
    let r = rows_from(&d, 0, 1, 0, 80, &o).remove(0);
    assert_eq!(r.text, "\u{fffd} ");
    assert_eq!(r.end, 2);
}

#[test]
fn characters_across_the_read_block_boundary_are_one_character() {
    // 40000 ideogrammi da 2 byte dopo una 'a': i caratteri cominciano a byte dispari, il confine dei 64 KiB (pari)
    // cade a meta' di uno; con GB18030 e una faccina da 4 byte il confine cade a 1 o 3 byte dall'inizio
    for (nome, carattere, lunghezza) in [
        ("shift-jis", '日', 2usize),
        ("euc-kr", '日', 2),
        ("gb18030", '\u{1F600}', 4),
    ] {
        let e = enc(nome);
        let n = 80_000 / lunghezza;
        let testo = format!("a{}\n", carattere.to_string().repeat(n));
        let (_f, d, o) = open(&bytes(nome, &testo), e);
        let r = rows_from(&d, 0, 1, 0, 200_000, &o).remove(0);
        assert_eq!(
            r.text,
            format!("a{}", carattere.to_string().repeat(n)),
            "{nome}"
        );
        assert_eq!(r.end, 1 + (n * lunghezza) as u64, "{nome}");
    }
}

// ---------- ricerca ----------

#[test]
fn search_does_not_match_inside_a_character() {
    let e = enc("shift-jis");
    let (c, p) = flags();
    // 表 = 95 5C: la barra rovesciata (5C) e' il secondo byte, non una barra
    let dati = bytes("shift-jis", "表\n");
    let (_f, d, _o) = open(&dati, e);
    let s = d.snapshot().unwrap();
    assert_eq!(
        s.find_next(b"\\", 0, &c, &p).unwrap(),
        Some(1),
        "senza confini la trova"
    );
    let s = s.with_boundary(e);
    assert_eq!(s.find_next(b"\\", 0, &c, &p).unwrap(), None);
    assert_eq!(s.find_prev(b"\\", 100, &c, &p).unwrap(), None);
    // con una barra vera dopo: solo quella (posizione 2)
    let dati = bytes("shift-jis", "表\\ソA ア\n");
    let (_g, d, _o) = open(&dati, e);
    let s = d.snapshot().unwrap().with_boundary(e);
    assert_eq!(s.find_next(b"\\", 0, &c, &p).unwrap(), Some(2));
    assert_eq!(s.find_prev(b"\\", 100, &c, &p).unwrap(), Some(2));
    // 'A' (41): ア = 83 41 la contiene come secondo byte (posizione 8); la vera 'A' e' a 5, dopo ソ
    assert_eq!(s.find_next(b"A", 0, &c, &p).unwrap(), Some(5));
    assert_eq!(s.find_prev(b"A", 100, &c, &p).unwrap(), Some(5));
    // cercare un carattere intero funziona e la fine dell'occorrenza e' un confine
    let ago = bytes("shift-jis", "ア");
    assert_eq!(s.find_next(&ago, 0, &c, &p).unwrap(), Some(7));
    // una codifica non multibyte non cambia: with_boundary non fa nulla
    let u = d.snapshot().unwrap().with_boundary(Encoding::Utf8);
    assert_eq!(u.find_next(b"\\", 0, &c, &p).unwrap(), Some(1));
}

#[test]
fn search_matches_only_at_character_starts_for_every_start_and_chunk() {
    let pool = [
        'a', 'A', '\\', '@', '0', ' ', '\n', '日', '本', '語', 'あ', 'ア', '表', 'ソ', '能', '中',
        '한',
    ];
    let mut r = Lcg(77);
    let (c, p) = flags();
    for nome in ["shift-jis", "euc-jp", "gbk", "gb18030", "big5", "euc-kr"] {
        let e = enc(nome);
        for round in 0..10 {
            // modello: i byte e gli inizi dei caratteri vengono da `encode`
            let n = 1 + (r.next() % 24) as usize;
            let mut byte: Vec<u8> = Vec::new();
            let mut inizi: Vec<usize> = Vec::new();
            for _ in 0..n {
                let ch = pool[(r.next() % pool.len() as u64) as usize];
                if let Some(b) = e.encode(ch) {
                    inizi.push(byte.len());
                    byte.extend(b);
                }
            }
            if byte.is_empty() {
                continue;
            }
            let (_f, d, _o) = open(&byte, e);
            let aghi: Vec<Vec<u8>> = vec![
                b"\\".to_vec(),
                b"A".to_vec(),
                b"@".to_vec(),
                b"0".to_vec(),
                e.encode('日').unwrap_or_default(),
                e.encode('ア').unwrap_or_default(),
                e.encode('表').unwrap_or_default(),
                {
                    let mut v = e.encode('a').unwrap();
                    v.extend(e.encode('日').unwrap_or_default());
                    v
                },
            ];
            for ago in aghi.iter().filter(|a| !a.is_empty()) {
                // tutte le occorrenze di byte che cominciano a un inizio di carattere
                let validi: Vec<u64> = (0..byte.len())
                    .filter(|&s| byte[s..].starts_with(ago) && inizi.contains(&s))
                    .map(|s| s as u64)
                    .collect();
                for chunk in [1usize, 2, 3, 5, 64] {
                    let s = d.snapshot().unwrap().with_chunk(chunk).with_boundary(e);
                    for da in 0..=byte.len() as u64 + 1 {
                        let ctx =
                            format!("{nome} giro {round} ago={ago:02x?} chunk={chunk} da={da}");
                        assert_eq!(
                            s.find_next(ago, da, &c, &p).unwrap(),
                            validi.iter().copied().find(|&v| v >= da),
                            "find_next {ctx}"
                        );
                        assert_eq!(
                            s.find_prev(ago, da, &c, &p).unwrap(),
                            validi.iter().copied().rev().find(|&v| v < da),
                            "find_prev {ctx}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn search_cancel_is_still_reported_with_boundaries() {
    let e = enc("shift-jis");
    let (_f, d, _o) = open(&bytes("shift-jis", "日本語\n"), e);
    let s = d.snapshot().unwrap().with_boundary(e);
    let (c, p) = flags();
    c.store(true, Ordering::Relaxed);
    assert!(matches!(
        s.find_next(b"a", 0, &c, &p),
        Err(SearchError::Cancelled)
    ));
}

#[test]
fn search_looks_back_far_enough_to_know_where_characters_start() {
    // 82 82 | 83 83 | 83 41 (ア): la 'A' (41) e' il secondo byte di ア. Guardando solo i 4 byte prima (82 83 83 83) si
    // leggerebbe (82 83)(83 83) e poi 41 da solo: la si prenderebbe per un inizio. Dal principio della riga si vede bene.
    let e = enc("shift-jis");
    let dati = [0x82, 0x82, 0x83, 0x83, 0x83, 0x41, b'\n'];
    let (_f, d, _o) = open(&dati, e);
    let (c, p) = flags();
    let s = d.snapshot().unwrap();
    assert_eq!(
        s.find_next(b"A", 0, &c, &p).unwrap(),
        Some(5),
        "senza confini la trova"
    );
    let s = s.with_boundary(e);
    assert_eq!(s.find_next(b"A", 0, &c, &p).unwrap(), None);
    assert_eq!(s.find_prev(b"A", 100, &c, &p).unwrap(), None);
}
