//! Test delle codifiche (`encoding`). Gli attesi sono scritti a mano da ARCH (fatti noti sulle code page, controllati
//! su una fonte indipendente) e mai calcolati dal codice.

use mrp_core::charsets::CHARSETS;
use mrp_core::encoding::Encoding;

fn enc(name: &str) -> Encoding {
    Encoding::by_name(name).unwrap_or_else(|| panic!("codifica sconosciuta: {name}"))
}

/// Il carattere del byte `b` nella codifica `e`.
fn ch(e: Encoding, b: u8) -> char {
    e.first_char(&[b]).unwrap().0
}

// ---------- nomi ----------

#[test]
fn names_resolve_without_case_and_with_underscore_for_dash() {
    assert_eq!(enc("utf-8"), Encoding::Utf8);
    assert_eq!(enc("UTF-8"), Encoding::Utf8);
    assert_eq!(enc("utf8"), Encoding::Utf8);
    assert_eq!(enc("UTF_8"), Encoding::Utf8);
    assert_eq!(enc("windows-1252"), enc("CP1252"));
    assert_eq!(enc("windows_1252"), enc("windows-1252"));
    assert_eq!(enc("ansi"), enc("windows-1252"));
    assert_eq!(enc("1252"), enc("windows-1252"));
    assert_eq!(enc("latin1"), enc("iso-8859-1"));
    assert_eq!(enc("Latin-1"), enc("ISO_8859_1"));
    assert_eq!(enc("iso8859-1"), enc("iso-8859-1"));
    assert_eq!(enc("latin9"), enc("iso-8859-15"));
    assert_eq!(enc("437"), enc("ibm437"));
    assert_eq!(enc("cp437"), enc("IBM437"));
    assert_eq!(enc("koi8r"), enc("koi8-r"));
    assert_eq!(enc("mac-roman"), enc("macintosh"));
    assert_ne!(enc("windows-1252"), enc("iso-8859-1"));
}

#[test]
fn unknown_names_are_none() {
    for n in ["", "klingon", "utf-16", "utf-32", "windows-9999"] {
        assert_eq!(Encoding::by_name(n), None, "{n}");
    }
}

#[test]
fn all_lists_utf8_then_utf16_then_every_code_page_with_distinct_names() {
    let all = Encoding::all();
    assert_eq!(
        all.len(),
        5 + CHARSETS.len() + mrp_core::multibyte::MULTI.len()
    );
    assert_eq!(all[0], Encoding::Utf8);
    assert_eq!(Encoding::Utf8.name(), "utf-8");
    let mut names: Vec<&str> = all.iter().map(|e| e.name()).collect();
    for (i, e) in all.iter().enumerate().skip(5).take(CHARSETS.len()) {
        assert_eq!(e.name(), CHARSETS[i - 5].name);
    }
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), all.len(), "nomi doppi");
    for e in &all {
        assert_eq!(Encoding::by_name(e.name()), Some(*e), "{}", e.name());
        assert_eq!(e.is_utf8(), *e == Encoding::Utf8);
    }
}

#[test]
fn every_alias_resolves_to_its_own_code_page() {
    for (i, cs) in CHARSETS.iter().enumerate() {
        let e = Encoding::all()[i + 5];
        assert_eq!(e.name(), cs.name);
        for a in cs.aliases {
            assert_eq!(Encoding::by_name(a), Some(e), "alias {a}");
        }
    }
}

#[test]
fn default_encoding_is_utf8() {
    assert_eq!(Encoding::default(), Encoding::Utf8);
}

// ---------- code page a byte singolo ----------

