//! UTF-16 in `Encoding`: nomi, carattere per carattere, scrittura, decodifica e riconoscimento. Gli attesi sono scritti
//! a mano da ARCH (valori noti: U+20AC = 20AC, U+1F600 = D83D DE00) oppure vengono dalla libreria standard
//! (`encode_utf16`, `from_utf16_lossy`), mai dal codice sotto prova.

use mrp_core::charsets::CHARSETS;
use mrp_core::encoding::Encoding;
use mrp_core::newline::Newline;

const LE: Encoding = Encoding::Utf16Le;
const BE: Encoding = Encoding::Utf16Be;

fn enc_le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn enc_be(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_be_bytes).collect()
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
fn utf16_names_resolve() {
    for n in [
        "utf-16le",
        "UTF-16LE",
        "utf16le",
        "utf_16_le",
        "UTF-16-LE",
        " utf-16le ",
    ] {
        assert_eq!(Encoding::by_name(n), Some(LE), "{n}");
    }
    for n in ["utf-16be", "UTF-16BE", "utf16be", "utf_16_be", "UTF-16-BE"] {
        assert_eq!(Encoding::by_name(n), Some(BE), "{n}");
    }
    // da solo e' ambiguo (non dice l'endian)
    assert_eq!(Encoding::by_name("utf-16"), None);
    assert_eq!(Encoding::by_name("utf16"), None);
    assert_eq!(LE.name(), "utf-16le");
    assert_eq!(BE.name(), "utf-16be");
    assert!(LE.is_utf16() && BE.is_utf16());
    assert!(!LE.is_utf8() && !BE.is_utf8());
    assert!(!Encoding::Utf8.is_utf16());
    assert!(!Encoding::by_name("windows-1252").unwrap().is_utf16());
}

#[test]
fn all_lists_utf8_then_utf16_then_the_code_pages() {
    let all = Encoding::all();
    assert_eq!(all.len(), 5 + CHARSETS.len());
    assert_eq!(&all[..3], &[Encoding::Utf8, LE, BE]);
    for (i, e) in all.iter().enumerate().skip(5) {
        assert_eq!(e.name(), CHARSETS[i - 5].name);
    }
    let mut nomi: Vec<&str> = all.iter().map(|e| e.name()).collect();
    nomi.sort_unstable();
    nomi.dedup();
    assert_eq!(nomi.len(), all.len(), "nomi doppi");
    for e in &all {
        assert_eq!(Encoding::by_name(e.name()), Some(*e));
    }
}

#[test]
fn newline_and_unit_per_encoding() {
    assert_eq!(LE.newline(), Newline::Le16);
    assert_eq!(BE.newline(), Newline::Be16);
    assert_eq!(Encoding::Utf8.newline(), Newline::Byte);
    assert_eq!(
        Encoding::by_name("koi8-r").unwrap().newline(),
        Newline::Byte
    );
    assert_eq!((LE.unit(), BE.unit(), Encoding::Utf8.unit()), (2, 2, 1));
    // scrivere l'a-capo da' i byte dell'a-capo
    for e in [
        Encoding::Utf8,
        LE,
        BE,
        Encoding::by_name("windows-1252").unwrap(),
    ] {
        assert_eq!(e.encode('\n').unwrap(), e.newline().bytes(), "{}", e.name());
    }
}

// ---------- carattere per carattere ----------

