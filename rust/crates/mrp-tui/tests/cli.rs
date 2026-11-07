//! Test del binario `mrp-tui`: argomenti, errori e la modalita' `--dump` (schermo come testo, senza terminale), con cui si
//! provano da fuori anche le sequenze di tasti. Scritti da ARCH prima dell'implementazione.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_cli_test_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn thirty(&self) -> PathBuf {
        let p = self.0.join("f.txt");
        let data: String = (1..=30).map(|i| format!("line {i:02}\n")).collect();
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
        // il test non deve leggere ne' scrivere il profilo vero dell'utente
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

fn lines(s: &str) -> Vec<&str> {
    s.lines().map(|l| l.trim_end()).collect()
}

// ---------- argomenti ----------

#[test]
fn help_lists_the_options() {
    let o = run(&["--help"]);
    assert_eq!(o.code, 0);
    assert!(o
        .stdout
        .contains("Manto.RustPad - the editor for files that do not fit in RAM"));
    assert!(o
        .stdout
        .contains("usage: mrp-tui [--lang CODE] [--theme NAME] FILE"));
    assert!(o
        .stdout
        .contains("--theme NAME   night | debian | console | minimal"));
    assert_eq!(run(&["-h"]).stdout, o.stdout);
}

#[test]
fn help_speaks_the_chosen_language() {
    let o = run(&["--lang-dir", &lang_dir(), "--lang", "it", "--help"]);
    assert_eq!(o.code, 0);
    assert!(
        o.stdout
            .contains("l'editor per i file che non stanno in RAM"),
        "{}",
        o.stdout
    );
}

#[test]
fn no_file_opens_a_new_untitled_document_in_edit_mode() {
    // senza file e senza profilo (MRP_NO_PROFILE) non c'e' una sessione da riaprire: un documento nuovo
    let o = run(&["--dump", "80x5"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("Untitled 1"), "{}", o.stdout);
    assert!(o.stdout.contains("EDIT"), "{}", o.stdout);
    // e nella lingua scelta
    let o = run(&["--lang-dir", &lang_dir(), "--lang", "it", "--dump", "80x5"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("Senza nome 1"), "{}", o.stdout);
}

#[test]
fn bad_usage_exits_with_2_and_says_why() {
    let dir = TestDir::new();
    let f = dir.thirty();
    let f = f.to_str().unwrap();
    let o = run(&["--theme", "rainbow", f]);
    assert_eq!((o.code, o.stderr.trim()), (2, "unknown theme: rainbow"));
    let o = run(&["--lang", "zz", f]);
    assert_eq!((o.code, o.stderr.trim()), (2, "unknown language: zz"));
    let o = run(&["--dump", "banana", f]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("banana"), "{}", o.stderr);
    let o = run(&["--dump", "30x7", "--keys", "Down,Wibble", f]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("Wibble"), "{}", o.stderr);
}

#[test]
fn a_missing_file_exits_with_1() {
    let dir = TestDir::new();
    let missing = dir.0.join("non_esiste.txt");
    let o = run(&["--dump", "30x7", missing.to_str().unwrap()]);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr
            .starts_with(&format!("cannot open {}: ", missing.display())),
        "{}",
        o.stderr
    );
}

// ---------- --dump ----------

#[test]
fn dump_prints_the_first_screen() {
    let dir = TestDir::new();
    let f = dir.thirty();
    let o = run(&["--dump", "30x7", f.to_str().unwrap()]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        lines(&o.stdout),
        vec![
            "  1│line 01",
            "  2│line 02",
            "  3│line 03",
            "  4│line 04",
            "  5│line 05",
            " Ln 1  Col 1  byte 0  0%",
            " Alt+V View  Alt+E Edit  Alt+T",
        ]
    );
}

#[test]
fn dump_applies_keys_in_order() {
    let dir = TestDir::new();
    let f = dir.thirty();
    let o = run(&[
        "--dump",
        "30x7",
        "--keys",
        "Down,Down,Down,Down,Down,Down",
        f.to_str().unwrap(),
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let l = lines(&o.stdout);
    assert_eq!(l[0], "  3│line 03");
    assert_eq!(l[4], "  7│line 07");
    assert_eq!(l[5], " Ln 7  Col 1  byte 48  20%");
    // altri nomi di tasto, in minuscolo e maiuscolo, e Ctrl+
    let o = run(&[
        "--dump",
        "30x7",
        "--keys",
        "ctrl+end, PgUp ,Home",
        f.to_str().unwrap(),
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(lines(&o.stdout)[5], " Ln 26  Col 1  byte 200  83%");
}

#[test]
fn dump_can_type_edit_and_save() {
    let dir = TestDir::new();
    let f = dir.thirty();
    let o = run(&[
        "--dump",
        "40x7",
        "--keys",
        "Alt+e,text:XY,Ctrl+S",
        f.to_str().unwrap(),
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let l = lines(&o.stdout);
    assert_eq!(l[0], "  1│XYline 01");
    assert!(l[5].starts_with(" saved: f.txt"), "{:?}", l[5]);
    let su_disco = std::fs::read_to_string(&f).unwrap();
    assert!(
        su_disco.starts_with("XYline 01\nline 02\n"),
        "{}",
        &su_disco[..40]
    );
    assert_eq!(su_disco.len(), 242);
}

#[test]
fn dump_drives_the_dialogs_and_settings() {
    let dir = TestDir::new();
    let f = dir.thirty();
    let settings = dir.0.join("prof").join("settings.txt");
    let o = run(&[
        "--dump",
        "90x23",
        "--settings",
        settings.to_str().unwrap(),
        "--keys",
        "F6,Up,Enter",
        f.to_str().unwrap(),
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        lines(&o.stdout)[21].starts_with(" theme: Debian (blue)"),
        "{:?}",
        lines(&o.stdout)[21]
    );
    assert_eq!(
        std::fs::read_to_string(&settings)
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<_>>(),
        vec!["theme=debian"]
    );
    // una seconda esecuzione riparte dal tema salvato e dalla lingua scelta con --lang
    let o = run(&[
        "--dump",
        "90x23",
        "--settings",
        settings.to_str().unwrap(),
        "--lang-dir",
        &lang_dir(),
        "--lang",
        "it",
        "--keys",
        "F6",
        f.to_str().unwrap(),
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("TEMA"), "{}", o.stdout);
    assert!(o.stdout.contains("Debian (blu)"), "{}", o.stdout);
}

#[test]
fn dump_goto_and_help() {
    let dir = TestDir::new();
    let f = dir.thirty();
    let o = run(&[
        "--dump",
        "80x23",
        "--keys",
        "Ctrl+G,text:12,Enter",
        f.to_str().unwrap(),
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        lines(&o.stdout)[21].starts_with(" Ln 12  Col 1  byte 88  36%"),
        "{:?}",
        lines(&o.stdout)[21]
    );
    let o = run(&["--dump", "80x23", "--keys", "F1", f.to_str().unwrap()]);
    assert!(
        o.stdout.contains("KEYS") && o.stdout.contains("Esc closes"),
        "{}",
        o.stdout
    );
}
