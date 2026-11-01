//! La colorazione dei log (fase 2, 3.2): `loghl::highlight` trova in una riga di testo i livelli (ERROR, WARN, INFO, DEBUG e
//! parenti), le date e gli orari e gli indirizzi IPv4; `line_level` dice il livello della riga (il primo che compare). Sono
//! solo posizioni in byte: i colori li decide chi disegna. Scritti da ARCH: gli attesi sono calcolati a mano contando i byte.

use mrp_core::loghl::{highlight, line_level, Kind, Span};

fn s(start: usize, end: usize, kind: Kind) -> Span {
    Span { start, end, kind }
}

#[test]
fn a_typical_log_line_has_a_time_and_a_level() {
    // "2026-10-31 09:00:53" = 19 byte; poi " [" e INFO a 21..25
    let l = "2026-10-31 09:00:53 [INFO] api-2: retrying upload (attempt 669)";
    assert_eq!(
        highlight(l),
        vec![s(0, 19, Kind::Time), s(21, 25, Kind::Info)]
    );
}

#[test]
fn the_levels_and_their_kinds() {
    for (parola, tipo) in [
        ("ERROR", Kind::Error),
        ("ERR", Kind::Error),
        ("FATAL", Kind::Error),
        ("CRITICAL", Kind::Error),
        ("SEVERE", Kind::Error),
        ("PANIC", Kind::Error),
        ("WARN", Kind::Warn),
        ("WARNING", Kind::Warn),
        ("INFO", Kind::Info),
        ("NOTICE", Kind::Info),
        ("DEBUG", Kind::Debug),
        ("TRACE", Kind::Debug),
    ] {
        let l = format!("[{parola}] x");
        assert_eq!(
            highlight(&l),
            vec![s(1, 1 + parola.len(), tipo)],
            "{parola}"
        );
    }
}

#[test]
fn only_whole_uppercase_words_are_levels() {
    for l in [
        "INFORMATION here",
        "NOERROR",
        "xERROR",
        "ERROR_CODE=5",
        "error: lowercase",
        "Error: capitalised",
        "WARNINGS",
        "9ERROR",
    ] {
        assert_eq!(highlight(l), vec![], "{l}");
    }
    // delimitati da segni: parentesi, due punti, uguale, punto
    assert_eq!(highlight("level=ERROR."), vec![s(6, 11, Kind::Error)]);
    assert_eq!(highlight("(WARN):"), vec![s(1, 5, Kind::Warn)]);
    assert_eq!(highlight("WARNING: x"), vec![s(0, 7, Kind::Warn)]);
    assert_eq!(highlight("WARN x"), vec![s(0, 4, Kind::Warn)]);
}

#[test]
fn the_time_forms() {
    // (testo, fine dello span che comincia a 0)
    for (l, fine) in [
        ("2026-10-31T09:00:53Z", 20),
        ("2026-10-31 09:00:53.123", 23),
        ("2026-10-31 09:00:53,5", 21),
        ("2026-10-31T09:00:53+02:00", 25),
        ("2026-10-31T09:00:53-0200", 24),
        ("2026-10-31 09:00:53 UTC", 19),
        ("2026-10-31T09:00:53+", 19),
        ("09:00:53 start", 8),
        ("09:00:53.5 start", 10),
        ("23:59:60", 8),
    ] {
        assert_eq!(highlight(l)[0], s(0, fine, Kind::Time), "{l}");
    }
    // una data sola
    assert_eq!(highlight("on 2026-10-31 we"), vec![s(3, 13, Kind::Time)]);
    // data e ora separate da due spazi: due span
    assert_eq!(
        highlight("2026-10-31  09:00:53"),
        vec![s(0, 10, Kind::Time), s(12, 20, Kind::Time)]
    );
}