#[test]
fn well_known_bytes_of_the_code_pages() {
    // (codifica, byte, carattere): fatti noti, uno per ogni famiglia
    let casi: &[(&str, u8, char)] = &[
        ("windows-1252", 0x80, '\u{20ac}'),
        ("windows-1252", 0x85, '\u{2026}'),
        ("windows-1252", 0x93, '\u{201c}'),
        ("windows-1252", 0xe9, '\u{e9}'),
        ("windows-1252", 0xff, '\u{ff}'),
        ("iso-8859-1", 0xa4, '\u{a4}'),
        ("iso-8859-1", 0xe9, '\u{e9}'),
        ("iso-8859-15", 0xa4, '\u{20ac}'),
        ("iso-8859-15", 0xa6, '\u{160}'),
        ("iso-8859-5", 0xb0, '\u{410}'),
        ("iso-8859-7", 0xe1, '\u{3b1}'),
        ("windows-1250", 0x8a, '\u{160}'),
        ("windows-1250", 0xe8, '\u{10d}'),
        ("windows-1251", 0xc0, '\u{410}'),
        ("windows-1251", 0xe0, '\u{430}'),
        ("ibm437", 0x80, '\u{c7}'),
        ("ibm437", 0xb0, '\u{2591}'),
        ("ibm437", 0xc9, '\u{2554}'),
        ("ibm437", 0xcd, '\u{2550}'),
        ("ibm437", 0xbb, '\u{2557}'),
        ("ibm437", 0xe1, '\u{df}'),
        ("ibm437", 0xfb, '\u{221a}'),
        ("ibm850", 0x9b, '\u{f8}'),
        ("ibm850", 0xd5, '\u{131}'),
        ("ibm858", 0xd5, '\u{20ac}'),
        ("ibm866", 0x80, '\u{410}'),
        ("ibm866", 0xa0, '\u{430}'),
        ("koi8-r", 0xc1, '\u{430}'),
        ("koi8-r", 0xe1, '\u{410}'),
        ("macintosh", 0x80, '\u{c4}'),
        ("macintosh", 0x8e, '\u{e9}'),
        ("macintosh", 0xa5, '\u{2022}'),
    ];
    for (nome, b, c) in casi {
        let e = enc(nome);
        assert_eq!(ch(e, *b), *c, "{nome} {b:#04x}");
        assert_eq!(
            e.first_char(&[*b, b'x']),
            Some((*c, 1)),
            "{nome} {b:#04x} seguito da x"
        );
        assert_eq!(e.encode(*c), Some(vec![*b]), "{nome} {c:?}");
    }
}

#[test]
fn undefined_bytes_are_replacement_characters_and_cannot_be_written() {
    // ibm857 non definisce 0xD5, 0xE7 e 0xF2: sono U+FFFD e U+FFFD non si scrive
    let w = enc("ibm857");
    for b in [0xd5u8, 0xe7, 0xf2] {
        assert_eq!(ch(w, b), '\u{fffd}', "{b:#04x}");
    }
    assert_eq!(w.encode('\u{fffd}'), None);
    assert_eq!(enc("windows-1252").encode('\u{fffd}'), None);
}

#[test]
fn windows_125x_undefined_c1_bytes_are_the_c1_controls() {
    // come Windows e i browser: 0x81, 0x8D, 0x8F, 0x90, 0x9D di windows-1252 sono U+0081 ecc., e si riscrivono uguali
    // (prima erano U+FFFD: un file con quei byte, salvato dopo una modifica, li perdeva)
    let w = enc("windows-1252");
    for b in [0x81u8, 0x8d, 0x8f, 0x90, 0x9d] {
        assert_eq!(ch(w, b), char::from(b), "{b:#04x}");
        assert_eq!(w.encode(char::from(b)), Some(vec![b]), "{b:#04x}");
    }
    // vale per tutte le windows-125x nella zona 0x80-0x9F; fuori da li' un byte indefinito resta U+FFFD
    assert_eq!(ch(enc("windows-1255"), 0x81), '\u{81}');
    assert_eq!(ch(enc("windows-1255"), 0xd9), '\u{fffd}');
}

#[test]
fn latin1_c1_range_is_controls_not_windows_1252_punctuation() {
    assert_eq!(ch(enc("iso-8859-1"), 0x85), '\u{85}');
    assert_eq!(ch(enc("windows-1252"), 0x85), '\u{2026}');
}

