//! `Encoding::bom_len` (E29): quanti byte ha il BOM all'inizio dei dati, per ogni codifica. Attesi scritti a mano.

use mrp_core::encoding::Encoding;

fn enc(nome: &str) -> Encoding {
    Encoding::by_name(nome).unwrap()
}

#[test]
fn each_encoding_knows_its_own_bom() {
    let casi: &[(&str, &[u8], usize)] = &[
        ("utf-8", &[0xEF, 0xBB, 0xBF, b'a'], 3),
        ("utf-8", &[0xEF, 0xBB, 0xBF], 3),
        ("utf-8", &[0xEF, 0xBB], 0),
        ("utf-8", b"abc", 0),
        ("utf-8", &[0xFF, 0xFE], 0),
        ("utf-16le", &[0xFF, 0xFE, b'a', 0], 2),
        ("utf-16le", &[0xFE, 0xFF], 0),
        ("utf-16be", &[0xFE, 0xFF, 0, b'a'], 2),
        ("utf-16be", &[0xFF, 0xFE], 0),
        ("utf-32le", &[0xFF, 0xFE, 0, 0, b'a', 0, 0, 0], 4),
        ("utf-32le", &[0xFF, 0xFE, 0, 1], 0),
        ("utf-32be", &[0, 0, 0xFE, 0xFF, 0, 0, 0, b'a'], 4),
        ("utf-32be", &[0, 0, 0xFE], 0),
        // le code page e le multibyte non hanno BOM: quei byte sono caratteri
        ("windows-1252", &[0xEF, 0xBB, 0xBF], 0),
        ("shift-jis", &[0xFF, 0xFE], 0),
        ("gb18030", &[0xEF, 0xBB, 0xBF], 0),
    ];
    for &(nome, dati, atteso) in casi {
        assert_eq!(enc(nome).bom_len(dati), atteso, "{nome} {dati:02x?}");
    }
    assert_eq!(enc("utf-8").bom_len(&[]), 0);
}