#[test]
fn first_char_reads_units_and_surrogate_pairs() {
    assert_eq!(LE.first_char(&[]), None);
    assert_eq!(LE.first_char(&[0x61, 0x00]), Some(('a', 2)));
    assert_eq!(BE.first_char(&[0x00, 0x61]), Some(('a', 2)));
    // U+20AC EURO SIGN
    assert_eq!(
        LE.first_char(&[0xAC, 0x20, 0x61, 0x00]),
        Some(('\u{20ac}', 2))
    );
    assert_eq!(
        BE.first_char(&[0x20, 0xAC, 0x00, 0x61]),
        Some(('\u{20ac}', 2))
    );
    // U+1F600 = D83D DE00
    assert_eq!(
        LE.first_char(&[0x3D, 0xD8, 0x00, 0xDE]),
        Some(('\u{1f600}', 4))
    );
    assert_eq!(
        BE.first_char(&[0xD8, 0x3D, 0xDE, 0x00]),
        Some(('\u{1f600}', 4))
    );
    // il massimo: U+10FFFF = DBFF DFFF
    assert_eq!(
        LE.first_char(&[0xFF, 0xDB, 0xFF, 0xDF]),
        Some(('\u{10ffff}', 4))
    );
    // surrogato alto spaiato: seguito da una unita' qualunque, da un altro alto, o alla fine
    assert_eq!(
        LE.first_char(&[0x3D, 0xD8, 0x61, 0x00]),
        Some(('\u{fffd}', 2))
    );
    assert_eq!(
        LE.first_char(&[0x3D, 0xD8, 0x3D, 0xD8]),
        Some(('\u{fffd}', 2))
    );
    assert_eq!(LE.first_char(&[0x3D, 0xD8]), Some(('\u{fffd}', 2)));
    assert_eq!(
        BE.first_char(&[0xD8, 0x3D, 0x00, 0x61]),
        Some(('\u{fffd}', 2))
    );
    // ai bordi: DC00 da solo non e' un alto; un alto seguito da E000 (non e' un basso) resta spaiato
    assert_eq!(
        LE.first_char(&[0x00, 0xDC, 0x00, 0xDC]),
        Some(('\u{fffd}', 2))
    );
    assert_eq!(
        LE.first_char(&[0x3D, 0xD8, 0x00, 0xE0]),
        Some(('\u{fffd}', 2))
    );
    assert_eq!(
        BE.first_char(&[0xDC, 0x00, 0xDC, 0x00]),
        Some(('\u{fffd}', 2))
    );
    // surrogato basso spaiato
    assert_eq!(
        LE.first_char(&[0x00, 0xDE, 0x61, 0x00]),
        Some(('\u{fffd}', 2))
    );
    assert_eq!(BE.first_char(&[0xDE, 0x00]), Some(('\u{fffd}', 2)));
    // byte spaiato in coda
    assert_eq!(LE.first_char(&[0x61]), Some(('\u{fffd}', 1)));
    assert_eq!(BE.first_char(&[0x61]), Some(('\u{fffd}', 1)));
    // U+FEFF (BOM) e' un carattere come gli altri
    assert_eq!(LE.first_char(&[0xFF, 0xFE]), Some(('\u{feff}', 2)));
    assert_eq!(BE.first_char(&[0xFE, 0xFF]), Some(('\u{feff}', 2)));
}

#[test]
fn last_char_len_reads_from_the_end() {
    assert_eq!(LE.last_char_len(&[]), 0);
    assert_eq!(LE.last_char_len(&enc_le("a")), 2);
    assert_eq!(LE.last_char_len(&enc_le("a\u{20ac}")), 2);
    assert_eq!(LE.last_char_len(&enc_le("a\u{1f600}")), 4);
    assert_eq!(BE.last_char_len(&enc_be("a\u{1f600}")), 4);
    assert_eq!(BE.last_char_len(&enc_be("\u{1f600}")), 4);
    // un solo basso, o un basso senza alto davanti: 2
    assert_eq!(LE.last_char_len(&[0x00, 0xDE]), 2);
    assert_eq!(LE.last_char_len(&[0x61, 0x00, 0x00, 0xDE]), 2);
    // un solo alto in coda: 2
    assert_eq!(LE.last_char_len(&[0x3D, 0xD8]), 2);
    assert_eq!(BE.last_char_len(&[0x00, 0x61, 0xD8, 0x3D]), 2);
    // lunghezza dispari: l'ultimo byte e' il carattere spaiato
    assert_eq!(LE.last_char_len(&[0x61, 0x00, 0x62]), 1);
    assert_eq!(BE.last_char_len(&[0x62]), 1);
}