#[test]
fn every_code_page_keeps_ascii_and_round_trips_every_defined_byte() {
    for e in Encoding::all().into_iter().skip(5).take(CHARSETS.len()) {
        for b in 0u8..=255 {
            let (c, n) = e.first_char(&[b]).unwrap();
            assert_eq!(n, 1, "{} {b:#04x}", e.name());
            if b < 0x80 {
                assert_eq!(c, b as char, "{} ASCII {b:#04x}", e.name());
            }
            if c != '\u{fffd}' {
                assert_eq!(e.encode(c), Some(vec![b]), "{} {b:#04x} {c:?}", e.name());
            } else {
                assert!(b >= 0x80, "{} byte ASCII non definito", e.name());
            }
        }
    }
}

#[test]
fn single_byte_first_and_last_char() {
    let e = enc("windows-1252");
    assert_eq!(e.first_char(b""), None);
    assert_eq!(e.first_char(&[0xc3, 0xa9]), Some(('\u{c3}', 1)));
    assert_eq!(e.last_char_len(b""), 0);
    assert_eq!(e.last_char_len(&[0xc3, 0xa9]), 1);
    assert_eq!(e.last_char_len(b"abc"), 1);
}

#[test]
fn single_byte_encode_and_decode_strings() {
    let w = enc("windows-1252");
    assert_eq!(w.encode('a'), Some(vec![0x61]));
    assert_eq!(w.encode('\n'), Some(vec![0x0a]));
    assert_eq!(w.encode('\u{20ac}'), Some(vec![0x80]));
    assert_eq!(w.encode('\u{3a9}'), None); // Omega non c'e' in windows-1252
    assert_eq!(w.encode('\u{4e2d}'), None);
    assert_eq!(
        w.encode_str("caff\u{e8} \u{20ac}\n"),
        Ok(b"caff\xe8 \x80\n".to_vec())
    );
    assert_eq!(w.encode_str("a \u{3a9} b \u{4e2d}"), Err('\u{3a9}'));
    assert_eq!(w.encode_str(""), Ok(Vec::new()));
    assert_eq!(w.decode_lossy(b"caff\xe8 \x80\n"), "caff\u{e8} \u{20ac}\n");
    assert_eq!(w.decode_lossy(b"a\x81b"), "a\u{81}b");
    assert_eq!(
        enc("ibm437").decode_lossy(b"\xc9\xcd\xbb"),
        "\u{2554}\u{2550}\u{2557}"
    );
    assert_eq!(enc("iso-8859-15").encode_str("\u{20ac}"), Ok(vec![0xa4]));
}

// ---------- UTF-8 ----------

#[test]
fn utf8_first_char() {
    let u = Encoding::Utf8;
    assert_eq!(u.first_char(b""), None);
    assert_eq!(u.first_char(b"abc"), Some(('a', 1)));
    assert_eq!(u.first_char("\u{e9}x".as_bytes()), Some(('\u{e9}', 2)));
    assert_eq!(u.first_char("\u{20ac}".as_bytes()), Some(('\u{20ac}', 3)));
    assert_eq!(u.first_char("\u{1f600}".as_bytes()), Some(('\u{1f600}', 4)));
    // byte non valido, sequenza troncata, continuazione isolata, sovralunga: un byte = U+FFFD
    assert_eq!(u.first_char(&[0xe9, b'a']), Some(('\u{fffd}', 1)));
    assert_eq!(u.first_char(&[0xc3]), Some(('\u{fffd}', 1)));
    assert_eq!(u.first_char(&[0xe2, 0x82]), Some(('\u{fffd}', 1)));
    assert_eq!(u.first_char(&[0xa9, 0xa9]), Some(('\u{fffd}', 1)));
    assert_eq!(u.first_char(&[0xc0, 0x80]), Some(('\u{fffd}', 1)));
    assert_eq!(u.first_char(&[0xff]), Some(('\u{fffd}', 1)));
}

