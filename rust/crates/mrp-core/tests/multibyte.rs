//! Le codifiche multibyte dell'Asia orientale (E27): nomi, ordine di `all()`, `first_char`, `last_char_len`,
//! `char_start`, `encode`, `decode_lossy`. Scritti da ARCH. Gli attesi vengono da DUE fonti che non sono `encoding_rs`
//! (la crate sotto test): i codec di Python e `Encoding.GetString` di .NET, incrociati da `tools/gen_multibyte_oracle.*`
//! in `data/multibyte_oracle.txt` (solo le sequenze su cui concordano), e da fatti noti scritti a mano e riletti con
//! Python.

use mrp_core::charsets::CHARSETS;
use mrp_core::encoding::Encoding;
use mrp_core::multibyte::{BACK_WINDOW, MULTI};

const ORACOLO: &str = include_str!("data/multibyte_oracle.txt");

fn enc(nome: &str) -> Encoding {
    Encoding::by_name(nome).unwrap_or_else(|| panic!("codifica {nome} assente"))
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

fn fnv(h: u64, byte: u8) -> u64 {
    (h ^ u64::from(byte)).wrapping_mul(0x100000001B3)
}

// ---------- nomi ----------

#[test]
fn names_and_aliases() {
    let casi: &[(&str, &str)] = &[
        ("shift-jis", "shift-jis"),
        ("Shift_JIS", "shift-jis"),
        ("sjis", "shift-jis"),
        ("windows-31j", "shift-jis"),
        ("CP932", "shift-jis"),
        ("euc-jp", "euc-jp"),
        ("EUC_JP", "euc-jp"),
        ("gbk", "gbk"),
        ("cp936", "gbk"),
        ("GB2312", "gbk"),
        ("gb18030", "gb18030"),
        ("big5", "big5"),
        ("Big5-HKSCS", "big5"),
        ("cp950", "big5"),
        ("euc-kr", "euc-kr"),
        ("cp949", "euc-kr"),
        ("UHC", "euc-kr"),
        ("KS_C_5601-1987", "euc-kr"),
    ];
    for (alias, canonico) in casi {
        let e = enc(alias);
        assert_eq!(e.name(), *canonico, "{alias}");
        assert_eq!(Encoding::by_name(e.name()), Some(e));
        assert!(e.is_multibyte(), "{alias}");
        assert!(!e.is_utf8() && !e.is_utf16() && !e.is_utf32());
        assert_eq!(e.unit(), 1);
    }
    // nomi che non ci sono (le codifiche con stato non si fanno)
    for n in ["iso-2022-jp", "hz", "shift-jis2", "big6", "euc"] {
        assert_eq!(Encoding::by_name(n), None, "{n}");
    }
}

#[test]
fn all_ends_with_the_multibyte_encodings_after_the_code_pages() {
    let all = Encoding::all();
    assert_eq!(all.len(), 5 + CHARSETS.len() + MULTI.len());
    for (k, m) in MULTI.iter().enumerate() {
        let e = all[5 + CHARSETS.len() + k];
        assert_eq!(e, Encoding::Multi(k));
        assert_eq!(e.name(), m.name);
    }
    let mut nomi: Vec<&str> = all.iter().map(|e| e.name()).collect();
    nomi.sort_unstable();
    nomi.dedup();
    assert_eq!(nomi.len(), all.len(), "nomi doppi");
}

#[test]
fn no_alias_belongs_to_two_encodings() {
    let mut visti: Vec<&str> = Vec::new();
    for cs in CHARSETS.iter() {
        visti.push(cs.name);
        visti.extend(cs.aliases.iter().copied());
    }
    for m in MULTI.iter() {
        visti.push(m.name);
        visti.extend(m.aliases.iter().copied());
    }
    for n in ["utf-8", "utf8", "utf-16le", "utf-32le"] {
        visti.push(n);
    }
    let totale = visti.len();
    visti.sort_unstable();
    visti.dedup();
    assert_eq!(visti.len(), totale, "un nome o alias e' usato due volte");
}

// ---------- le tabelle contro l'oracolo a due fonti ----------

#[test]
fn every_sequence_both_oracles_agree_on_decodes_to_that_character() {
    let mut righe = 0;
    let mut sequenze = 0u64;
    let mut per_nome: Vec<&str> = Vec::new();
    for riga in ORACOLO.lines().filter(|l| !l.is_empty()) {
        let campi: Vec<&str> = riga.split(' ').collect();
        assert_eq!(campi.len(), 4, "{riga}");
        let (nome, prefisso, maschera, hash) = (campi[0], campi[1], campi[2], campi[3]);
        let e = enc(nome);
        let prefisso: Vec<u8> = if prefisso == "-" {
            Vec::new()
        } else {
            (0..prefisso.len() / 2)
                .map(|i| u8::from_str_radix(&prefisso[2 * i..2 * i + 2], 16).unwrap())
                .collect()
        };
        assert_eq!(maschera.len(), 64, "{riga}");
        let atteso = u64::from_str_radix(hash, 16).unwrap();
        let mut h = 0xCBF29CE484222325u64;
        let mut nostre: Vec<String> = Vec::new();
        for ultimo in 0..=255u8 {
            // il bit `ultimo` della maschera (esadecimale, il byte 255 e' il bit piu' significativo)
            let cifra = u32::from_str_radix(
                &maschera[63 - usize::from(ultimo) / 4..64 - usize::from(ultimo) / 4],
                16,
            )
            .unwrap();
            if (cifra >> (ultimo % 4)) & 1 == 0 {
                continue;
            }
            let mut b = prefisso.clone();
            b.push(ultimo);
            let (c, n) = e
                .first_char(&b)
                .unwrap_or_else(|| panic!("{nome} {b:02x?}: nessun carattere"));
            assert_eq!(n, b.len(), "{nome} {b:02x?}: lunghezza");
            assert_ne!(c, '\u{fffd}', "{nome} {b:02x?}: non valido");
            h = fnv(h, ultimo);
            for k in [24, 16, 8, 0] {
                h = fnv(h, ((c as u32) >> k) as u8);
            }
            nostre.push(format!("{ultimo:02X}=U+{:04X}", c as u32));
            sequenze += 1;
        }
        assert_eq!(
            h, atteso,
            "{nome} prefisso {prefisso:02x?}: i punti di codice non sono quelli dell'oracolo \
             (python tools/gen_multibyte_oracle.py <cartella> --dump {nome} <prefisso> per vederli); nostri: {}",
            nostre.join(" ")
        );
        righe += 1;
        if !per_nome.contains(&nome) {
            per_nome.push(nome);
        }
    }
    assert!(
        righe > 1000 && sequenze > 90_000,
        "{righe} righe, {sequenze} sequenze"
    );
    assert_eq!(per_nome.len(), MULTI.len(), "ogni codifica ha l'oracolo");
}

// ---------- fatti noti ----------

#[test]
fn well_known_characters_decode_and_encode() {
    // (codifica, carattere, byte): riletti con i codec di Python
    let casi: &[(&str, char, &[u8])] = &[
        ("shift-jis", 'あ', &[0x82, 0xA0]),
        ("shift-jis", '表', &[0x95, 0x5C]),
        ("shift-jis", 'ソ', &[0x83, 0x5C]),
        ("shift-jis", '能', &[0x94, 0x5C]),
        ("shift-jis", '日', &[0x93, 0xFA]),
        ("shift-jis", '\u{FF71}', &[0xB1]),
        ("euc-jp", 'あ', &[0xA4, 0xA2]),
        ("euc-jp", '日', &[0xC6, 0xFC]),
        ("euc-jp", '\u{FF71}', &[0x8E, 0xB1]),
        ("gbk", '中', &[0xD6, 0xD0]),
        ("gbk", '表', &[0xB1, 0xED]),
        ("gb18030", '\u{1F600}', &[0x94, 0x39, 0xFC, 0x36]),
        ("gb18030", '\u{80}', &[0x81, 0x30, 0x81, 0x30]),
        ("gb18030", '€', &[0xA2, 0xE3]),
        ("big5", '中', &[0xA4, 0xA4]),
        ("big5", '日', &[0xA4, 0xE9]),
        ("euc-kr", '한', &[0xC7, 0xD1]),
        ("euc-kr", '가', &[0xB0, 0xA1]),
        ("euc-kr", '日', &[0xEC, 0xED]),
    ];
    for &(nome, c, bytes) in casi {
        let e = enc(nome);
        assert_eq!(e.first_char(bytes), Some((c, bytes.len())), "{nome} {c:?}");
        assert_eq!(e.encode(c), Some(bytes.to_vec()), "{nome} {c:?}");
        // seguito da altro, il primo carattere e' lo stesso
        let mut piu = bytes.to_vec();
        piu.extend(b"xyz");
        assert_eq!(
            e.first_char(&piu),
            Some((c, bytes.len())),
            "{nome} {c:?} + xyz"
        );
        assert_eq!(e.decode_lossy(bytes), c.to_string());
        assert_eq!(e.last_char_len(bytes), bytes.len(), "{nome} {c:?}");
    }
    // EUC-JP a 3 byte (JIS X 0212): si legge, non si scrive
    let ej = enc("euc-jp");
    assert_eq!(
        ej.first_char(&[0x8F, 0xB0, 0xA1, 0x41]),
        Some(('\u{4E02}', 3))
    );
    assert_eq!(ej.last_char_len(&[0x8F, 0xB0, 0xA1]), 3);
    // l'ASCII e' il suo byte in tutte, compresi 0x5C e 0x7E
    for m in MULTI.iter() {
        let e = enc(m.name);
        for b in *b"a\\~\n\t 0" {
            assert_eq!(e.first_char(&[b]), Some((b as char, 1)), "{}", m.name);
            assert_eq!(e.encode(b as char), Some(vec![b]), "{}", m.name);
        }
    }
}

#[test]
fn characters_the_encoding_cannot_write_are_none() {
    // riletti con Python: l'euro non e' in Shift_JIS, EUC-JP, Big5; la faccina non e' fuori da GB18030
    for nome in ["shift-jis", "euc-jp"] {
        assert_eq!(enc(nome).encode('€'), None, "{nome}");
    }
    // in Big5 invece si scrive (A3E1, come in cp950 e in WHATWG; solo il codec big5hkscs di Python non lo ha)
    assert_eq!(enc("big5").encode('€'), Some(vec![0xA3, 0xE1]));
    // EUC-JP si legge a 3 byte (JIS X 0212) ma encoding_rs non lo scrive: un carattere solo di JIS X 0212 non si digita
    assert_eq!(enc("euc-jp").encode('\u{4E02}'), None);
    for nome in ["shift-jis", "euc-jp", "gbk", "big5", "euc-kr"] {
        assert_eq!(enc(nome).encode('\u{1F600}'), None, "{nome}");
    }
    for m in MULTI.iter() {
        assert_eq!(enc(m.name).encode('\u{fffd}'), None, "{}", m.name);
    }
    let e = enc("shift-jis");
    assert_eq!(e.encode_str("a日\u{1F600}b"), Err('\u{1F600}'));
    assert_eq!(e.encode_str("a日b"), Ok(vec![b'a', 0x93, 0xFA, b'b']));
}

// ---------- byte non validi e sequenze troncate ----------

#[test]
fn invalid_and_truncated_sequences_are_one_replacement_byte() {
    let r = '\u{fffd}';
    // un primo byte da solo in fondo ai dati
    assert_eq!(enc("shift-jis").first_char(&[0x81]), Some((r, 1)));
    assert_eq!(enc("big5").first_char(&[0xA4]), Some((r, 1)));
    assert_eq!(enc("euc-kr").first_char(&[0xB0]), Some((r, 1)));
    // seguito da un secondo byte che non lo e' (ASCII): il primo vale U+FFFD e l'ASCII si rilegge da capo
    for (nome, lead) in [
        ("shift-jis", 0x81u8),
        ("big5", 0xA4),
        ("euc-kr", 0xB0),
        ("euc-jp", 0xA4),
        ("gbk", 0xB0),
    ] {
        let e = enc(nome);
        let dati = [lead, b' ', b'x'];
        assert_eq!(e.first_char(&dati), Some((r, 1)), "{nome}");
        assert_eq!(e.first_char(&dati[1..]), Some((' ', 1)), "{nome}");
        assert_eq!(e.decode_lossy(&dati), format!("{r} x"), "{nome}");
    }
    // sequenze lunghe troncate: EUC-JP a 3 byte, GB18030 a 4
    assert_eq!(enc("euc-jp").first_char(&[0x8F, 0xB0]), Some((r, 1)));
    assert_eq!(enc("gb18030").first_char(&[0x94, 0x39, 0xFC]), Some((r, 1)));
    assert_eq!(enc("gb18030").first_char(&[0x94, 0x39]), Some((r, 1)));
    // vuoto
    for m in MULTI.iter() {
        assert_eq!(enc(m.name).first_char(&[]), None);
        assert_eq!(enc(m.name).last_char_len(&[]), 0);
    }
}

// ---------- tornare indietro ----------

#[test]
fn the_trail_byte_that_looks_like_ascii_does_not_confuse_the_backward_step() {
    let sj = enc("shift-jis");
    // 'A' poi 表 (95 5C): l'ultimo carattere e' lungo 2, non 1 (0x5C da solo sarebbe la barra rovesciata)
    assert_eq!(sj.last_char_len(&[b'A', 0x95, 0x5C]), 2);
    assert_eq!(sj.last_char_len(&[0x95, 0x5C]), 2);
    // 表表: due caratteri, l'ultimo e' lungo 2
    assert_eq!(sj.last_char_len(&[0x95, 0x5C, 0x95, 0x5C]), 2);
    // ソ e poi una vera barra rovesciata: l'ultimo carattere e' lungo 1
    assert_eq!(sj.last_char_len(&[0x83, 0x5C, 0x5C]), 1);
    // dopo uno spazio (byte sicuro) o una cifra si riparte da li'
    assert_eq!(sj.last_char_len(&[0x20, 0x83, 0x5C]), 2);
    assert_eq!(sj.last_char_len(&[0x95, 0x5C, 0x20]), 1);
    // il carattere che contiene il byte 0x5C: comincia a 0, non a 1
    assert_eq!(sj.char_start(&[0x95, 0x5C, 0x41], 1), 0);
    assert_eq!(sj.char_start(&[0x95, 0x5C, 0x41], 0), 0);
    assert_eq!(sj.char_start(&[0x95, 0x5C, 0x41], 2), 2);
    // GB18030: 😀 e' 94 39 FC 36 e le cifre ASCII (0x30-0x39) possono essere secondo o quarto byte
    let gb = enc("gb18030");
    assert_eq!(gb.last_char_len(&[b'1', 0x94, 0x39, 0xFC, 0x36]), 4);
    assert_eq!(gb.last_char_len(&[0x94, 0x39, 0xFC, 0x36, b'9']), 1);
    assert_eq!(gb.char_start(&[0x94, 0x39, 0xFC, 0x36, b'9'], 3), 0);
    assert_eq!(gb.char_start(&[0x94, 0x39, 0xFC, 0x36, b'9'], 1), 0);
    // EUC-JP: 3 byte per JIS X 0212
    let ej = enc("euc-jp");
    assert_eq!(ej.last_char_len(&[b'a', 0x8F, 0xB0, 0xA1]), 3);
    assert_eq!(ej.char_start(&[0x8F, 0xB0, 0xA1, b'a'], 2), 0);
    // le altre codifiche: char_start non cambia la posizione
    assert_eq!(Encoding::Utf8.char_start(b"abc", 2), 2);
    assert_eq!(enc("windows-1252").char_start(b"abc", 1), 1);
}

#[test]
fn walking_back_over_random_text_gives_the_same_characters_as_forward() {
    // modello: i caratteri e le loro lunghezze vengono da `encode`, mai dalla ricerca all'indietro
    let pool = [
        'a', 'Z', '\\', '@', '[', '0', '9', ' ', '\n', '~', '日', '本', '語', 'あ', 'ア', '表',
        'ソ', '能', '中', '한', '\u{FF71}',
    ];
    let mut r = Lcg(2024);
    for m in MULTI.iter() {
        let e = enc(m.name);
        let mut provati = 0;
        for _ in 0..300 {
            let n = (r.next() % 60) as usize;
            let mut testo = String::new();
            let mut byte: Vec<u8> = Vec::new();
            let mut inizi: Vec<usize> = Vec::new();
            for _ in 0..n {
                let c = pool[(r.next() % pool.len() as u64) as usize];
                let Some(b) = e.encode(c) else { continue };
                testo.push(c);
                inizi.push(byte.len());
                byte.extend(b);
            }
            provati += 1;
            // avanti
            assert_eq!(
                e.decode_lossy(&byte),
                testo,
                "{} avanti {byte:02x?}",
                m.name
            );
            // indietro dalla fine, un carattere per volta
            let mut fine = byte.len();
            for &inizio in inizi.iter().rev() {
                let l = e.last_char_len(&byte[..fine]);
                assert_eq!(
                    fine - l,
                    inizio,
                    "{} indietro da {fine} in {byte:02x?}",
                    m.name
                );
                fine = inizio;
            }
            // il carattere che contiene ogni byte, con i byte seguenti disponibili
            for (k, &inizio) in inizi.iter().enumerate() {
                let fine_k = inizi.get(k + 1).copied().unwrap_or(byte.len());
                for i in inizio..fine_k {
                    assert_eq!(
                        e.char_start(&byte, i),
                        inizio,
                        "{} byte {i} di {byte:02x?}",
                        m.name
                    );
                }
            }
        }
        assert_eq!(provati, 300);
    }
}

#[test]
fn a_long_run_of_ideographs_is_walked_from_its_start() {
    // 400 ideogrammi senza un solo byte sicuro: se il contesto parte dall'inizio dei dati, si torna indietro bene
    for nome in ["shift-jis", "euc-jp", "gbk", "big5", "euc-kr"] {
        let e = enc(nome);
        let mut byte = Vec::new();
        for i in 0..400 {
            byte.extend(e.encode(if i % 2 == 0 { '日' } else { '本' }).unwrap());
        }
        for k in (1..=400usize).rev() {
            let l = e.last_char_len(&byte[..2 * k]);
            assert_eq!(l, 2, "{nome} carattere {k}");
        }
    }
}

#[test]
fn the_back_window_is_four_except_for_the_multibyte_encodings() {
    for e in [
        Encoding::Utf8,
        Encoding::Utf16Le,
        Encoding::Utf32Be,
        Encoding::Single(0),
    ] {
        assert_eq!(e.back_window(), 4, "{e:?}");
        assert!(!e.is_multibyte());
    }
    for m in MULTI.iter() {
        assert_eq!(enc(m.name).back_window(), BACK_WINDOW);
    }
    assert_eq!(BACK_WINDOW, 1024);
}

// ---------- il riconoscimento non cambia ----------

#[test]
fn detect_never_guesses_a_multibyte_encoding() {
    // un testo Shift_JIS non e' UTF-8 valido: si ripiega su windows-1252 come prima (con F9 si sceglie a mano)
    let sj = enc("shift-jis").encode_str("日本語のテキスト\n").unwrap();
    assert_eq!(Encoding::detect(&sj), enc("windows-1252"));
}