#[test]
fn walking_forward_and_backward_agree_on_random_text() {
    let alfabeto = [
        'a',
        '\n',
        'é',
        '\u{20ac}',
        '\u{0a41}',
        '\u{4100}',
        '\u{1f600}',
        '\u{10000}',
        '\u{10ffff}',
    ];
    let mut r = Lcg(3);
    for _ in 0..200 {
        let n = (r.next() % 20) as usize;
        let testo: String = (0..n)
            .map(|_| alfabeto[(r.next() % alfabeto.len() as u64) as usize])
            .collect();
        for (e, bytes) in [(LE, enc_le(&testo)), (BE, enc_be(&testo))] {
            // avanti: i caratteri e le loro lunghezze sono quelli del testo
            let mut i = 0;
            let mut visti = String::new();
            while i < bytes.len() {
                let (c, l) = e.first_char(&bytes[i..]).unwrap();
                assert_eq!(l, c.len_utf16() * 2);
                visti.push(c);
                i += l;
            }
            assert_eq!(visti, testo);
            // indietro: dall'ultimo al primo
            let mut fine = bytes.len();
            for c in testo.chars().rev() {
                let l = e.last_char_len(&bytes[..fine]);
                assert_eq!(l, c.len_utf16() * 2, "{testo:?}");
                fine -= l;
            }
            assert_eq!(fine, 0);
        }
    }
}

// ---------- scrittura e decodifica ----------

#[test]
fn encode_writes_units_and_pairs() {
    assert_eq!(LE.encode('a'), Some(vec![0x61, 0x00]));
    assert_eq!(BE.encode('a'), Some(vec![0x00, 0x61]));
    assert_eq!(LE.encode('\u{20ac}'), Some(vec![0xAC, 0x20]));
    assert_eq!(BE.encode('\u{20ac}'), Some(vec![0x20, 0xAC]));
    assert_eq!(LE.encode('\u{1f600}'), Some(vec![0x3D, 0xD8, 0x00, 0xDE]));
    assert_eq!(BE.encode('\u{1f600}'), Some(vec![0xD8, 0x3D, 0xDE, 0x00]));
    // in UTF-16 si scrive tutto, U+FFFD compreso
    assert_eq!(LE.encode('\u{fffd}'), Some(vec![0xFD, 0xFF]));
    assert_eq!(BE.encode('\u{fffd}'), Some(vec![0xFF, 0xFD]));
    assert_eq!(LE.encode('\n'), Some(vec![0x0A, 0x00]));
    assert_eq!(BE.encode('\n'), Some(vec![0x00, 0x0A]));
}

#[test]
fn encode_str_equals_the_standard_library() {
    let s = "Ciao, mondo\r\n\u{20ac} caff\u{e8} \u{1f600} \u{10ffff} \u{0a41}\u{0100}\n";
    assert_eq!(LE.encode_str(s), Ok(enc_le(s)));
    assert_eq!(BE.encode_str(s), Ok(enc_be(s)));
    assert_eq!(LE.encode_str(""), Ok(Vec::new()));
}

#[test]
fn decode_lossy_equals_the_standard_library_plus_one_replacement_for_an_odd_byte() {
    let testo = "ab\u{e8}\u{20ac}\u{1f600}\n\u{0a41}\u{0100}";
    assert_eq!(LE.decode_lossy(&enc_le(testo)), testo);
    assert_eq!(BE.decode_lossy(&enc_be(testo)), testo);
    // surrogati spaiati: ognuno diventa un U+FFFD
    assert_eq!(LE.decode_lossy(&[0x3D, 0xD8, 0x61, 0x00]), "\u{fffd}a");
    assert_eq!(LE.decode_lossy(&[0x00, 0xDE, 0x61, 0x00]), "\u{fffd}a");
    assert_eq!(BE.decode_lossy(&[0xD8, 0x3D]), "\u{fffd}");
    // byte spaiato in coda
    assert_eq!(LE.decode_lossy(&[0x61, 0x00, 0x62]), "a\u{fffd}");
    assert_eq!(BE.decode_lossy(&[0x62]), "\u{fffd}");
    assert_eq!(LE.decode_lossy(&[]), "");
    // unita' qualunque (surrogati compresi): come String::from_utf16_lossy
    let mut r = Lcg(8);
    for _ in 0..300 {
        let n = (r.next() % 12) as usize;
        let unita: Vec<u16> = (0..n)
            .map(|_| match r.next() % 4 {
                0 => 0xD800 + (r.next() % 0x400) as u16,
                1 => 0xDC00 + (r.next() % 0x400) as u16,
                _ => 0x20 + (r.next() % 0x60) as u16,
            })
            .collect();
        let le: Vec<u8> = unita.iter().flat_map(|u| u.to_le_bytes()).collect();
        let be: Vec<u8> = unita.iter().flat_map(|u| u.to_be_bytes()).collect();
        let atteso = String::from_utf16_lossy(&unita);
        assert_eq!(LE.decode_lossy(&le), atteso, "{unita:x?}");
        assert_eq!(BE.decode_lossy(&be), atteso, "{unita:x?}");
    }
}

