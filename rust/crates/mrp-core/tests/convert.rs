//! La conversione di codifica (4.1): `convert_document` scrive il contenuto di un documento in un altro file con un'altra
//! codifica, a blocchi, con un file temporaneo e una rinomina. Scritti da ARCH prima dell'implementazione: tutti gli attesi
//! sono byte calcolati a mano dalle tabelle delle codifiche (o da `std`), mai dall'output del codice.

use mrp_core::convert::{convert_document, ConvertError, ConvertOptions};
use mrp_core::encoding::Encoding;
use mrp_core::Document;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_conv_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, nome: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(nome);
        std::fs::write(&p, data).unwrap();
        p
    }
    fn path(&self, nome: &str) -> PathBuf {
        self.0.join(nome)
    }
    /// I nomi dei file nella cartella, in ordine.
    fn nomi(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn enc(nome: &str) -> Encoding {
    Encoding::by_name(nome).unwrap_or_else(|| panic!("codifica {nome}"))
}

/// Converte `sorgente` (byte in `da`) in `a`; restituisce i byte scritti o l'errore.
fn converti(
    sorgente: &[u8],
    da: &str,
    a: &str,
    bom: bool,
    blocco: usize,
) -> Result<Vec<u8>, ConvertError> {
    let d = TestDir::new();
    let p = d.file("src.txt", sorgente);
    let doc = Document::open(&p).unwrap();
    let t = d.path("dst.txt");
    let opts = ConvertOptions {
        block_bytes: blocco,
    };
    convert_document(
        &doc,
        enc(da),
        enc(a),
        bom,
        &t,
        &opts,
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    )?;
    Ok(std::fs::read(&t).unwrap())
}

const GRANDE: usize = 1 << 20;

#[test]
fn utf8_to_a_code_page() {
    // "caffè €\nnaïve\n": è = E8 e ï = EF in windows-1252, € = 80
    let r = converti(
        "caffè €\nnaïve\n".as_bytes(),
        "utf-8",
        "windows-1252",
        false,
        GRANDE,
    )
    .unwrap();
    assert_eq!(
        r,
        [0x63, 0x61, 0x66, 0x66, 0xE8, 0x20, 0x80, 0x0A, 0x6E, 0x61, 0xEF, 0x76, 0x65, 0x0A]
    );
}

#[test]
fn a_code_page_to_utf8() {
    // E8 = è (C3 A8), 80 = € (E2 82 AC) in windows-1252
    let r = converti(
        &[0x61, 0xE8, 0x80, 0x0A],
        "windows-1252",
        "utf-8",
        false,
        GRANDE,
    )
    .unwrap();
    assert_eq!(r, [0x61, 0xC3, 0xA8, 0xE2, 0x82, 0xAC, 0x0A]);
}

#[test]
fn utf8_to_utf16le_with_bom_writes_surrogate_pairs() {
    // A, €, U+1F600 (coppia D83D DE00), a-capo
    let r = converti("A€😀\n".as_bytes(), "utf-8", "utf-16le", true, GRANDE).unwrap();
    assert_eq!(
        r,
        [0xFF, 0xFE, 0x41, 0x00, 0xAC, 0x20, 0x3D, 0xD8, 0x00, 0xDE, 0x0A, 0x00]
    );
}

#[test]
fn utf8_to_utf16be_without_bom() {
    let r = converti("A€😀\n".as_bytes(), "utf-8", "utf-16be", false, GRANDE).unwrap();
    assert_eq!(
        r,
        [0x00, 0x41, 0x20, 0xAC, 0xD8, 0x3D, 0xDE, 0x00, 0x00, 0x0A]
    );
}

#[test]
fn utf8_to_utf32le_with_bom() {
    // U+1F600 = 0001F600
    let r = converti("A€😀\n".as_bytes(), "utf-8", "utf-32le", true, GRANDE).unwrap();
    assert_eq!(
        r,
        [
            0xFF, 0xFE, 0x00, 0x00, 0x41, 0x00, 0x00, 0x00, 0xAC, 0x20, 0x00, 0x00, 0x00, 0xF6,
            0x01, 0x00, 0x0A, 0x00, 0x00, 0x00
        ]
    );
}

#[test]
fn utf8_to_utf32be_with_bom() {
    let r = converti("A\n".as_bytes(), "utf-8", "utf-32be", true, GRANDE).unwrap();
    assert_eq!(
        r,
        [0x00, 0x00, 0xFE, 0xFF, 0x00, 0x00, 0x00, 0x41, 0x00, 0x00, 0x00, 0x0A]
    );
}

#[test]
fn a_source_bom_is_skipped_and_not_doubled() {
    // UTF-16LE con BOM -> UTF-8 senza BOM: il BOM sorgente non diventa un carattere U+FEFF
    let r = converti(
        &[0xFF, 0xFE, 0x41, 0x00, 0xAC, 0x20],
        "utf-16le",
        "utf-8",
        false,
        GRANDE,
    )
    .unwrap();
    assert_eq!(r, [0x41, 0xE2, 0x82, 0xAC]);
    // UTF-8 con BOM -> UTF-8 con BOM: uno solo
    let r = converti(&[0xEF, 0xBB, 0xBF, 0x61], "utf-8", "utf-8", true, GRANDE).unwrap();
    assert_eq!(r, [0xEF, 0xBB, 0xBF, 0x61]);
    // e senza BOM richiesto, non resta
    let r = converti(&[0xEF, 0xBB, 0xBF, 0x61], "utf-8", "utf-8", false, GRANDE).unwrap();
    assert_eq!(r, [0x61]);
}

#[test]
fn a_code_page_or_multibyte_target_never_gets_a_bom() {
    let r = converti("a\n".as_bytes(), "utf-8", "windows-1252", true, GRANDE).unwrap();
    assert_eq!(r, [0x61, 0x0A]);
    // shift-jis: 日 = 93 FA, 本 = 96 7B, 語 = 8C EA
    let r = converti("日本語".as_bytes(), "utf-8", "shift-jis", true, GRANDE).unwrap();
    assert_eq!(r, [0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA]);
}

#[test]
fn an_unrepresentable_character_stops_the_conversion_with_its_offset_and_writes_nothing() {
    let d = TestDir::new();
    // "ab" + U+1F600 (4 byte) + "cd": il carattere sta al byte 2
    let p = d.file("src.txt", "ab😀cd".as_bytes());
    let t = d.file("dst.txt", b"old");
    let doc = Document::open(&p).unwrap();
    let r = convert_document(
        &doc,
        enc("utf-8"),
        enc("windows-1252"),
        false,
        &t,
        &ConvertOptions::default(),
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    );
    match r {
        Err(ConvertError::Unrepresentable { offset, ch }) => {
            assert_eq!(offset, 2);
            assert_eq!(ch, '😀');
        }
        altro => panic!("atteso Unrepresentable, trovato {altro:?}"),
    }
    // il file che c'era resta com'era e non resta nessun temporaneo
    assert_eq!(std::fs::read(&t).unwrap(), b"old");
    assert_eq!(d.nomi(), ["dst.txt", "src.txt"]);
}

#[test]
fn the_offset_of_an_unrepresentable_character_counts_the_source_bom() {
    let d = TestDir::new();
    let p = d.file("src.txt", &[0xEF, 0xBB, 0xBF, b'a', 0xF0, 0x9F, 0x98, 0x80]);
    let doc = Document::open(&p).unwrap();
    let r = convert_document(
        &doc,
        enc("utf-8"),
        enc("ibm437"),
        false,
        &d.path("dst.txt"),
        &ConvertOptions::default(),
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    );
    assert!(
        matches!(
            r,
            Err(ConvertError::Unrepresentable {
                offset: 4,
                ch: '😀'
            })
        ),
        "{r:?}"
    );
    assert!(!d.path("dst.txt").exists());
}

#[test]
fn an_invalid_byte_is_a_replacement_character_that_utf_targets_keep_and_code_pages_refuse() {
    // 0xFF in UTF-8 e' un byte non valido: U+FFFD (un carattere per byte non valido)
    let r = converti(&[0x61, 0xFF, 0x62], "utf-8", "utf-16le", false, GRANDE).unwrap();
    assert_eq!(r, [0x61, 0x00, 0xFD, 0xFF, 0x62, 0x00]);
    let r = converti(&[0x61, 0xFF, 0x62], "utf-8", "utf-8", false, GRANDE).unwrap();
    assert_eq!(r, [0x61, 0xEF, 0xBF, 0xBD, 0x62]);
    let r = converti(&[0x61, 0xFF, 0x62], "utf-8", "windows-1252", false, GRANDE);
    assert!(
        matches!(
            r,
            Err(ConvertError::Unrepresentable {
                offset: 1,
                ch: '\u{fffd}'
            })
        ),
        "{r:?}"
    );
}

#[test]
fn a_dangling_byte_at_the_end_of_a_utf16_source_is_one_replacement_character() {
    // 41 00 = A, 42 solo: un byte che non completa un'unita'
    let r = converti(&[0x41, 0x00, 0x42], "utf-16le", "utf-8", false, GRANDE).unwrap();
    assert_eq!(r, [0x41, 0xEF, 0xBF, 0xBD]);
}

#[test]
fn an_empty_document_gives_an_empty_file_or_just_the_bom() {
    assert_eq!(
        converti(b"", "utf-8", "utf-16le", false, GRANDE).unwrap(),
        b""
    );
    assert_eq!(
        converti(b"", "utf-8", "utf-16le", true, GRANDE).unwrap(),
        [0xFF, 0xFE]
    );
}

#[test]
fn block_boundaries_never_split_a_character() {
    // testo con caratteri da 1, 2, 3 e 4 byte, ripetuto: l'oracolo e' `std` (encode_utf16)
    let base = "aé€😀日本語\nxyz ñ 😀😀 €\n";
    let testo: String = base.repeat(40);
    let mut atteso = vec![0xFF, 0xFE];
    for u in testo.encode_utf16() {
        atteso.extend_from_slice(&u.to_le_bytes());
    }
    for blocco in [8, 9, 10, 11, 13, 64, 1000, GRANDE] {
        let r = converti(testo.as_bytes(), "utf-8", "utf-16le", true, blocco).unwrap();
        assert_eq!(r, atteso, "blocco {blocco}");
    }
    // e il ritorno: da UTF-16LE (con quel BOM) a UTF-8 e' il testo, con ogni misura di blocco
    for blocco in [8, 9, 10, 11, 13, 64, 1000, GRANDE] {
        let r = converti(&atteso, "utf-16le", "utf-8", false, blocco).unwrap();
        assert_eq!(r, testo.as_bytes(), "ritorno, blocco {blocco}");
    }
    // da multibyte (shift-jis) con blocchi piccoli: 日本語 ripetuto
    let sj: Vec<u8> = [0x93u8, 0xFA, 0x96, 0x7B, 0x8C, 0xEA].repeat(30);
    let atteso_utf8 = "日本語".repeat(30);
    for blocco in [8, 9, 10, 11, 64] {
        let r = converti(&sj, "shift-jis", "utf-8", false, blocco).unwrap();
        assert_eq!(r, atteso_utf8.as_bytes(), "shift-jis, blocco {blocco}");
    }
}

#[test]
fn a_block_limit_below_the_minimum_is_raised() {
    let r = converti("é€😀\n".as_bytes(), "utf-8", "utf-16be", false, 1).unwrap();
    assert_eq!(
        r,
        [0x00, 0xE9, 0x20, 0xAC, 0xD8, 0x3D, 0xDE, 0x00, 0x00, 0x0A]
    );
}

#[test]
fn unsaved_edits_are_converted_too() {
    let d = TestDir::new();
    let p = d.file("src.txt", b"abc\n");
    let mut doc = Document::open(&p).unwrap();
    doc.insert(3, "é".as_bytes()).unwrap();
    let t = d.path("dst.txt");
    convert_document(
        &doc,
        enc("utf-8"),
        enc("windows-1252"),
        false,
        &t,
        &ConvertOptions::default(),
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    )
    .unwrap();
    assert_eq!(std::fs::read(&t).unwrap(), [0x61, 0x62, 0x63, 0xE9, 0x0A]);
    // il documento non cambia: e' ancora modificato e il file di partenza e' com'era
    assert!(doc.is_modified());
    assert_eq!(std::fs::read(&p).unwrap(), b"abc\n");
}

#[test]
fn it_reports_the_work_done_and_replaces_an_existing_target() {
    let d = TestDir::new();
    let p = d.file("src.txt", "A€😀\n".as_bytes());
    let t = d.file("dst.txt", b"vecchio contenuto molto piu' lungo del nuovo");
    let doc = Document::open(&p).unwrap();
    let progress = AtomicU64::new(0);
    let stats = convert_document(
        &doc,
        enc("utf-8"),
        enc("utf-16le"),
        true,
        &t,
        &ConvertOptions { block_bytes: 8 },
        &AtomicBool::new(false),
        &progress,
    )
    .unwrap();
    // 4 caratteri (A, €, U+1F600, a-capo); 12 byte scritti con il BOM
    assert_eq!(stats.chars, 4);
    assert_eq!(stats.bytes_out, 12);
    assert_eq!(std::fs::read(&t).unwrap().len(), 12);
    // il progresso conta i byte letti dal documento: 1 + 3 + 4 + 1
    assert_eq!(progress.load(Ordering::Relaxed), 9);
    assert_eq!(d.nomi(), ["dst.txt", "src.txt"]);
}

#[test]
fn a_cancel_stops_before_writing_and_leaves_nothing_behind() {
    let d = TestDir::new();
    let p = d.file("src.txt", "ciao\n".repeat(1000).as_bytes());
    let doc = Document::open(&p).unwrap();
    let t = d.path("dst.txt");
    let r = convert_document(
        &doc,
        enc("utf-8"),
        enc("utf-16le"),
        false,
        &t,
        &ConvertOptions { block_bytes: 64 },
        &AtomicBool::new(true),
        &AtomicU64::new(0),
    );
    assert!(matches!(r, Err(ConvertError::Cancelled)), "{r:?}");
    assert_eq!(d.nomi(), ["src.txt"]);
}

#[test]
fn the_target_can_be_the_open_file_and_replaces_it() {
    let d = TestDir::new();
    let p = d.file(
        "src.txt",
        "caffè
"
        .as_bytes(),
    );
    let doc = Document::open(&p).unwrap();
    convert_document(
        &doc,
        enc("utf-8"),
        enc("windows-1252"),
        false,
        &p,
        &ConvertOptions::default(),
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(&p).unwrap(),
        [0x63, 0x61, 0x66, 0x66, 0xE8, 0x0A]
    );
    // niente temporanei
    assert_eq!(d.nomi(), ["src.txt"]);
}

#[test]
fn a_target_in_a_missing_folder_is_an_io_error() {
    let d = TestDir::new();
    let p = d.file("src.txt", b"abc\n");
    let doc = Document::open(&p).unwrap();
    let t: &Path = &d.path("manca").join("dst.txt");
    let r = convert_document(
        &doc,
        enc("utf-8"),
        enc("windows-1252"),
        false,
        t,
        &ConvertOptions::default(),
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    );
    assert!(matches!(r, Err(ConvertError::Io(_))), "{r:?}");
}
