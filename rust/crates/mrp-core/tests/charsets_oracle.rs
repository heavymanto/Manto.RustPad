//! Le tabelle delle code page contro una seconda fonte. `charsets.rs` e' generato dai codec di Python;
//! `data/charsets_oracle_net.txt` e' stato scritto da .NET (cioe' da Windows) con un altro programma
//! (una riga per code page: nome, spazio, 128 unita' UTF-16 in esadecimale a 4 cifre, `fffd` = byte non definito).
//! Se le due fonti sono d'accordo su tutti i 128 byte, un errore di trascrizione o di tabella e' improbabile.

use mrp_core::charsets::CHARSETS;
use mrp_core::encoding::Encoding;

const ORACOLO: &str = include_str!("data/charsets_oracle_net.txt");

fn oracolo() -> Vec<(&'static str, Vec<u32>)> {
    ORACOLO
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let (nome, hex) = l.split_once(' ').expect("riga: nome e tabella");
            assert_eq!(hex.len(), 128 * 4, "{nome}: la tabella ha 128 unita'");
            let unita = (0..128)
                .map(|i| u32::from_str_radix(&hex[i * 4..i * 4 + 4], 16).expect("esadecimale"))
                .collect();
            (nome, unita)
        })
        .collect()
}

/// Dove .NET e Python sono in disaccordo per ragioni note: (code page, byte, carattere di .NET, carattere nostro).
/// Sono tutte revisioni successive di uno standard o di una tabella Apple, che .NET tiene alla versione vecchia
/// (o alla variante Microsoft); le nostre vengono dai codec di Python, che seguono le tabelle aggiornate.
const DIFFERENZE_NOTE: &[(&str, u8, u32, u32)] = &[
    // ISO 8859-7:2003 (nostra) contro la versione del 1987 (.NET): apici greci, euro, dracma, ypogegrammeni
    ("iso-8859-7", 0xa1, 0x02bd, 0x2018),
    ("iso-8859-7", 0xa2, 0x02bc, 0x2019),
    ("iso-8859-7", 0xa4, 0xf7c2, 0x20ac),
    ("iso-8859-7", 0xa5, 0xf7c3, 0x20af),
    ("iso-8859-7", 0xaa, 0xf7c4, 0x037a),
    // ISO 8859-8:1999: macron al posto della linea sopra, e i due segni di direzione che .NET non conosce
    ("iso-8859-8", 0xaf, 0x203e, 0x00af),
    ("iso-8859-8", 0xfd, 0xf7bf, 0x200e),
    ("iso-8859-8", 0xfe, 0xf7c0, 0x200f),
    // windows-1255: Windows ha aggiunto in seguito U+05BA (holam haser per vav) in 0xCA; il codec di Python no
    ("windows-1255", 0xca, 0x05ba, 0xfffd),
    // KOI8-U come nella RFC 2319 (graficaz. a riquadri); la variante di Microsoft ha le lettere bielorusse
    ("koi8-u", 0xae, 0x045e, 0x255d),
    ("koi8-u", 0xbe, 0x040e, 0x256c),
    // Apple: U+2126 (segno ohm) e U+03A9 (omega) sono lo stesso carattere dopo la normalizzazione Unicode
    ("macintosh", 0xbd, 0x2126, 0x03a9),
    ("mac-iceland", 0xbd, 0x2126, 0x03a9),
    ("mac-turkish", 0xbd, 0x2126, 0x03a9),
    ("mac-croatian", 0xbd, 0x2126, 0x03a9),
    ("mac-romanian", 0xbd, 0x2126, 0x03a9),
    // Apple, edizioni dal 1998 in poi: l'euro prende il posto del segno di valuta
    ("mac-cyrillic", 0xff, 0x00a4, 0x20ac),
    ("mac-iceland", 0xdb, 0x00a4, 0x20ac),
    ("mac-croatian", 0xdb, 0x00a4, 0x20ac),
    ("mac-romanian", 0xdb, 0x00a4, 0x20ac),
    // Apple, mac-cyrillic: Ґ e ґ ucraine al posto di cent e derivata parziale
    ("mac-cyrillic", 0xa2, 0x00a2, 0x0490),
    ("mac-cyrillic", 0xb6, 0x2202, 0x0491),
    // Apple, mac-romanian: la virgola sotto (corretta in romeno) al posto della cediglia
    ("mac-romanian", 0xaf, 0x015e, 0x0218),
    ("mac-romanian", 0xbf, 0x015f, 0x0219),
    ("mac-romanian", 0xde, 0x0162, 0x021a),
    ("mac-romanian", 0xdf, 0x0163, 0x021b),
];

/// Un byte che la code page non definisce: .NET lo manda in un controllo C1 (U+0080-U+009F) o nell'area privata
/// (U+E000-U+F8FF) invece di segnalarlo; noi diciamo U+FFFD, come per un byte non valido in UTF-8.
fn indefinito_per_dotnet(u: u32) -> bool {
    (0x80..=0x9f).contains(&u) || (0xe000..=0xf8ff).contains(&u)
}

