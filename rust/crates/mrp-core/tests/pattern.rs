//! Ricerche con regole e sostituzioni (E36, nucleo): glob, regex, testo senza maiuscole, per riga, a blocchi, e la
//! sostituzione come un solo passo annullabile. Scritti da ARCH: i risultati delle regex vengono da Python (`re.sub`, vedi
//! `tools/gen_replace_oracle.py`), quelli del testo da `str::replace`/modelli a mano, quelli del glob da esempi calcolati
//! a mano.

use mrp_core::docsearch::SearchError;
use mrp_core::document::Document;
use mrp_core::pattern::{glob_to_regex, Edit, Mode, Pattern, Replacement};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

mod replace_oracle_data;

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn file(data: &[u8]) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_pattern_test_{}_{}.txt", std::process::id(), n));
    std::fs::write(&p, data).unwrap();
    p
}

fn doc(data: &[u8]) -> (Document, PathBuf) {
    let p = file(data);
    (Document::open(&p).unwrap(), p)
}

fn no() -> AtomicBool {
    AtomicBool::new(false)
}

fn prog() -> AtomicU64 {
    AtomicU64::new(0)
}

/// Le sostituzioni di `p` -> `r` su `dati` (con blocchi di `chunk` byte), applicate: il testo che ne esce.
fn sostituisci(dati: &[u8], p: &Pattern, r: &Replacement, chunk: usize) -> Vec<u8> {
    let (mut d, f) = doc(dati);
    let snap = d.snapshot().unwrap().with_chunk(chunk);
    let edits = snap
        .replace_edits(p, r, usize::MAX - 1, &no(), &prog())
        .unwrap()
        .unwrap();
    d.apply_edits(&edits).unwrap();
    let fuori = d.read_range(0, d.len());
    drop(d);
    let _ = std::fs::remove_file(f);
    fuori
}

fn pat(t: &str, m: Mode, case: bool) -> Pattern {
    Pattern::new(t, m, case).unwrap()
}

// ---- il glob ----

#[test]
fn a_glob_becomes_exactly_this_regex() {
    for (glob, regex) in [
        ("a*b", "a[^\\n]*?b"),
        ("x?y", "x[^\\n]y"),
        ("[a-c]", "[a-c]"),
        ("[!ab]", "[^ab]"),
        ("[^ab]", "[^ab]"),
        ("a.b", "a\\.b"),
        ("a+b(c)", "a\\+b\\(c\\)"),
        ("a\\*b", "a\\*b"),
        ("a\\?", "a\\?"),
        ("[", "\\["),
        ("[]", "\\[\\]"),
        ("[abc", "\\[abc"),
        ("[!]", "\\[!\\]"),
        ("[a-]", "[a\\-]"),
        ("[-a]", "[\\-a]"),
        ("[a]b", "[a]b"),
        ("[\\]", "[\\\\]"),
        ("*", "[^\\n]*?"),
        ("", ""),
    ] {
        assert_eq!(glob_to_regex(glob), regex, "glob {glob:?}");
    }
}

#[test]
fn glob_matches_by_line_the_least_it_can() {
    // "a*b" su "a1b2b": il meno possibile, quindi "a1b"
    let out = sostituisci(
        b"a1b2b\nxab\nab\na\nb\n",
        &pat("a*b", Mode::Glob, true),
        &Replacement::new("#", Mode::Glob),
        1 << 20,
    );
    // "a1b2b" -> "#2b"; "xab" -> "x#"; "ab" -> "#"; "a" e "b" restano (l'a-capo non e' coperto da *)
    assert_eq!(out, b"#2b\nx#\n#\na\nb\n");
}

#[test]
fn glob_question_mark_and_classes() {
    let out = sostituisci(
        b"cat cot cut cit\ncxt c\nt\n",
        &pat("c[ao]t", Mode::Glob, true),
        &Replacement::new("X", Mode::Glob),
        1 << 20,
    );
    assert_eq!(out, b"X X cut cit\ncxt c\nt\n");
    let out = sostituisci(
        b"cat cot cut\n",
        &pat("c[!ao]t", Mode::Glob, true),
        &Replacement::new("X", Mode::Glob),
        1 << 20,
    );
    assert_eq!(out, b"cat cot X\n");
    let out = sostituisci(
        "caffe caf\u{e8} caf\n".as_bytes(),
        &pat("caf?", Mode::Glob, true),
        &Replacement::new("K", Mode::Glob),
        1 << 20,
    );
    // `?` e' un carattere intero, anche a due byte: "caff" e "cafè" combaciano, "caf" a fine riga no
    assert_eq!(out, "Ke K caf\n".as_bytes());
}

