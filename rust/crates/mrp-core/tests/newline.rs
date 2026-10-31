//! Test di `newline`: gli a-capo si trovano solo se ALLINEATI. L'oracolo e' un confronto a coppie scritto qui, senza
//! `memchr`, e fatti noti sui byte di caratteri veri.

use mrp_core::newline::Newline;

/// Oracolo: le posizioni degli a-capo guardando una unita' alla volta, partendo da 0.
fn modello(nl: Newline, hay: &[u8]) -> Vec<usize> {
    let u = nl.unit();
    let mut v = Vec::new();
    let mut i = 0;
    while i + u <= hay.len() {
        let ok = match nl {
            Newline::Byte => hay[i] == 0x0A,
            Newline::Le16 => hay[i] == 0x0A && hay[i + 1] == 0x00,
            Newline::Be16 => hay[i] == 0x00 && hay[i + 1] == 0x0A,
            Newline::Le32 => hay[i..i + 4] == [0x0A, 0, 0, 0],
            Newline::Be32 => hay[i..i + 4] == [0, 0, 0, 0x0A],
        };
        if ok {
            v.push(i);
        }
        i += u;
    }
    v
}

/// Generatore pseudo-casuale con seme fisso (nessuna dipendenza).
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

#[test]
fn unit_and_bytes() {
    assert_eq!(Newline::Byte.unit(), 1);
    assert_eq!(Newline::Le16.unit(), 2);
    assert_eq!(Newline::Be16.unit(), 2);
    assert_eq!(Newline::Byte.bytes(), &[0x0A]);
    assert_eq!(Newline::Le16.bytes(), &[0x0A, 0x00]);
    assert_eq!(Newline::Be16.bytes(), &[0x00, 0x0A]);
    assert_eq!(Newline::default(), Newline::Byte);
    assert!(Newline::Le16.is_newline(&[0x0A, 0x00]));
    assert!(!Newline::Le16.is_newline(&[0x00, 0x0A]));
    assert!(Newline::Be16.is_newline(&[0x00, 0x0A]));
    assert!(!Newline::Be16.is_newline(&[0x0A, 0x00]));
    assert!(Newline::Byte.is_newline(b"\n"));
    assert!(!Newline::Byte.is_newline(b"a"));
}

#[test]
fn byte_form_is_plain_newline_search() {
    let h = b"ab\ncd\n\nxyz";
    assert_eq!(
        Newline::Byte.positions(h).collect::<Vec<_>>(),
        vec![2, 5, 6]
    );
    assert_eq!(Newline::Byte.count(h), 3);
    assert_eq!(Newline::Byte.find(h), Some(2));
    assert_eq!(Newline::Byte.rfind(h), Some(6));
    assert_eq!(Newline::Byte.find(b"abc"), None);
    assert_eq!(Newline::Byte.rfind(b""), None);
    assert!(Newline::Byte.ends_with_newline(b"ab\n"));
    assert!(!Newline::Byte.ends_with_newline(b"ab"));
    assert!(!Newline::Byte.ends_with_newline(b""));
}

#[test]
fn utf16_plain_text() {
    // "a\nb\n" in UTF-16 LE e BE
    let le = [0x61, 0, 0x0A, 0, 0x62, 0, 0x0A, 0];
    let be = [0, 0x61, 0, 0x0A, 0, 0x62, 0, 0x0A];
    assert_eq!(Newline::Le16.positions(&le).collect::<Vec<_>>(), vec![2, 6]);
    assert_eq!(Newline::Be16.positions(&be).collect::<Vec<_>>(), vec![2, 6]);
    assert_eq!(Newline::Le16.count(&le), 2);
    assert_eq!(Newline::Be16.count(&be), 2);
    assert_eq!(Newline::Le16.find(&le), Some(2));
    assert_eq!(Newline::Le16.rfind(&le), Some(6));
    assert_eq!(Newline::Be16.rfind(&be), Some(6));
    assert!(Newline::Le16.ends_with_newline(&le));
    assert!(Newline::Be16.ends_with_newline(&be));
    // la forma sbagliata non trova nulla
    assert_eq!(Newline::Be16.count(&le), 0);
    assert_eq!(Newline::Le16.count(&be), 0);
}

#[test]
fn utf16_chars_that_contain_the_byte_0a_are_not_newlines() {
    // U+010A (LATIN CAPITAL LETTER C WITH DOT ABOVE) = 0A 01 in LE, 01 0A in BE
    let le = [0x0A, 0x01, 0x0A, 0x01];
    assert_eq!(Newline::Le16.count(&le), 0);
    let be = [0x01, 0x0A, 0x01, 0x0A];
    assert_eq!(Newline::Be16.count(&be), 0);
    // U+0A00 = 00 0A in LE (0A in posizione dispari), 0A 00 in BE
    let le = [0x00, 0x0A, 0x00, 0x0A];
    assert_eq!(Newline::Le16.count(&le), 0);
    let be = [0x0A, 0x00, 0x0A, 0x00];
    assert_eq!(Newline::Be16.count(&be), 0);
}

