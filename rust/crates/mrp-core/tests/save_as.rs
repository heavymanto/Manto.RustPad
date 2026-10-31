//! Salva con nome (E42): `Document::save_as` scrive il contenuto attuale in un altro file e il documento diventa quel
//! file. Scritti da ARCH: gli attesi sono i byte calcolati a mano (un `Vec<u8>` su cui si fa splice), mai l'output del codice.

use mrp_core::{Document, SaveError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_saveas_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, nome: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(nome);
        std::fs::write(&p, data).unwrap();
        p
    }
    fn names(&self) -> Vec<String> {
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

fn flags() -> (AtomicBool, AtomicU64) {
    (AtomicBool::new(false), AtomicU64::new(0))
}

fn salva_con_nome(d: &mut Document, dove: &Path) -> Result<(), SaveError> {
    let (c, p) = flags();
    d.save_as(dove, &c, &p)
}

fn tutto(d: &Document) -> Vec<u8> {
    d.read_range(0, d.len())
}

#[test]
fn save_as_writes_the_edited_content_to_the_new_file_and_leaves_the_old_one() {
    let dir = TestDir::new();
    let a = dir.file("a.txt", b"hello world\n");
    let b = dir.0.join("b.txt");
    let mut d = Document::open(&a).unwrap();
    d.insert(5, b",").unwrap();
    d.delete(6, 1).unwrap(); // "hello, world" -> "hello,world" (lo spazio e' alla posizione 6)
    assert_eq!(tutto(&d), b"hello,world\n");
    salva_con_nome(&mut d, &b).unwrap();
    assert_eq!(std::fs::read(&b).unwrap(), b"hello,world\n");
    assert_eq!(
        std::fs::read(&a).unwrap(),
        b"hello world\n",
        "l'originale non si tocca"
    );
    assert_eq!(d.path(), b.canonicalize().unwrap().as_path());
    assert!(!d.is_modified());
    assert_eq!(tutto(&d), b"hello,world\n");
    assert!(!d.can_undo(), "la storia riparte dal file nuovo");
    assert_eq!(
        dir.names(),
        vec!["a.txt", "b.txt"],
        "nessun temporaneo rimasto"
    );
}

#[test]
fn save_as_without_edits_makes_a_copy() {
    let dir = TestDir::new();
    let a = dir.file("a.txt", b"uno\ndue\n");
    let b = dir.0.join("copia.txt");
    let mut d = Document::open(&a).unwrap();
    salva_con_nome(&mut d, &b).unwrap();
    assert_eq!(std::fs::read(&b).unwrap(), b"uno\ndue\n");
    assert_eq!(d.path(), b.canonicalize().unwrap().as_path());
    assert!(!d.is_modified());
}

#[test]
fn save_as_replaces_an_existing_target() {
    let dir = TestDir::new();
    let a = dir.file("a.txt", b"nuovo\n");
    let b = dir.file("b.txt", b"vecchio e piu' lungo\n");
    let mut d = Document::open(&a).unwrap();
    salva_con_nome(&mut d, &b).unwrap();
    assert_eq!(std::fs::read(&b).unwrap(), b"nuovo\n");
    assert_eq!(dir.names(), vec!["a.txt", "b.txt"]);
}

#[test]
fn after_save_as_the_next_edits_and_saves_go_to_the_new_file() {
    let dir = TestDir::new();
    let a = dir.file("a.txt", b"abc\n");
    let b = dir.0.join("b.txt");
    let mut d = Document::open(&a).unwrap();
    salva_con_nome(&mut d, &b).unwrap();
    d.insert(3, b"XYZ").unwrap();
    assert!(d.is_modified());
    let (c, p) = flags();
    d.save(&c, &p).unwrap();
    assert_eq!(std::fs::read(&b).unwrap(), b"abcXYZ\n");
    assert_eq!(
        std::fs::read(&a).unwrap(),
        b"abc\n",
        "il file di partenza resta com'era"
    );
}

#[test]
fn save_as_to_the_same_path_is_a_normal_save() {
    let dir = TestDir::new();
    let a = dir.file("a.txt", b"abc\n");
    let mut d = Document::open(&a).unwrap();
    d.insert(0, b">").unwrap();
    salva_con_nome(&mut d, &a).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), b">abc\n");
    assert!(!d.is_modified());
    // senza modifiche non c'e' niente da fare
    salva_con_nome(&mut d, &a).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), b">abc\n");
    assert_eq!(dir.names(), vec!["a.txt"]);
}

