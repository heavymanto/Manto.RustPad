//! Le codifiche dalla riga di comando (E20): `--encoding NAME`, `--list-encodings`, l'aiuto. Scritti da ARCH.

use mrp_core::encoding::Encoding;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_clienc_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, data: &[u8]) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = self.0.join(format!("f{n}.txt"));
        std::fs::write(&p, data).unwrap();
        p
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Out {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run(args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_mrp-tui"))
        .env("MRP_NO_PROFILE", "1")
        .args(args)
        .output()
        .unwrap();
    Out {
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        code: o.status.code().unwrap_or(-1),
    }
}

fn lang_dir() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../mrp-core/lang")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn list_encodings_prints_every_name_one_per_line_and_needs_no_file() {
    let o = run(&["--list-encodings"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let righe: Vec<&str> = o.stdout.lines().map(|l| l.trim_end()).collect();
    let attese: Vec<&str> = Encoding::all().iter().map(|e| e.name()).collect();
    assert_eq!(righe, attese);
    for nome in ["utf-8", "windows-1252", "ibm437", "iso-8859-15", "koi8-r"] {
        assert!(righe.contains(&nome), "manca {nome}");
    }
}

#[test]
fn an_unknown_encoding_is_a_usage_error() {
    let dir = TestDir::new();
    let f = dir.file(b"abc\n");
    let o = run(&["--encoding", "bogus", f.to_str().unwrap()]);
    assert_eq!((o.code, o.stderr.trim()), (2, "unknown encoding: bogus"));
    let o = run(&["--encoding", "utf-16", f.to_str().unwrap()]);
    assert_eq!((o.code, o.stderr.trim()), (2, "unknown encoding: utf-16"));
}

#[test]
fn the_encoding_option_changes_how_the_dump_reads_the_bytes() {
    let dir = TestDir::new();
    let f = dir.file(b"caff\xe8 \x80\n");
    let f = f.to_str().unwrap();
    let con = |args: &[&str]| {
        let mut v: Vec<&str> = args.to_vec();
        v.extend(["--dump", "40x6", f]);
        run(&v)
    };
    let o = con(&["--encoding", "windows-1252"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("caff\u{e8} \u{20ac}"), "{}", o.stdout);
    let o = con(&["--encoding", "CP1252"]);
    assert!(o.stdout.contains("caff\u{e8} \u{20ac}"), "{}", o.stdout);
    let o = con(&["--encoding", "utf-8"]);
    assert!(o.stdout.contains("caff\u{fffd} \u{fffd}"), "{}", o.stdout);
    // senza opzione, o con `auto`, decide l'inizio del file: byte non UTF-8 -> windows-1252
    let o = con(&[]);
    assert!(o.stdout.contains("caff\u{e8} \u{20ac}"), "{}", o.stdout);
    let o = con(&["--encoding", "auto"]);
    assert!(o.stdout.contains("caff\u{e8} \u{20ac}"), "{}", o.stdout);
    // il nome della codifica compare nella riga di stato del dump
    assert!(o.stdout.contains("windows-1252"), "{}", o.stdout);
}

#[test]
fn a_utf8_file_is_not_misdetected() {
    let dir = TestDir::new();
    let f = dir.file("caff\u{e8} \u{20ac}\n".as_bytes());
    let o = run(&["--dump", "40x6", f.to_str().unwrap()]);
    assert!(o.stdout.contains("caff\u{e8} \u{20ac}"), "{}", o.stdout);
    assert!(!o.stdout.contains("windows-1252"), "{}", o.stdout);
}

#[test]
fn help_lists_the_option_in_every_language() {
    let o = run(&["--help"]);
    assert!(o.stdout.contains("--encoding NAME"), "{}", o.stdout);
    assert!(o.stdout.contains("--list-encodings"), "{}", o.stdout);
    let o = run(&["--lang-dir", &lang_dir(), "--lang", "it", "--help"]);
    assert!(o.stdout.contains("--encoding NOME"), "{}", o.stdout);
}

#[test]
fn the_error_speaks_the_chosen_language() {
    let dir = TestDir::new();
    let f = dir.file(b"abc\n");
    let o = run(&[
        "--lang-dir",
        &lang_dir(),
        "--lang",
        "it",
        "--encoding",
        "bogus",
        f.to_str().unwrap(),
    ]);
    assert_eq!(
        (o.code, o.stderr.trim()),
        (2, "codifica sconosciuta: bogus")
    );
}
