//! Il modo portatile: una cartella `lang` accanto all'eseguibile la rende "la cartella portatile".

use mrp_core::lang::portable_dir_of;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_portable_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_lang_folder_next_to_the_exe_makes_its_folder_portable() {
    let d = TestDir::new();
    std::fs::create_dir_all(d.0.join("lang")).unwrap();
    assert_eq!(portable_dir_of(&d.0.join("mrp.exe")), Some(d.0.clone()));
}

#[test]
fn without_a_lang_folder_there_is_no_portable_mode() {
    let d = TestDir::new();
    assert_eq!(portable_dir_of(&d.0.join("mrp.exe")), None);
    // un FILE chiamato lang non basta
    std::fs::write(d.0.join("lang"), b"x").unwrap();
    assert_eq!(portable_dir_of(&d.0.join("mrp.exe")), None);
}

#[test]
fn a_path_without_a_parent_is_not_portable() {
    assert_eq!(portable_dir_of(std::path::Path::new("")), None);
}