#[test]
fn a_glob_with_special_regex_characters_is_literal() {
    let out = sostituisci(
        b"a.b a+b (x) a*b\n",
        &pat("a.b", Mode::Glob, true),
        &Replacement::new("Z", Mode::Glob),
        1 << 20,
    );
    assert_eq!(out, b"Z a+b (x) a*b\n");
    let out = sostituisci(
        b"(x) [y]\n",
        &pat("(x)", Mode::Glob, true),
        &Replacement::new("Q", Mode::Glob),
        1 << 20,
    );
    assert_eq!(out, b"Q [y]\n");
}

// ---- l'oracolo Python per le regex ----

#[test]
fn regex_replacements_match_python_re_sub_line_by_line() {
    for (i, (p, r, testo, atteso)) in replace_oracle_data::CASI.iter().enumerate() {
        let pattern = pat(p, Mode::Regex, true);
        let sost = Replacement::new(r, Mode::Regex);
        // a blocchi molto piccoli (con righe che li superano) e a blocchi grandi: lo stesso risultato
        for chunk in [5, 17, 1 << 20] {
            let out = sostituisci(testo.as_bytes(), &pattern, &sost, chunk);
            assert_eq!(
                String::from_utf8_lossy(&out),
                *atteso,
                "caso {i}: {p:?} -> {r:?} (blocco {chunk})"
            );
        }
    }
}

// ---- testo semplice ----

#[test]
fn plain_text_replacement_equals_str_replace_over_many_random_texts() {
    let mut x: u64 = 7;
    let mut rnd = |n: u64| {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (x >> 33) % n
    };
    for _ in 0..200 {
        let len = rnd(120) as usize;
        let testo: String = (0..len)
            .map(|_| match rnd(5) {
                0 => 'a',
                1 => 'b',
                2 => '\n',
                3 => 'è',
                _ => ' ',
            })
            .collect();
        let n = 1 + rnd(3) as usize;
        let ago: String = (0..n)
            .map(|_| if rnd(2) == 0 { 'a' } else { 'b' })
            .collect();
        let chunk = 3 + rnd(40) as usize;
        let out = sostituisci(
            testo.as_bytes(),
            &pat(&ago, Mode::Text, true),
            &Replacement::new("<>", Mode::Text),
            chunk,
        );
        // l'ago non contiene a-capo: `str::replace` sul testo intero e' come per riga
        assert_eq!(
            String::from_utf8(out).unwrap(),
            testo.replace(&ago, "<>"),
            "testo {testo:?} ago {ago:?} blocco {chunk}"
        );
    }
}

#[test]
fn text_without_case_matches_both_cases_and_unicode_folds() {
    let out = sostituisci(
        "Ciao CIAO ciao cIaO\nÈ è e\n".as_bytes(),
        &pat("ciao", Mode::Text, false),
        &Replacement::new("x", Mode::Text),
        1 << 20,
    );
    assert_eq!(out, "x x x x\nÈ è e\n".as_bytes());
    let out = sostituisci(
        "Ciao CIAO\nÈ è e E\n".as_bytes(),
        &pat("è", Mode::Text, false),
        &Replacement::new("_", Mode::Text),
        1 << 20,
    );
    assert_eq!(out, "Ciao CIAO\n_ _ e E\n".as_bytes());
    // con le maiuscole che contano
    let out = sostituisci(
        b"Ciao ciao\n",
        &pat("ciao", Mode::Text, true),
        &Replacement::new("x", Mode::Text),
        1 << 20,
    );
    assert_eq!(out, b"Ciao x\n");
}

#[test]
fn plain_text_treats_regex_characters_literally() {
    let out = sostituisci(
        b"a.c abc a*c\n",
        &pat("a.c", Mode::Text, true),
        &Replacement::new("$1", Mode::Text),
        1 << 20,
    );
    // la sostituzione e' letterale in modo Text: `$1` resta `$1`
    assert_eq!(out, b"$1 abc a*c\n");
}