#[test]
fn utf16_pair_that_straddles_two_units_is_not_a_newline() {
    // LE: U+0A41 = 41 0A, poi U+0100 = 00 01: i byte 1 e 2 sono `0A 00` ma sfasati di uno
    let le = [0x41, 0x0A, 0x00, 0x01];
    assert_eq!(Newline::Le16.count(&le), 0);
    assert_eq!(Newline::Le16.find(&le), None);
    assert_eq!(Newline::Le16.rfind(&le), None);
    // BE: U+4100 = 41 00, poi U+0A41 = 0A 41: i byte 1 e 2 sono `00 0A` ma sfasati di uno
    let be = [0x41, 0x00, 0x0A, 0x41];
    assert_eq!(Newline::Be16.count(&be), 0);
    assert_eq!(Newline::Be16.find(&be), None);
    assert_eq!(Newline::Be16.rfind(&be), None);
    // e un a-capo vero subito dopo resta trovato, alla sua posizione allineata
    let le = [0x41, 0x0A, 0x00, 0x01, 0x0A, 0x00];
    assert_eq!(Newline::Le16.positions(&le).collect::<Vec<_>>(), vec![4]);
    let be = [0x41, 0x00, 0x0A, 0x41, 0x00, 0x0A];
    assert_eq!(Newline::Be16.positions(&be).collect::<Vec<_>>(), vec![4]);
}

#[test]
fn utf16_odd_length_tail_is_never_a_newline() {
    // LE: l'ultimo byte 0A e' spaiato
    assert_eq!(Newline::Le16.count(&[0x61, 0x00, 0x0A]), 0);
    assert!(!Newline::Le16.ends_with_newline(&[0x61, 0x00, 0x0A]));
    // BE: lo 00 spaiato in coda non forma una unita'
    assert_eq!(Newline::Be16.count(&[0x00, 0x0A, 0x00]), 1);
    assert!(!Newline::Be16.ends_with_newline(&[0x00, 0x0A, 0x00]));
    // un a-capo vero seguito da un byte spaiato
    assert_eq!(
        Newline::Le16
            .positions(&[0x0A, 0x00, 0x7A])
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert!(!Newline::Le16.ends_with_newline(&[0x0A, 0x00, 0x7A]));
    // fetta vuota o troppo corta
    for nl in [Newline::Le16, Newline::Be16] {
        assert_eq!(nl.count(&[]), 0);
        assert_eq!(nl.count(&[0x0A]), 0);
        assert_eq!(nl.find(&[]), None);
        assert!(!nl.ends_with_newline(&[]));
        assert!(!nl.ends_with_newline(&[0x0A]));
    }
}

#[test]
fn random_bytes_agree_with_the_pairwise_model() {
    // byte presi da un alfabeto piccolo, per avere spesso 0x00 e 0x0A vicini e in ogni parita'
    let mut r = Lcg(12345);
    let alfabeto = [0x00u8, 0x0A, 0x01, 0x41, 0x00, 0x0A];
    for len in 0..70 {
        for _ in 0..40 {
            let hay: Vec<u8> = (0..len)
                .map(|_| alfabeto[(r.next() % alfabeto.len() as u64) as usize])
                .collect();
            for nl in [Newline::Byte, Newline::Le16, Newline::Be16] {
                let atteso = modello(nl, &hay);
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

#[test]
fn text_encoded_by_std_has_the_expected_newline_positions() {
    // testo con a-capo, CRLF, lettere con 0x0A nel byte, caratteri sopra il piano base (coppie surrogate)
    let testo = "riga uno\r\nĊ\u{0A41}\u{0100}\nfine \u{1F600}\n\u{0A00}x\u{4100}\u{0A41}";
    let le: Vec<u8> = testo.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let be: Vec<u8> = testo.encode_utf16().flat_map(u16::to_be_bytes).collect();
    // posizione attesa di ogni '\n' = 2 * (indice dell'unita' UTF-16 in cui compare), dal testo, non dai byte
    let attese: Vec<usize> = testo
        .encode_utf16()
        .enumerate()
        .filter(|&(_, u)| u == 0x000A)
        .map(|(i, _)| 2 * i)
        .collect();
    assert_eq!(attese.len(), 3);
    assert_eq!(Newline::Le16.positions(&le).collect::<Vec<_>>(), attese);
    assert_eq!(Newline::Be16.positions(&be).collect::<Vec<_>>(), attese);
}
