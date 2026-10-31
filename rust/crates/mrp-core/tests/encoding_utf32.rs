//! UTF-32 LE e BE in `Encoding`: nomi, ordine di `all()`, `first_char`, `last_char_len`, `encode`, `decode_lossy`,
//! riconoscimento del BOM. Scritti da ARCH prima dell'implementazione. Gli attesi vengono dalla libreria standard
//! (`char as u32`) e dal decodificatore UTF-32 di Python (`bytes.decode('utf-32-le', errors='replace')`): un U+FFFD per
//! ogni unita' non valida (valore sopra 0x10FFFF o surrogato D800-DFFF), e UNO SOLO per una coda di 1-3 byte.

use mrp_core::charsets::CHARSETS;
use mrp_core::encoding::Encoding;
use mrp_core::newline::Newline;

const LE: Encoding = Encoding::Utf32Le;
const BE: Encoding = Encoding::Utf32Be;

fn le(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

fn be(v: u32) -> [u8; 4] {
    v.to_be_bytes()
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

// ---------- nomi ----------

#[test]
fn names_and_aliases() {
    for n in [
        "utf-32le",
        "utf32le",
        "utf-32-le",
        "UTF-32LE",
        "Utf_32_Le",
        " utf-32le ",
    ] {
        assert_eq!(Encoding::by_name(n), Some(LE), "{n}");
    }
    for n in ["utf-32be", "utf32be", "utf-32-be", "UTF-32BE", "utf_32_be"] {
        assert_eq!(Encoding::by_name(n), Some(BE), "{n}");
    }
    // senza endian e' ambiguo: non esiste (come utf-16)
    for n in ["utf-32", "utf32", "ucs-4", "utf-32le2"] {
        assert_eq!(Encoding::by_name(n), None, "{n}");
    }
    assert_eq!(LE.name(), "utf-32le");
    assert_eq!(BE.name(), "utf-32be");
}

#[test]
fn all_lists_utf8_utf16_utf32_then_the_code_pages() {
    let all = Encoding::all();
    assert_eq!(all.len(), 5 + CHARSETS.len());
    assert_eq!(
        &all[..5],
        &[Encoding::Utf8, Encoding::Utf16Le, Encoding::Utf16Be, LE, BE]
    );
    for (i, e) in all.iter().enumerate().skip(5) {
        assert_eq!(e.name(), CHARSETS[i - 5].name);
    }
    let mut nomi: Vec<&str> = all.iter().map(|e| e.name()).collect();
    nomi.sort_unstable();
    nomi.dedup();
    assert_eq!(nomi.len(), all.len(), "nomi doppi");
    for e in &all {
        assert_eq!(Encoding::by_name(e.name()), Some(*e), "{}", e.name());
    }
}

#[test]
fn unit_newline_and_flags() {
    for e in [LE, BE] {
        assert_eq!(e.unit(), 4);
        assert!(!e.is_utf8());
        assert!(!e.is_utf16());
        assert!(e.is_utf32());
    }
    assert_eq!(LE.newline(), Newline::Le32);
    assert_eq!(BE.newline(), Newline::Be32);
    for e in [
        Encoding::Utf8,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
        Encoding::Single(0),
    ] {
        assert!(!e.is_utf32(), "{e:?}");
    }
    assert!(Encoding::Utf16Le.is_utf16());
}

// ---------- first_char ----------

#[test]
fn first_char_of_valid_units() {
    let casi: [(u32, char); 8] = [
        (0x61, 'a'),
        (0x0A, '\n'),
        (0x00, '\0'),
        (0xE9, 'é'),
        (0x20AC, '€'),
        (0x1F600, '\u{1F600}'),
        (0xFEFF, '\u{FEFF}'), // il BOM e' un carattere come gli altri
        (0x10FFFF, '\u{10FFFF}'),
    ];
    for (v, c) in casi {
        assert_eq!(LE.first_char(&le(v)), Some((c, 4)), "LE {v:#x}");
        assert_eq!(BE.first_char(&be(v)), Some((c, 4)), "BE {v:#x}");
    }
    // legge solo la prima unita'
    let mut b = le(0x61).to_vec();
    b.extend(le(0x62));
    assert_eq!(LE.first_char(&b), Some(('a', 4)));
    // l'endian conta: `61 00 00 00` letto come BE vale 0x61000000, sopra 0x10FFFF
    assert_eq!(BE.first_char(&le(0x61)), Some(('\u{fffd}', 4)));
}

#[test]
fn first_char_of_invalid_units_and_tails() {
    // valori non validi: un carattere U+FFFD di 4 byte
    for v in [
        0x110000u32,
        0xD800,
        0xDBFF,
        0xDC00,
        0xDFFF,
        0x7FFFFFFF,
        0xFFFFFFFF,
    ] {
        assert_eq!(LE.first_char(&le(v)), Some(('\u{fffd}', 4)), "LE {v:#x}");
        assert_eq!(BE.first_char(&be(v)), Some(('\u{fffd}', 4)), "BE {v:#x}");
    }
    // i limiti dei surrogati: D7FF e E000 sono caratteri veri
    assert_eq!(LE.first_char(&le(0xD7FF)), Some(('\u{D7FF}', 4)));
    assert_eq!(LE.first_char(&le(0xE000)), Some(('\u{E000}', 4)));
    // una coda che non completa una unita' e' UN carattere U+FFFD lungo quanto la coda
    for n in 1..4 {
        let tail = vec![0x41; n];
        assert_eq!(LE.first_char(&tail), Some(('\u{fffd}', n)), "LE coda {n}");
        assert_eq!(BE.first_char(&tail), Some(('\u{fffd}', n)), "BE coda {n}");
    }
    assert_eq!(LE.first_char(&[]), None);
    assert_eq!(BE.first_char(&[]), None);
}

// ---------- last_char_len ----------

#[test]
fn last_char_len_follows_the_alignment_from_the_start() {
    assert_eq!(LE.last_char_len(&[]), 0);
    assert_eq!(BE.last_char_len(&[]), 0);
    for e in [LE, BE] {
        for len in 1..=16usize {
            let bytes = vec![0x41u8; len];
            let atteso = if len % 4 == 0 { 4 } else { len % 4 };
            assert_eq!(e.last_char_len(&bytes), atteso, "{e:?} lunghezza {len}");
        }
    }
}

// ---------- encode ----------

#[test]
fn encode_writes_four_bytes_in_the_right_order() {
    assert_eq!(LE.encode('a'), Some(vec![0x61, 0, 0, 0]));
    assert_eq!(BE.encode('a'), Some(vec![0, 0, 0, 0x61]));
    assert_eq!(LE.encode('€'), Some(vec![0xAC, 0x20, 0, 0]));
    assert_eq!(BE.encode('€'), Some(vec![0, 0, 0x20, 0xAC]));
    assert_eq!(LE.encode('\u{1F600}'), Some(vec![0x00, 0xF6, 0x01, 0]));
    assert_eq!(BE.encode('\u{1F600}'), Some(vec![0, 0x01, 0xF6, 0x00]));
    assert_eq!(LE.encode('\n'), Some(vec![0x0A, 0, 0, 0]));
    assert_eq!(BE.encode('\n'), Some(vec![0, 0, 0, 0x0A]));
    // in UTF-32 si scrive tutto, U+FFFD compreso
    assert_eq!(LE.encode('\u{fffd}'), Some(vec![0xFD, 0xFF, 0, 0]));
    assert_eq!(BE.encode('\u{10FFFF}'), Some(vec![0, 0x10, 0xFF, 0xFF]));
    assert_eq!(LE.encode_str("a\n"), Ok(vec![0x61, 0, 0, 0, 0x0A, 0, 0, 0]));
    assert_eq!(BE.encode_str(""), Ok(vec![]));
    // la forma dell'a-capo coincide con `encode('\n')`
    assert_eq!(LE.encode('\n').unwrap(), LE.newline().bytes());
    assert_eq!(BE.encode('\n').unwrap(), BE.newline().bytes());
}

// ---------- decode_lossy: i casi di Python ----------

#[test]
fn decode_lossy_matches_python() {
    // (endian, byte, atteso) calcolati con bytes.decode('utf-32-xx', errors='replace')
    let casi: &[(Encoding, &[u8], &str)] = &[
        (
            LE,
            &[0x61, 0, 0, 0, 0, 0, 0x11, 0, 0x41, 0],
            "a\u{fffd}\u{fffd}",
        ),
        (LE, &[0, 0xD8, 0, 0, 0x62, 0, 0, 0], "\u{fffd}b"),
        (LE, &[0xFF, 0xDF, 0, 0], "\u{fffd}"),
        (LE, &[0xFF, 0xFF, 0x10, 0, 0x41], "\u{10FFFF}\u{fffd}"),
        (LE, &[0xFF, 0xFF, 0xFF, 0xFF], "\u{fffd}"),
        (
            BE,
            &[0, 0x11, 0, 0, 0, 0, 0, 0x61, 0, 0],
            "\u{fffd}a\u{fffd}",
        ),
        (LE, &[1, 2, 3], "\u{fffd}"),
        (LE, &[0x61, 0, 0, 0, 1, 2, 3], "a\u{fffd}"),
        (BE, &[], ""),
    ];
    for (e, bytes, atteso) in casi {
        assert_eq!(e.decode_lossy(bytes), *atteso, "{e:?} {bytes:?}");
    }
}

#[test]
fn text_round_trips_and_first_char_walks_it() {
    let mut r = Lcg(77);
    let pool = [
        'a',
        'Z',
        '\n',
        '\r',
        '\t',
        'é',
        '€',
        '\u{0A0A}',
        '\u{0100}',
        '\u{D7FF}',
        '\u{E000}',
        '\u{FFFD}',
        '\u{FEFF}',
        '\u{10000}',
        '\u{1F600}',
        '\u{A0000}',
        '\u{10FFFF}',
    ];
    for _ in 0..200 {
        let n = (r.next() % 40) as usize;
        let s: String = (0..n)
            .map(|_| pool[(r.next() % pool.len() as u64) as usize])
            .collect();
        for e in [LE, BE] {
            let bytes = e.encode_str(&s).unwrap();
            assert_eq!(bytes.len(), 4 * s.chars().count());
            // l'oracolo: i valori dei caratteri scritti con la libreria standard
            let atteso: Vec<u8> = s
                .chars()
                .flat_map(|c| if e == LE { le(c as u32) } else { be(c as u32) })
                .collect();
            assert_eq!(bytes, atteso, "{e:?} {s:?}");
            assert_eq!(e.decode_lossy(&bytes), s, "{e:?} {s:?}");
            let mut i = 0;
            let mut visti = String::new();
            while let Some((c, k)) = e.first_char(&bytes[i..]) {
                assert_eq!(k, 4);
                visti.push(c);
                i += k;
            }
            assert_eq!(visti, s);
            if !bytes.is_empty() {
                assert_eq!(e.last_char_len(&bytes), 4);
            }
        }
    }
}

// ---------- riconoscimento ----------

#[test]
fn detect_utf32_by_bom() {
    // BOM LE: FF FE 00 00; BOM BE: 00 00 FE FF
    assert_eq!(Encoding::detect(&[0xFF, 0xFE, 0, 0]), LE);
    assert_eq!(Encoding::detect(&[0, 0, 0xFE, 0xFF]), BE);
    let mut testo = vec![0xFF, 0xFE, 0, 0];
    testo.extend(le(0x61));
    testo.extend(le(0x0A));
    assert_eq!(Encoding::detect(&testo), LE);
    let mut testo = vec![0, 0, 0xFE, 0xFF];
    testo.extend(be(0x61));
    testo.extend(be(0x0A));
    assert_eq!(Encoding::detect(&testo), BE);
    // FF FE seguito da altro e' UTF-16 LE (come prima); FE FF e' UTF-16 BE
    assert_eq!(
        Encoding::detect(&[0xFF, 0xFE, 0x41, 0x00]),
        Encoding::Utf16Le
    );
    assert_eq!(Encoding::detect(&[0xFF, 0xFE, 0x00]), Encoding::Utf16Le);
    assert_eq!(
        Encoding::detect(&[0xFE, 0xFF, 0x00, 0x41]),
        Encoding::Utf16Be
    );
    // `00 00 FE FF` incompleto non e' un BOM
    assert_ne!(Encoding::detect(&[0, 0, 0xFE]), BE);
}

#[test]
fn detect_without_bom_does_not_guess_utf32() {
    // testo ASCII in UTF-32 senza BOM: tre byte zero su quattro, e' "binario" per le regole di prima
    let mut testo = Vec::new();
    for c in "riga uno\nriga due\n".chars() {
        testo.extend(le(c as u32));
    }
    assert_eq!(Encoding::detect(&testo), Encoding::Utf8);
    let mut testo = Vec::new();
    for c in "riga uno\nriga due\n".chars() {
        testo.extend(be(c as u32));
    }
    assert_eq!(Encoding::detect(&testo), Encoding::Utf8);
}
