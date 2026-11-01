//! Test del JUN per la conversione di codifica: le piccole cose non coperte dai test di ARCH in
//! `tests/convert.rs` (opzioni predefinite, messaggi degli errori, avanzamento con il BOM sorgente,
//! statistiche di un documento vuoto, carattere che attraversa il confine del blocco). Tutti i valori attesi
//! sono calcolati a mano dalle tabelle delle codifiche, mai dall'output del codice.

use mrp_core::convert::{convert_document, ConvertError, ConvertOptions};
use mrp_core::encoding::Encoding;
use mrp_core::Document;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(std::path::PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_convjun_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, nome: &str, data: &[u8]) -> std::path::PathBuf {
        let p = self.0.join(nome);
        std::fs::write(&p, data).unwrap();
        p
    }
    fn path(&self, nome: &str) -> std::path::PathBuf {
        self.0.join(nome)
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

#[test]
fn default_options_use_a_one_mebibyte_block() {
    assert_eq!(ConvertOptions::default().block_bytes, 1 << 20);
}

#[test]
fn error_messages_are_short_and_in_italian() {
    assert_eq!(ConvertError::Cancelled.to_string(), "conversione annullata");
    // {ch} scrive il carattere, {offset} la posizione in byte nel documento
    let e = ConvertError::Unrepresentable {
        offset: 2, ch: 'é'
    };
    assert_eq!(
        e.to_string(),
        "il carattere é all'offset 2 non si puo' scrivere in questa codifica"
    );
    let e = ConvertError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "prova",
    ));
    assert_eq!(e.to_string(), "errore di I/O: prova");
}

#[test]
fn progress_counts_the_source_bom_too() {
    // EF BB BF = BOM UTF-8, poi "ab": 5 byte di documento; a fine lavoro l'avanzamento vale doc.len()
    let d = TestDir::new();
    let p = d.file("src.txt", &[0xEF, 0xBB, 0xBF, b'a', b'b']);
    let doc = Document::open(&p).unwrap();
    let progress = AtomicU64::new(0);
    let stats = convert_document(
        &doc,
        enc("utf-8"),
        enc("utf-16le"),
        false,
        &d.path("dst.txt"),
        &ConvertOptions { block_bytes: 4 },
        &AtomicBool::new(false),
        &progress,
    )
    .unwrap();
    // "ab" in UTF-16LE little endian: 61 00 62 00 (il BOM sorgente non si riscrive, nessuno richiesto)
    assert_eq!(
        std::fs::read(d.path("dst.txt")).unwrap(),
        [0x61, 0x00, 0x62, 0x00]
    );
    assert_eq!(stats.chars, 2);
    assert_eq!(stats.bytes_out, 4);
    assert_eq!(progress.load(Ordering::Relaxed), 5);
}

#[test]
fn stats_of_an_empty_document() {
    // documento vuoto con BOM richiesto: solo il BOM, zero caratteri
    let d = TestDir::new();
    let p = d.file("src.txt", b"");
    let doc = Document::open(&p).unwrap();
    let stats = convert_document(
        &doc,
        enc("utf-8"),
        enc("utf-16le"),
        true,
        &d.path("dst.txt"),
        &ConvertOptions::default(),
        &AtomicBool::new(false),
        &AtomicU64::new(0),
    )
    .unwrap();
    assert_eq!(std::fs::read(d.path("dst.txt")).unwrap(), [0xFF, 0xFE]);
    assert_eq!(stats.chars, 0);
    assert_eq!(stats.bytes_out, 2);
}

#[test]
fn a_character_crossing_the_block_limit_is_decoded_whole() {
    // "ab😀cd": 😀 occupa i byte 2..6 del documento; con blocco 4 comincia prima della fine del blocco e
    // sporge oltre: si decodifica intero con la guardia e il blocco dopo parte da 'c'
    let d = TestDir::new();
    let p = d.file("src.txt", "ab😀cd".as_bytes());
    let doc = Document::open(&p).unwrap();
    let progress = AtomicU64::new(0);
    let stats = convert_document(
        &doc,
        enc("utf-8"),
        enc("utf-16le"),
        false,
        &d.path("dst.txt"),
        &ConvertOptions { block_bytes: 4 },
        &AtomicBool::new(false),
        &progress,
    )
    .unwrap();
    // a, b, la coppia di surrogati D83D DE00 di U+1F600, c, d, tutti little endian
    assert_eq!(
        std::fs::read(d.path("dst.txt")).unwrap(),
        [0x61, 0x00, 0x62, 0x00, 0x3D, 0xD8, 0x00, 0xDE, 0x63, 0x00, 0x64, 0x00]
    );
    assert_eq!(stats.chars, 5);
    assert_eq!(stats.bytes_out, 12);
    // il documento era lungo 8 byte (61 62 F0 9F 98 80 63 64)
    assert_eq!(progress.load(Ordering::Relaxed), 8);
}