#[test]
fn the_tables_agree_with_dotnet_on_every_byte() {
    let mut diverse = Vec::new();
    let mut usate = vec![false; DIFFERENZE_NOTE.len()];
    for (nome, atteso) in oracolo() {
        let e = Encoding::by_name(nome).unwrap_or_else(|| panic!("codifica {nome} assente"));
        assert_eq!(e.name(), nome, "il nome canonico");
        for (j, &u) in atteso.iter().enumerate() {
            let b = 0x80 + j as u8;
            let ottenuto = e.decode_lossy(&[b]).chars().next().map_or(0, |c| c as u32);
            if ottenuto == u {
                continue;
            }
            if ottenuto == 0xfffd && indefinito_per_dotnet(u) {
                continue;
            }
            if let Some(k) = DIFFERENZE_NOTE
                .iter()
                .position(|d| *d == (nome, b, u, ottenuto))
            {
                usate[k] = true;
                continue;
            }
            diverse.push(format!(
                "{nome} 0x{b:02x}: .NET U+{u:04X}, qui U+{ottenuto:04X}"
            ));
        }
    }
    assert!(
        diverse.is_empty(),
        "{} byte diversi:
{}",
        diverse.len(),
        diverse.join(
            "
"
        )
    );
    // ogni eccezione dichiarata deve servire davvero (altrimenti la tabella e' cambiata e l'elenco mente)
    for (k, d) in DIFFERENZE_NOTE.iter().enumerate() {
        assert!(usate[k], "eccezione non piu' necessaria: {d:?}");
    }
}

#[test]
fn every_code_page_is_checked_except_the_known_ones_without_a_dotnet_table() {
    // .NET non ha queste: restano garantite dal solo codec di Python (e dalle regole del generatore)
    const SENZA_ORACOLO: [&str; 5] = [
        "iso-8859-10",
        "iso-8859-11",
        "iso-8859-14",
        "iso-8859-16",
        "ibm1125",
    ];
    let nomi: Vec<&str> = oracolo().iter().map(|(n, _)| *n).collect();
    for cs in CHARSETS.iter() {
        let c = nomi.contains(&cs.name);
        let s = SENZA_ORACOLO.contains(&cs.name);
        assert!(
            c != s,
            "{}: o ha l'oracolo o e' nell'elenco delle eccezioni, non entrambi",
            cs.name
        );
    }
    assert_eq!(nomi.len() + SENZA_ORACOLO.len(), CHARSETS.len());
}

#[test]
fn us_ascii_has_no_byte_above_0x7f_and_cannot_write_other_characters() {
    let e = Encoding::by_name("ascii").expect("alias ascii");
    assert_eq!(e.name(), "us-ascii");
    assert_eq!(Encoding::by_name("US-ASCII"), Some(e));
    assert_eq!(Encoding::by_name("iso646-us"), Some(e));
    // tutta la meta' alta e' non valida: un U+FFFD per byte
    let alto: Vec<u8> = (0x80..=0xffu8).collect();
    assert_eq!(e.decode_lossy(&alto), "\u{fffd}".repeat(128));
    assert_eq!(e.decode_lossy(b"abc\n"), "abc\n");
    // si scrive solo l'ASCII
    assert_eq!(e.encode('a'), Some(vec![b'a']));
    assert_eq!(e.encode('\n'), Some(vec![0x0a]));
    assert_eq!(e.encode('\u{e8}'), None);
    assert_eq!(e.encode('\u{20ac}'), None);
    assert_eq!(e.encode_str("caff\u{e8}"), Err('\u{e8}'));
}

#[test]
fn the_new_code_pages_follow_the_old_ones_so_old_indexes_do_not_move() {
    // l'indice in CHARSETS e' quello di `Encoding::Single(i)`: le 33 di prima restano dove erano
    let primi = [
        "windows-1250",
        "windows-1251",
        "windows-1252",
        "windows-1253",
        "windows-1254",
        "windows-1255",
        "windows-1256",
        "windows-1257",
        "windows-1258",
        "iso-8859-1",
    ];
    for (i, n) in primi.iter().enumerate() {
        assert_eq!(CHARSETS[i].name, *n);
    }
    assert_eq!(CHARSETS[32].name, "macintosh");
    assert_eq!(CHARSETS[33].name, "us-ascii");
    assert_eq!(CHARSETS.len(), 53);
}

#[test]
fn well_known_bytes_of_the_new_code_pages() {
    // fatti noti, uno per famiglia nuova (non vengono ne' da Python ne' da .NET: li ho scritti io dalle tabelle di riferimento)
    let casi: &[(&str, u8, char)] = &[
        ("windows-874", 0x80, '\u{20ac}'),
        ("windows-874", 0xa1, '\u{e01}'), // ko kai
        ("windows-874", 0xdf, '\u{e3f}'), // baht
        ("ibm855", 0x80, '\u{452}'),
        ("ibm855", 0xa0, '\u{430}'),
        ("ibm855", 0xa1, '\u{410}'),
        ("ibm857", 0xa7, '\u{11f}'), // g breve
        ("ibm860", 0x80, '\u{c7}'),
        ("ibm860", 0x86, '\u{c1}'),
        ("ibm862", 0x80, '\u{5d0}'), // alef
        ("ibm865", 0x9b, '\u{f8}'),
        ("ibm869", 0xa4, '\u{391}'), // alfa maiuscola
        ("ibm775", 0x80, '\u{106}'),
        ("mac-cyrillic", 0x80, '\u{410}'),
        ("mac-turkish", 0xda, '\u{11e}'),
        ("mac-greek", 0x80, '\u{c4}'),
        ("mac-greek", 0xe1, '\u{3b1}'), // alfa minuscola
        ("mac-greek", 0x9c, '\u{ad}'),  // trattino debole (accordo di .NET e libiconv)
        ("mac-greek", 0xaf, '\u{387}'), // ano teleia
    ];
    for &(n, b, c) in casi {
        let e = Encoding::by_name(n).unwrap();
        assert_eq!(e.decode_lossy(&[b]), c.to_string(), "{n} 0x{b:02x}");
        assert_eq!(e.encode(c), Some(vec![b]), "{n} {c:?}");
    }
    // i byte che queste code page non definiscono non si scrivono e valgono U+FFFD
    let e = Encoding::by_name("ibm857").unwrap();
    assert_eq!(e.decode_lossy(&[0xd5]), "\u{fffd}");
}