#[test]
fn utf8_last_char_len() {
    let u = Encoding::Utf8;
    assert_eq!(u.last_char_len(b""), 0);
    assert_eq!(u.last_char_len(b"ab"), 1);
    assert_eq!(u.last_char_len("a\u{e9}".as_bytes()), 2);
    assert_eq!(u.last_char_len("a\u{20ac}".as_bytes()), 3);
    assert_eq!(u.last_char_len("a\u{1f600}".as_bytes()), 4);
    assert_eq!(u.last_char_len(&[b'a', 0xe9]), 1);
    assert_eq!(u.last_char_len(&[0xa9]), 1);
    assert_eq!(u.last_char_len(&[0xc3, 0xa9, 0xa9]), 1);
}

#[test]
fn utf8_encode_and_decode() {
    let u = Encoding::Utf8;
    assert_eq!(u.encode('a'), Some(vec![0x61]));
    assert_eq!(u.encode('\u{e9}'), Some(vec![0xc3, 0xa9]));
    assert_eq!(u.encode('\u{20ac}'), Some(vec![0xe2, 0x82, 0xac]));
    assert_eq!(u.encode('\u{1f600}'), Some(vec![0xf0, 0x9f, 0x98, 0x80]));
    assert_eq!(u.encode('\u{fffd}'), Some(vec![0xef, 0xbf, 0xbd]));
    assert_eq!(
        u.encode_str("a\u{e9}\u{4e2d}"),
        Ok("a\u{e9}\u{4e2d}".as_bytes().to_vec())
    );
    assert_eq!(u.decode_lossy("a\u{e9}".as_bytes()), "a\u{e9}");
    assert_eq!(u.decode_lossy(&[b'a', 0xe9, b'b']), "a\u{fffd}b");
}

// ---------- riconoscimento ----------

#[test]
fn detect_utf8_cases() {
    let w = enc("windows-1252");
    assert_eq!(Encoding::detect(b""), Encoding::Utf8);
    assert_eq!(Encoding::detect(b"plain ascii\r\n"), Encoding::Utf8);
    assert_eq!(
        Encoding::detect("caff\u{e8} \u{20ac} \u{1f600}\n".as_bytes()),
        Encoding::Utf8
    );
    // il BOM UTF-8 e' UTF-8 valido
    assert_eq!(Encoding::detect(b"\xef\xbb\xbfabc"), Encoding::Utf8);
    // campione tagliato nel mezzo di un carattere multibyte: e' ancora UTF-8
    assert_eq!(Encoding::detect(b"abc\xc3"), Encoding::Utf8);
    assert_eq!(Encoding::detect(b"abc\xe2\x82"), Encoding::Utf8);
    assert_eq!(Encoding::detect(b"abc\xf0\x9f\x98"), Encoding::Utf8);
    // un byte NUL: binario, non si indovina (un BOM UTF-16 invece si riconosce: vedi encoding_utf16.rs)
    assert_eq!(Encoding::detect(b"a\x00b\xe8 c"), Encoding::Utf8);
    assert_eq!(Encoding::detect(b"\xff\xfea\x00b\x00"), Encoding::Utf16Le);
    // ambiguita' dichiarata: un solo byte di testa in coda e' "sequenza incompleta", non Windows-1252
    assert_eq!(Encoding::detect(b"caff\xe8"), Encoding::Utf8);
    assert_ne!(Encoding::detect(b"caff\xe8 ok"), Encoding::Utf8);
    assert_eq!(Encoding::detect(b"caff\xe8 ok"), w);
}

#[test]
fn detect_falls_back_to_windows_1252() {
    let w = enc("windows-1252");
    assert_eq!(Encoding::detect(b"\xe8 caff"), w);
    assert_eq!(Encoding::detect(b"caff\xe8\n\x80 e"), w);
    assert_eq!(Encoding::detect(b"\xc3 x"), w); // testa seguita da non continuazione
    assert_eq!(Encoding::detect(b"\xc0\x80 x"), w); // sovralunga
    assert_eq!(Encoding::detect(b"abc\xff"), w); // 0xFF non inizia mai un carattere
    assert_eq!(Encoding::detect(b"abc\xa9 d"), w); // continuazione isolata
}