// ---- la sostituzione ----

#[test]
fn the_replacement_understands_groups_named_groups_and_escapes_in_regex_mode() {
    let p = pat(r"(?P<n>\d+)-(\w+)", Mode::Regex, true);
    let out = sostituisci(
        b"12-ab 7-z\n",
        &p,
        &Replacement::new("${2}=${n}", Mode::Regex),
        1 << 20,
    );
    assert_eq!(out, b"ab=12 z=7\n");
    // \n \t \\ e $$
    let out = sostituisci(
        b"12-ab\n",
        &p,
        &Replacement::new("[\\n|\\t|\\\\|$$]", Mode::Regex),
        1 << 20,
    );
    assert_eq!(out, b"[\n|\t|\\|$]\n");
    // un gruppo che non c'e' vale vuoto
    let out = sostituisci(
        b"12-ab\n",
        &p,
        &Replacement::new("<$9>", Mode::Regex),
        1 << 20,
    );
    assert_eq!(out, b"<>\n");
}

#[test]
fn matches_never_cross_a_newline_and_empty_matches_are_dropped() {
    let (d, f) = doc(b"a\nb\n");
    let snap = d.snapshot().unwrap();
    // `a.b` non combacia attraverso l'a-capo, e `a\nb` non puo' combaciare per riga
    for t in ["a.b", "a\\nb", "a[^x]b"] {
        let p = pat(t, Mode::Regex, true);
        assert_eq!(
            snap.find_pattern_next(&p, 0, &no(), &prog()).unwrap(),
            None,
            "{t}"
        );
        assert_eq!(
            snap.count_pattern(&p, 100, &no(), &prog()).unwrap(),
            0,
            "{t}"
        );
    }
    // `x*` combacia col vuoto dappertutto: nessuna occorrenza
    let p = pat("x*", Mode::Regex, true);
    assert_eq!(snap.count_pattern(&p, 100, &no(), &prog()).unwrap(), 0);
    assert_eq!(snap.find_pattern_next(&p, 0, &no(), &prog()).unwrap(), None);
    drop(d);
    let _ = std::fs::remove_file(f);
}

// ---- trovare ----

/// Tutte le occorrenze del testo semplice `ago` per riga (modello a mano): (inizio, lunghezza).
fn modello(dati: &[u8], ago: &[u8]) -> Vec<(u64, u64)> {
    let mut v = Vec::new();
    let mut inizio_riga = 0usize;
    for riga in dati.split(|&b| b == b'\n') {
        let mut i = 0usize;
        while i + ago.len() <= riga.len() {
            if &riga[i..i + ago.len()] == ago {
                v.push(((inizio_riga + i) as u64, ago.len() as u64));
                i += ago.len();
            } else {
                i += 1;
            }
        }
        inizio_riga += riga.len() + 1;
    }
    v
}

#[test]
fn next_and_prev_agree_with_a_model_over_random_texts_and_tiny_blocks() {
    let mut x: u64 = 11;
    let mut rnd = |n: u64| {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (x >> 33) % n
    };
    for _ in 0..150 {
        let len = rnd(200) as usize;
        let dati: Vec<u8> = (0..len)
            .map(|_| match rnd(4) {
                0 => b'a',
                1 => b'b',
                2 => b'\n',
                _ => b'x',
            })
            .collect();
        let ago: Vec<u8> = (0..1 + rnd(2)).map(|_| b'a' + rnd(2) as u8).collect();
        let (d, f) = doc(&dati);
        let chunk = 2 + rnd(30) as usize;
        let snap = d.snapshot().unwrap().with_chunk(chunk);
        let p = pat(std::str::from_utf8(&ago).unwrap(), Mode::Text, true);
        let tutte = modello(&dati, &ago);
        for from in 0..=dati.len() as u64 {
            let atteso = tutte.iter().find(|(s, _)| *s >= from).copied();
            let ottenuto = snap.find_pattern_next(&p, from, &no(), &prog()).unwrap();
            assert_eq!(
                ottenuto, atteso,
                "next da {from} in {dati:?} ago {ago:?} blocco {chunk}"
            );
            let atteso = tutte.iter().rev().find(|(s, _)| *s < from).copied();
            let ottenuto = snap.find_pattern_prev(&p, from, &no(), &prog()).unwrap();
            assert_eq!(
                ottenuto, atteso,
                "prev prima di {from} in {dati:?} ago {ago:?} blocco {chunk}"
            );
        }
        assert_eq!(
            snap.count_pattern(&p, 1000, &no(), &prog()).unwrap(),
            tutte.len() as u64
        );
        drop(d);
        let _ = std::fs::remove_file(f);
    }
}