#[test]
fn a_missing_target_folder_fails_and_the_document_stays_as_it_was() {
    let dir = TestDir::new();
    let a = dir.file("a.txt", b"abc\n");
    let mut d = Document::open(&a).unwrap();
    d.insert(0, b">").unwrap();
    let ko = dir.0.join("non_esiste").join("b.txt");
    assert!(matches!(salva_con_nome(&mut d, &ko), Err(SaveError::Io(_))));
    assert!(d.is_modified());
    assert_eq!(d.path(), a.canonicalize().unwrap().as_path());
    assert_eq!(tutto(&d), b">abc\n");
    assert!(d.can_undo());
    assert_eq!(std::fs::read(&a).unwrap(), b"abc\n");
    assert_eq!(dir.names(), vec!["a.txt"]);
}

#[test]
fn a_cancelled_save_as_leaves_everything_untouched() {
    let dir = TestDir::new();
    let a = dir.file("a.txt", b"abc\n");
    let b = dir.0.join("b.txt");
    let mut d = Document::open(&a).unwrap();
    d.insert(0, b">").unwrap();
    let (c, p) = flags();
    c.store(true, Ordering::Relaxed);
    assert!(matches!(d.save_as(&b, &c, &p), Err(SaveError::Cancelled)));
    assert!(d.is_modified());
    assert_eq!(d.path(), a.canonicalize().unwrap().as_path());
    assert_eq!(
        dir.names(),
        vec!["a.txt"],
        "ne' il nuovo file ne' temporanei"
    );
}

#[test]
fn save_as_of_an_empty_document_with_typed_text() {
    // il caso del documento nuovo: un file vuoto di partenza, poi si scrive e si salva con nome
    let dir = TestDir::new();
    let vuoto = dir.file("senza_nome.txt", b"");
    let b = dir.0.join("nota.txt");
    let mut d = Document::open(&vuoto).unwrap();
    assert!(d.is_empty());
    d.insert(0, "ciao\n".as_bytes()).unwrap();
    d.insert(5, "mondo\n".as_bytes()).unwrap();
    salva_con_nome(&mut d, &b).unwrap();
    assert_eq!(std::fs::read(&b).unwrap(), b"ciao\nmondo\n");
    assert_eq!(d.len(), 11);
}

#[test]
fn save_as_keeps_a_large_file_intact_across_pieces() {
    // 300 KB con un'aggiunta nel mezzo e una cancellazione: l'oracolo e' uno splice su un Vec
    let dir = TestDir::new();
    let mut atteso: Vec<u8> = (0..300_000u32).map(|i| b'a' + (i % 26) as u8).collect();
    let a = dir.file("grande.txt", &atteso);
    let b = dir.0.join("grande2.txt");
    let mut d = Document::open(&a).unwrap();
    d.insert(150_000, b"<<<INSERITO>>>").unwrap();
    atteso.splice(150_000..150_000, b"<<<INSERITO>>>".iter().copied());
    d.delete(10, 1000).unwrap();
    atteso.drain(10..1010);
    salva_con_nome(&mut d, &b).unwrap();
    assert_eq!(std::fs::read(&b).unwrap(), atteso);
    assert_eq!(d.len() as usize, atteso.len());
}

#[test]
fn save_as_into_another_folder_leaves_no_temporary_in_either() {
    let dir = TestDir::new();
    std::fs::create_dir_all(dir.0.join("sotto")).unwrap();
    let a = dir.file("a.txt", b"dati\n");
    let b = dir.0.join("sotto").join("b.txt");
    let mut d = Document::open(&a).unwrap();
    d.insert(0, b"+").unwrap();
    salva_con_nome(&mut d, &b).unwrap();
    assert_eq!(std::fs::read(&b).unwrap(), b"+dati\n");
    assert_eq!(dir.names(), vec!["a.txt", "sotto"]);
    let sotto: Vec<String> = std::fs::read_dir(dir.0.join("sotto"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(sotto, vec!["b.txt"]);
}