// ---------- riconoscimento ----------

#[test]
fn detect_utf16_by_bom() {
    assert_eq!(Encoding::detect(&[0xFF, 0xFE, 0x61, 0x00, 0x62, 0x00]), LE);
    assert_eq!(Encoding::detect(&[0xFE, 0xFF, 0x00, 0x61, 0x00, 0x62]), BE);
    assert_eq!(Encoding::detect(&[0xFF, 0xFE]), LE);
    assert_eq!(Encoding::detect(&[0xFE, 0xFF]), BE);
    // FF FE 00 00 e' il BOM di UTF-32 LE: non e' UTF-16 (dalla E26 lo si riconosce come UTF-32 LE)
    assert_eq!(
        Encoding::detect(&[0xFF, 0xFE, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00]),
        Encoding::Utf32Le
    );
    // il BOM UTF-8 e' sempre UTF-8
    assert_eq!(Encoding::detect(b"\xef\xbb\xbfabc"), Encoding::Utf8);
}

#[test]
fn detect_utf16_without_bom_by_the_zero_bytes() {
    let testo = "Ciao mondo, questa \u{e8} una riga di testo.\nSeconda riga.\n";
    assert_eq!(Encoding::detect(&enc_le(testo)), LE);
    assert_eq!(Encoding::detect(&enc_be(testo)), BE);
    // campione tagliato a lunghezza dispari
    let mut le = enc_le(testo);
    le.push(0x61);
    assert_eq!(Encoding::detect(&le), LE);
    let mut be = enc_be(testo);
    be.pop();
    assert_eq!(Encoding::detect(&be), BE);
    // un carattere con byte alto 0 nella parita' sbagliata (U+0100 = 00 01 in LE) non basta a cambiare idea
    let con_a_macron = format!("{}\u{100}{}", "x".repeat(60), "y".repeat(30));
    assert_eq!(Encoding::detect(&enc_le(&con_a_macron)), LE);
    assert_eq!(Encoding::detect(&enc_be(&con_a_macron)), BE);
}

#[test]
fn detect_does_not_take_binary_or_short_data_for_utf16() {
    // troppo corto (meno di 8 byte) e senza BOM
    assert_eq!(Encoding::detect(&enc_le("ab")), Encoding::Utf8);
    assert_eq!(Encoding::detect(b"a\x00b\xe8 c"), Encoding::Utf8);
    // tutti zeri: i NUL stanno in tutte e due le parita'
    assert_eq!(Encoding::detect(&[0u8; 64]), Encoding::Utf8);
    // un'intestazione binaria: zeri da entrambe le parti
    let mz = b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00\xff\xff\x00\x00";
    assert_eq!(Encoding::detect(mz), Encoding::Utf8);
    // testo ASCII senza NUL: come prima
    assert_eq!(Encoding::detect(b"plain ascii text\n"), Encoding::Utf8);
    // i casi di sempre non cambiano
    assert_eq!(
        Encoding::detect(b"caff\xe8 ok"),
        Encoding::by_name("windows-1252").unwrap()
    );
}