#[test]
fn a_line_longer_than_the_blocks_is_still_searched_whole() {
    let mut dati = vec![b'x'; 1000];
    dati.extend_from_slice(b"NEEDLE");
    dati.extend(vec![b'y'; 1000]);
    dati.extend_from_slice(b"\nNEEDLE\n");
    let (d, f) = doc(&dati);
    let snap = d.snapshot().unwrap().with_chunk(16);
    let p = pat("NEEDLE", Mode::Text, true);
    assert_eq!(
        snap.find_pattern_next(&p, 0, &no(), &prog()).unwrap(),
        Some((1000, 6))
    );
    assert_eq!(
        snap.find_pattern_next(&p, 1001, &no(), &prog()).unwrap(),
        Some((2007, 6))
    );
    assert_eq!(
        snap.find_pattern_prev(&p, 2007, &no(), &prog()).unwrap(),
        Some((1000, 6))
    );
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn find_and_count_report_progress_and_honour_cancel() {
    let dati: Vec<u8> = b"riga di prova\n".repeat(500);
    let (d, f) = doc(&dati);
    let snap = d.snapshot().unwrap().with_chunk(64);
    let p = pat("zzz", Mode::Text, true);
    let pr = prog();
    assert_eq!(snap.find_pattern_next(&p, 0, &no(), &pr).unwrap(), None);
    assert_eq!(pr.load(Ordering::Relaxed), dati.len() as u64);
    let annulla = AtomicBool::new(true);
    assert!(matches!(
        snap.find_pattern_next(&p, 0, &annulla, &prog()),
        Err(SearchError::Cancelled)
    ));
    assert!(matches!(
        snap.find_pattern_prev(&p, 100, &annulla, &prog()),
        Err(SearchError::Cancelled)
    ));
    assert!(matches!(
        snap.count_pattern(&p, 100, &annulla, &prog()),
        Err(SearchError::Cancelled)
    ));
    assert!(matches!(
        snap.replace_edits(&p, &Replacement::new("", Mode::Text), 10, &annulla, &prog()),
        Err(SearchError::Cancelled)
    ));
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn counting_and_replacing_stop_at_the_maximum() {
    let (d, f) = doc(&b"a\n".repeat(100));
    let snap = d.snapshot().unwrap();
    let p = pat("a", Mode::Text, true);
    assert_eq!(snap.count_pattern(&p, 100, &no(), &prog()).unwrap(), 100);
    assert_eq!(snap.count_pattern(&p, 40, &no(), &prog()).unwrap(), 41);
    let r = Replacement::new("b", Mode::Text);
    assert_eq!(
        snap.replace_edits(&p, &r, 100, &no(), &prog())
            .unwrap()
            .unwrap()
            .len(),
        100
    );
    assert!(snap
        .replace_edits(&p, &r, 99, &no(), &prog())
        .unwrap()
        .is_err());
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn a_replacement_over_the_modified_document_sees_the_edits() {
    // la fotografia e' del documento modificato (come la ricerca): le sostituzioni valgono per il testo di adesso
    let (mut d, f) = doc(b"uno due\ntre\n");
    d.insert(0, b"due ").unwrap();
    d.delete(8, 3).unwrap();
    assert_eq!(d.read_range(0, d.len()), b"due uno \ntre\n");
    let snap = d.snapshot().unwrap();
    let edits = snap
        .replace_edits(
            &pat("uno", Mode::Text, true),
            &Replacement::new("1", Mode::Text),
            10,
            &no(),
            &prog(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        edits,
        vec![Edit {
            start: 4,
            len: 3,
            new: b"1".to_vec()
        }]
    );
    drop(d);
    let _ = std::fs::remove_file(f);
}

// ---- applicare ----

#[test]
fn applying_edits_is_one_undo_step_and_redo_brings_it_back() {
    let (mut d, f) = doc(b"aa bb aa\naa\n");
    d.insert(0, b">").unwrap();
    d.seal();
    let prima = d.read_range(0, d.len());
    let snap = d.snapshot().unwrap();
    let edits = snap
        .replace_edits(
            &pat("aa", Mode::Text, true),
            &Replacement::new("Z", Mode::Text),
            10,
            &no(),
            &prog(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(edits.len(), 3);
    assert_eq!(d.apply_edits(&edits).unwrap(), 3);
    let dopo = d.read_range(0, d.len());
    assert_eq!(dopo, b">Z bb Z\nZ\n");
    // un solo annulla toglie tutte e tre
    d.undo().unwrap();
    assert_eq!(d.read_range(0, d.len()), prima);
    // un secondo annulla toglie la modifica di prima (la '>')
    d.undo().unwrap();
    assert_eq!(d.read_range(0, d.len()), b"aa bb aa\naa\n");
    d.redo().unwrap();
    d.redo().unwrap();
    assert_eq!(d.read_range(0, d.len()), dopo);
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn edits_that_are_not_valid_change_nothing() {
    let (mut d, f) = doc(b"abcdef\n");
    let e = |start, len, nuovo: &str| Edit {
        start,
        len,
        new: nuovo.as_bytes().to_vec(),
    };
    for lista in [
        // oltre la fine
        vec![e(5, 10, "x")],
        // fuori ordine
        vec![e(4, 1, "x"), e(0, 1, "y")],
        // sovrapposte
        vec![e(0, 3, "x"), e(2, 2, "y")],
        // overflow
        vec![e(u64::MAX, 2, "x")],
    ] {
        assert!(d.apply_edits(&lista).is_err());
        assert_eq!(d.read_range(0, d.len()), b"abcdef\n");
        assert!(!d.is_modified());
    }
    assert_eq!(d.apply_edits(&[]).unwrap(), 0);
    assert!(!d.is_modified());
    // confinanti e' ammesso (la seconda comincia dove finisce la prima); una a lunghezza zero si inserisce
    assert_eq!(
        d.apply_edits(&[e(0, 2, "XY"), e(2, 1, ""), e(6, 0, "!")])
            .unwrap(),
        3
    );
    assert_eq!(d.read_range(0, d.len()), b"XYdef!\n");
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn many_replacements_on_a_big_text_are_applied_and_undone_at_once() {
    let dati = "voce numero 1234\n".repeat(20_000).into_bytes();
    let (mut d, f) = doc(&dati);
    let snap = d.snapshot().unwrap();
    let edits = snap
        .replace_edits(
            &pat(r"\d+", Mode::Regex, true),
            &Replacement::new("<$0>", Mode::Regex),
            1_000_000,
            &no(),
            &prog(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(edits.len(), 20_000);
    d.apply_edits(&edits).unwrap();
    let atteso = "voce numero <1234>\n".repeat(20_000).into_bytes();
    assert_eq!(d.read_range(0, d.len()), atteso);
    d.undo().unwrap();
    assert_eq!(d.read_range(0, d.len()), dati);
    drop(d);
    let _ = std::fs::remove_file(f);
}

// ---- compilare ----

#[test]
fn bad_or_empty_patterns_are_refused() {
    assert!(Pattern::new("", Mode::Text, true).is_err());
    assert!(Pattern::new("", Mode::Glob, true).is_err());
    assert!(Pattern::new("", Mode::Regex, true).is_err());
    assert!(Pattern::new("(", Mode::Regex, true).is_err());
    assert!(Pattern::new("[a", Mode::Regex, true).is_err());
    assert!(Pattern::new("a{9999999}", Mode::Regex, true).is_err());
    // una regex normale con un riferimento che il motore non ha (all'indietro) si rifiuta
    assert!(Pattern::new("(a)\\1", Mode::Regex, true).is_err());
    assert!(Pattern::new("(", Mode::Text, true).is_ok());
    assert_eq!(
        Pattern::new("a", Mode::Glob, true).unwrap().mode(),
        Mode::Glob
    );
}

// ---- testo semplice in altre codifiche ----

#[test]
fn literal_occurrences_in_utf16_are_aligned_and_do_not_overlap() {
    // "aaa" in UTF-16 LE: 61 00 61 00 61 00; l'ago "a\0a\0" combacia a 0 e (sfasato, scartato) a 1 e (allineato) a 2
    let dati = [0x61u8, 0, 0x61, 0, 0x61, 0];
    let (mut d, f) = doc(&dati);
    d.set_newline(mrp_core::newline::Newline::Le16);
    let snap = d.snapshot().unwrap();
    let v = snap
        .find_all_literal(&[0x61, 0, 0x61, 0], 100, &no(), &prog())
        .unwrap()
        .unwrap();
    // non sovrapposte: l'occorrenza a 0 occupa 0..4, la successiva (a 2) cade dentro e non conta
    assert_eq!(v, vec![0]);
    let v = snap
        .find_all_literal(&[0x61, 0], 100, &no(), &prog())
        .unwrap()
        .unwrap();
    assert_eq!(v, vec![0, 2, 4]);
    // un ago sfasato (00 61 comincia a 1, dispari): non conta; quello allineato dopo un falso inizio si trova lo stesso
    let v = snap
        .find_all_literal(&[0, 0x61], 100, &no(), &prog())
        .unwrap()
        .unwrap();
    assert_eq!(v, Vec::<u64>::new());
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn literal_occurrences_in_shift_jis_skip_the_second_byte_of_a_character() {
    // 表 = 95 5C: il secondo byte e' 5C (barra rovesciata); cercare "\" non deve trovarlo li'
    let dati = [0x95u8, 0x5C, b'a', 0x5C, b'b'];
    let (d, f) = doc(&dati);
    let sj = mrp_core::encoding::Encoding::by_name("shift-jis").unwrap();
    let snap = d.snapshot().unwrap().with_boundary(sj);
    let v = snap
        .find_all_literal(b"\\", 100, &no(), &prog())
        .unwrap()
        .unwrap();
    assert_eq!(v, vec![3]);
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn literal_search_over_many_blocks_finds_each_occurrence_once_and_stops_at_the_maximum() {
    let dati = b"ab".repeat(500);
    let (d, f) = doc(&dati);
    let snap = d.snapshot().unwrap().with_chunk(7);
    let v = snap
        .find_all_literal(b"ab", 10_000, &no(), &prog())
        .unwrap()
        .unwrap();
    assert_eq!(v, (0..500u64).map(|i| i * 2).collect::<Vec<_>>());
    // un ago a cavallo dei blocchi ("ba" attraversa ogni confine da 7 byte) si trova una volta sola
    let v = snap
        .find_all_literal(b"ba", 10_000, &no(), &prog())
        .unwrap()
        .unwrap();
    assert_eq!(v, (0..499u64).map(|i| i * 2 + 1).collect::<Vec<_>>());
    assert!(snap
        .find_all_literal(b"ab", 100, &no(), &prog())
        .unwrap()
        .is_err());
    drop(d);
    let _ = std::fs::remove_file(f);
}

// ---- i casi trovati dalle mutazioni ----

#[test]
fn line_anchors_hold_even_when_the_search_starts_in_the_middle_of_a_line() {
    let (d, f) = doc(b"ab\nb\n");
    let snap = d.snapshot().unwrap().with_chunk(2);
    let p = pat("^b", Mode::Regex, true);
    // da 1 il "b" di "ab" non e' all'inizio di una riga: il prossimo e' quello della riga dopo (a 3)
    assert_eq!(
        snap.find_pattern_next(&p, 1, &no(), &prog()).unwrap(),
        Some((3, 1))
    );
    // e all'indietro da 4: la riga "b" a 3
    assert_eq!(
        snap.find_pattern_prev(&p, 4, &no(), &prog()).unwrap(),
        Some((3, 1))
    );
    // una riga che comincia esattamente a `before` non si conta (inizio < before)
    assert_eq!(snap.find_pattern_prev(&p, 3, &no(), &prog()).unwrap(), None);
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn an_expanding_replacement_also_drops_empty_matches() {
    // `x*` con la sostituzione che espande i gruppi: solo le "x" vere si sostituiscono, mai il vuoto
    let out = sostituisci(
        b"axxb x\n",
        &pat("x*", Mode::Regex, true),
        &Replacement::new("<$0>", Mode::Regex),
        1 << 20,
    );
    assert_eq!(out, b"a<xx>b <x>\n");
}

#[test]
fn the_maximum_is_exact_for_replacements_and_counts() {
    let (d, f) = doc(&b"a\n".repeat(10));
    let snap = d.snapshot().unwrap();
    let p = pat("a", Mode::Text, true);
    let r = Replacement::new("b", Mode::Text);
    assert!(snap
        .replace_edits(&p, &r, 10, &no(), &prog())
        .unwrap()
        .is_ok());
    // con tetto 9: sono 10, quindi si ferma: e dice quante ne ha contate (10)
    assert_eq!(
        snap.replace_edits(&p, &r, 9, &no(), &prog())
            .unwrap()
            .unwrap_err(),
        10
    );
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn backslash_escapes_in_the_replacement_are_exactly_these() {
    let p = pat("x", Mode::Regex, true);
    for (r, atteso) in [
        ("a\\nb", "a\nb"),
        ("a\\tb", "a\tb"),
        ("a\\rb", "a\rb"),
        ("a\\\\b", "a\\b"),
        ("a\\qb", "a\\qb"),
        ("a\\", "a\\"),
    ] {
        let out = sostituisci(b"x", &p, &Replacement::new(r, Mode::Regex), 1 << 20);
        assert_eq!(out, atteso.as_bytes(), "{r:?}");
    }
}

#[test]
fn a_caret_class_negates_like_a_bang() {
    let out = sostituisci(
        b"cat cot cut\n",
        &pat("c[^ao]t", Mode::Glob, true),
        &Replacement::new("X", Mode::Glob),
        1 << 20,
    );
    assert_eq!(out, b"cat cot X\n");
}

#[test]
fn a_line_longer_than_the_limit_is_cut_not_read_without_end() {
    // il tetto e' 16 MiB: una riga piu' lunga (qui 17 MiB di 'a' e un 'b' in fondo) si legge a pezzi, ma il lavoro finisce
    let mut dati = vec![b'a'; 17 * 1024 * 1024];
    dati.push(b'b');
    let (d, f) = doc(&dati);
    let snap = d.snapshot().unwrap();
    let p = pat("b", Mode::Text, true);
    let inizio = std::time::Instant::now();
    let r = snap.find_pattern_next(&p, 0, &no(), &prog()).unwrap();
    assert!(inizio.elapsed() < std::time::Duration::from_secs(60));
    // il 'b' sta oltre il tetto dall'inizio della riga: puo' non essere visto (limite documentato), ma se lo e', e' al posto giusto
    if let Some((s, l)) = r {
        assert_eq!((s, l), (dati.len() as u64 - 1, 1));
    }
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn a_replacement_is_its_own_undo_step_even_without_a_manual_seal() {
    let (mut d, f) = doc(b"aa bb\n");
    // una modifica non chiusa prima, e una dopo: la sostituzione sta in mezzo, da sola
    d.insert(0, b">").unwrap();
    let snap = d.snapshot().unwrap();
    let edits = snap
        .replace_edits(
            &pat("aa", Mode::Text, true),
            &Replacement::new("Z", Mode::Text),
            10,
            &no(),
            &prog(),
        )
        .unwrap()
        .unwrap();
    d.apply_edits(&edits).unwrap();
    d.insert(0, b"Q").unwrap();
    assert_eq!(d.read_range(0, d.len()), b"Q>Z bb\n");
    d.undo().unwrap();
    assert_eq!(
        d.read_range(0, d.len()),
        b">Z bb\n",
        "la Q si annulla da sola"
    );
    d.undo().unwrap();
    assert_eq!(
        d.read_range(0, d.len()),
        b">aa bb\n",
        "poi la sostituzione, ma non la '>'"
    );
    d.undo().unwrap();
    assert_eq!(d.read_range(0, d.len()), b"aa bb\n");
    drop(d);
    let _ = std::fs::remove_file(f);
}

#[test]
fn literal_occurrences_never_overlap_across_block_boundaries() {
    let (d, f) = doc(&[b'a'; 50]);
    let snap = d.snapshot().unwrap().with_chunk(7);
    let v = snap
        .find_all_literal(b"aa", 1000, &no(), &prog())
        .unwrap()
        .unwrap();
    assert_eq!(v, (0..25u64).map(|i| i * 2).collect::<Vec<_>>());
    drop(d);
    let _ = std::fs::remove_file(f);
}