#[test]
fn what_is_not_a_time() {
    for l in [
        "12:30",
        "1:2:3",
        "x09:00:53",
        "2026-13-45",
        "2026-10-32",
        "2026-13-01",
        "2026-00-10",
        "2026-10-00",
        "24:00:00",
        "25:61:61",
        "09:60:00",
        "1234-56-78",
        "12026-10-31",
    ] {
        assert_eq!(highlight(l), vec![], "{l}");
    }
}

#[test]
fn the_ipv4_addresses() {
    assert_eq!(
        highlight("from 192.168.1.20 port 22"),
        vec![s(5, 17, Kind::Ip)]
    );
    // la porta non fa parte dell'indirizzo
    assert_eq!(highlight("10.0.0.1:8080"), vec![s(0, 8, Kind::Ip)]);
    assert_eq!(highlight("0.0.0.0"), vec![s(0, 7, Kind::Ip)]);
    assert_eq!(highlight("255.255.255.255!"), vec![s(0, 15, Kind::Ip)]);
    for l in [
        "256.1.1.1",
        "1.2.3",
        "1.2.3.4.5",
        "v1.2.3.4",
        "999.999.999.999",
        "1.2.3.4a",
        "1.2.3.",
        "1234.1.1.1",
    ] {
        assert_eq!(highlight(l), vec![], "{l}");
    }
}

#[test]
fn several_kinds_in_one_line_are_sorted_and_never_overlap() {
    // Time 0..19, ERROR a 20..25, l'indirizzo a 26..34
    let l = "2026-10-31 09:00:53 ERROR 10.0.0.1 failed";
    assert_eq!(
        highlight(l),
        vec![
            s(0, 19, Kind::Time),
            s(20, 25, Kind::Error),
            s(26, 34, Kind::Ip)
        ]
    );
    // un indirizzo e un orario vicini
    assert_eq!(
        highlight("192.168.1.1 09:00:53"),
        vec![s(0, 11, Kind::Ip), s(12, 20, Kind::Time)]
    );
}

#[test]
fn non_ascii_text_keeps_the_offsets_in_bytes_on_character_boundaries() {
    // "é " = 3 byte, ERROR a 3..8, " è " = 4 byte (8..12), l'indirizzo a 12..19
    let l = "é ERROR è 1.2.3.4";
    let v = highlight(l);
    assert_eq!(v, vec![s(3, 8, Kind::Error), s(12, 19, Kind::Ip)]);
    for sp in &v {
        assert!(l.is_char_boundary(sp.start) && l.is_char_boundary(sp.end));
    }
    // caratteri larghi attorno
    let l = "日本 WARN 日本";
    assert_eq!(highlight(l), vec![s(7, 11, Kind::Warn)]);
}

#[test]
fn the_level_of_a_line_is_the_first_level_that_appears() {
    assert_eq!(line_level("WARN then ERROR"), Some(Kind::Warn));
    assert_eq!(line_level("[INFO] ERROR inside"), Some(Kind::Info));
    assert_eq!(line_level("ERROR"), Some(Kind::Error));
    assert_eq!(line_level("2026-10-31 09:00:53 10.0.0.1 nothing"), None);
    assert_eq!(line_level(""), None);
    // un orario o un indirizzo non sono un livello
    assert_eq!(line_level("09:00:53"), None);
}

#[test]
fn empty_and_degenerate_lines() {
    assert_eq!(highlight(""), vec![]);
    assert_eq!(highlight("   "), vec![]);
    assert_eq!(highlight("::::...----"), vec![]);
    assert_eq!(highlight("-"), vec![]);
}

#[test]
fn a_very_long_line_is_scanned_in_one_pass() {
    // 12.500 indirizzi in 100.000 caratteri
    let l = "1.1.1.1 ".repeat(12_500);
    let v = highlight(&l);
    assert_eq!(v.len(), 12_500);
    assert_eq!(v[0], s(0, 7, Kind::Ip));
    assert_eq!(v[12_499], s(99_992, 99_999, Kind::Ip));
}
