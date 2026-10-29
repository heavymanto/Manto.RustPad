//! Test delle lingue. Scritti da ARCH prima dell'implementazione (stessa tecnica di Manto.FreeDisk: inglese incorporato,
//! le altre lingue sono file JSON, un file per lingua). Gli attesi sono scritti a mano.

use mrp_core::lang::{registry_in, Catalog, Lang};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> TestDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("mrp_lang_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn file(&self, nome: &str, data: &str) {
        std::fs::write(self.0.join(nome), data).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn read(text: &str) -> Result<Lang, String> {
    Lang::from_json_str("xx", text)
}

// ---------- il lettore ----------

#[test]
fn a_flat_object_is_read() {
    let l = read(r#"{"a": "uno", "b": "due"}"#).unwrap();
    assert_eq!(l.get("a"), Some("uno"));
    assert_eq!(l.get("b"), Some("due"));
    assert_eq!(l.get("c"), None);
    assert_eq!(l.code, "xx");
    assert_eq!(l.name, "xx", "senza _name il nome e' il codice");
}

#[test]
fn bom_and_windows_line_ends_are_tolerated() {
    let l = read("\u{feff}{\r\n  \"a\": \"uno\",\r\n  \"b\": \"due\"\r\n}\r\n").unwrap();
    assert_eq!(l.keys(), vec!["a", "b"]);
}

#[test]
fn escapes_and_raw_non_ascii_read_the_same() {
    let raw = read("{\"k\": \"caffè 日\"}").unwrap();
    let esc = read("{\"k\": \"caff\\u00e8 \\u65e5\"}").unwrap();
    assert_eq!(raw.get("k"), esc.get("k"));
    assert_eq!(raw.get("k"), Some("caffè 日"));
    let d = read("{\"k\": \"a\\\"b\\\\c\\nd\\te\\/f\\bg\\fh\\ri\"}").unwrap();
    assert_eq!(d.get("k"), Some("a\"b\\c\nd\te/f\u{8}g\u{c}h\ri"));
}

#[test]
fn surrogate_pairs_compose_and_lone_ones_become_the_replacement_char() {
    let d = read("{\"k\": \"\\ud83d\\uddc2\"}").unwrap();
    assert_eq!(d.get("k"), Some("\u{1f5c2}"));
    let solo = read("{\"k\": \"\\ud83d!\"}").unwrap();
    assert_eq!(solo.get("k"), Some("\u{fffd}!"));
    let basso = read("{\"k\": \"\\udc00\"}").unwrap();
    assert_eq!(basso.get("k"), Some("\u{fffd}"));
}

#[test]
fn malformed_text_is_refused_and_says_where() {
    for (testo, perche) in [
        ("{", "non finisce"),
        ("{\"a\" \"b\"}", "manca ':'"),
        ("{\"a\": 1}", "non e' una stringa"),
        ("{\"a\": \"b\",}", "virgola finale"),
        ("{\"a\": \"b\"} xy", "roba dopo"),
        ("[\"a\"]", "non e' un oggetto"),
        ("", "vuoto"),
        ("{\"a\": \"b\\q\"}", "escape sconosciuto"),
        ("{\"a\": \"b", "stringa non chiusa"),
        ("{\"a\": \"\\u12\"}", "\\u troppo corto"),
        ("{\"a\": \"x\ny\"}", "a-capo dentro la stringa"),
    ] {
        assert!(read(testo).is_err(), "{perche}: {testo:?}");
    }
    let e = read("{\"a\": \"b\"\n\"c\": \"d\"}").unwrap_err();
    assert!(e.contains("line 2"), "dice la riga, in inglese: {e}");
}

#[test]
fn the_empty_object_is_a_valid_language() {
    let l = read("{}").unwrap();
    assert!(l.keys().is_empty());
}

#[test]
fn meta_keys_are_not_vocabulary() {
    let l = read(r#"{"_name": "Deutsch", "_ai": "KI-generierte Uebersetzung", "k": "v"}"#).unwrap();
    assert_eq!(l.name, "Deutsch");
    assert!(l.ai);
    assert_eq!(l.get("_ai"), Some("KI-generierte Uebersetzung"));
    assert_eq!(l.keys(), vec!["k"], "la parita' non guarda le chiavi meta");
    assert!(!read(r#"{"k": "v"}"#).unwrap().ai);
}

// ---------- il catalogo ----------

#[test]
fn the_catalog_falls_back_to_english_then_to_the_key() {
    let en = Lang::english();
    let una_chiave = en.keys()[0].to_string();
    let it = read(&format!(
        "{{\"_name\": \"Xx\", \"{una_chiave}\": \"TRADOTTO\"}}"
    ))
    .unwrap();
    let cat = Catalog::with(it);
    assert_eq!(cat.t(&una_chiave), "TRADOTTO");
    assert_eq!(cat.code(), "xx");
    let altra = en.keys()[1].to_string();
    assert_eq!(
        cat.t(&altra),
        en.get(&altra).unwrap(),
        "chiave mancante: l'inglese incorporato"
    );
    assert_eq!(cat.t("chiave.che.non.esiste"), "chiave.che.non.esiste");
    let cat_en = Catalog::english();
    assert_eq!(cat_en.code(), "en");
    assert_eq!(cat_en.t(&una_chiave), en.get(&una_chiave).unwrap());
}

#[test]
fn placeholders_fill_in_order_without_panicking() {
    let cat = Catalog::english();
    assert_eq!(cat.tf("x{}y{}z", &["1", "2"]), "x1y2z");
    assert_eq!(cat.tf("x{}y", &["1"]), "x1y");
    assert_eq!(cat.tf("x{}y", &[]), "xy");
    assert_eq!(cat.tf("xy", &["1"]), "xy");
    assert_eq!(cat.tf("x{}y", &["1", "2"]), "x1y");
    assert_eq!(cat.tf("{}{}", &["a", "b"]), "ab");
}

// ---------- la scoperta ----------

#[test]
fn discovery_skips_unreadable_and_non_json_files() {
    let dir = TestDir::new();
    dir.file("xx.json", "{\"_name\": \"Xx\", \"k\": \"v\"}");
    dir.file("rotto.json", "{non json");
    dir.file("note.txt", "non e' una lingua");
    let (langs, unreadable) = registry_in(Some(&dir.0));
    assert_eq!(langs.len(), 2, "en incorporato + xx");
    assert_eq!(langs[0].code, "en");
    assert_eq!(langs[1].code, "xx");
    assert_eq!(langs[1].name, "Xx");
    assert_eq!(unreadable, vec!["rotto.json".to_string()]);
}

#[test]
fn discovered_languages_are_ordered_by_code_after_english() {
    let dir = TestDir::new();
    for c in ["zz", "bb", "mm"] {
        dir.file(&format!("{c}.json"), "{}");
    }
    let (langs, _) = registry_in(Some(&dir.0));
    let codici: Vec<&str> = langs.iter().map(|l| l.code.as_str()).collect();
    assert_eq!(codici, vec!["en", "bb", "mm", "zz"]);
}

#[test]
fn the_profile_overrides_the_embedded_english() {
    let dir = TestDir::new();
    dir.file("en.json", "{\"_name\": \"English (custom)\"}");
    let (langs, unreadable) = registry_in(Some(&dir.0));
    assert!(unreadable.is_empty());
    assert_eq!(langs.len(), 1);
    assert_eq!(langs[0].code, "en");
    assert_eq!(langs[0].name, "English (custom)");
    // le chiavi che l'inglese custom non ha cadono nell'inglese incorporato
    let en = Lang::english();
    let k = en.keys()[0].to_string();
    let cat = Catalog::with(langs[0].clone());
    assert_eq!(cat.t(&k), en.get(&k).unwrap());
}

#[test]
fn a_missing_or_absent_folder_gives_only_english() {
    let (langs, unreadable) = registry_in(None);
    assert_eq!(langs.len(), 1);
    assert!(unreadable.is_empty());
    let (langs, unreadable) = registry_in(Some(Path::new("/questa/cartella/non/esiste/mrp")));
    assert_eq!(langs.len(), 1);
    assert!(unreadable.is_empty());
}

// ---------- il gate delle lingue (la completezza non si revisiona a mano) ----------

fn placeholders(s: &str) -> usize {
    s.matches("{}").count()
}

#[test]
fn every_language_file_has_exactly_the_keys_and_placeholders_of_english() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("lang");
    let en = Lang::english();
    let mut viste = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let l = Lang::from_file(&path)
            .unwrap_or_else(|e| panic!("{} non si legge: {}", path.display(), e));
        assert_eq!(
            l.keys(),
            en.keys(),
            "{}: le chiavi devono essere quelle di en.json",
            path.display()
        );
        assert!(
            !l.get("_name").unwrap_or("").trim().is_empty(),
            "{}: _name manca",
            path.display()
        );
        for k in en.keys() {
            let v = l.get(k).unwrap();
            assert!(
                !v.trim().is_empty(),
                "{}: valore vuoto per {k}",
                path.display()
            );
            assert_eq!(
                placeholders(v),
                placeholders(en.get(k).unwrap()),
                "{}: {k} deve avere gli stessi segnaposto {{}} dell'inglese",
                path.display()
            );
        }
        // le traduzioni AI si dichiarano; l'inglese e l'italiano no
        let code = l.code.clone();
        if code == "en" || code == "it" {
            assert!(!l.ai, "{code} non e' una traduzione AI");
        } else {
            assert!(l.ai, "{code}: le traduzioni non verificate portano _ai");
        }
        viste.push(code);
    }
    viste.sort();
    assert!(
        viste.contains(&"en".to_string()) && viste.contains(&"it".to_string()),
        "viste: {viste:?}"
    );
    // sale a 7 (en, it, es, pt-BR, de, ja, zh-CN) quando arrivano le traduzioni AI di partenza
    assert!(viste.len() >= 2, "almeno en e it (trovate: {viste:?})");
}

#[test]
fn the_embedded_english_is_the_file_on_disk() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("lang");
    let da_file = Lang::from_file(&dir.join("en.json")).unwrap();
    let incorporato = Lang::english();
    assert_eq!(da_file.keys(), incorporato.keys());
    for k in incorporato.keys() {
        assert_eq!(da_file.get(k), incorporato.get(k));
    }
    assert!(
        incorporato.keys().len() >= 40,
        "l'inglese incorporato non e' vuoto"
    );
}
