//! La maschera dell'orario (F1): come si legge l'orario scritto in una riga di log (`TimeMask`), come si riconosce da qualche riga
//! (`detect`) e come si scrive una chiave (`iso`). Scritti da ARCH prima dell'implementazione: tutti i valori attesi sono millisecondi
//! dal 1970 calcolati con `calendar.timegm` di Python (un calcolo indipendente), mai dal codice.

use mrp_core::timemask::{detect, iso, MaskError, TimeMask};

fn maschera(p: &str) -> TimeMask {
    TimeMask::parse(p, 0).unwrap_or_else(|e| panic!("{p:?}: {e:?}"))
}

fn leggi(p: &str, riga: &str) -> Option<i64> {
    maschera(p).read(riga.as_bytes())
}

// ---------- la lettura ----------

#[test]
fn iso_with_milliseconds_and_the_text_after_it_is_ignored() {
    assert_eq!(
        leggi(
            "yyyy-MM-dd HH:mm:ss.SSS",
            "2026-11-01 02:00:00.877 [INFO ] [main] Job started"
        ),
        Some(1793498400877)
    );
    // la frazione: da una a nove cifre, si contano i millisecondi (le altre cifre si lasciano)
    let p = "yyyy-MM-dd HH:mm:ss.SSS";
    assert_eq!(leggi(p, "2026-11-01 02:00:00.5 x"), Some(1793498400500));
    assert_eq!(leggi(p, "2026-11-01 02:00:00.87 x"), Some(1793498400870));
    assert_eq!(
        leggi(p, "2026-11-01 02:00:00.877123456 x"),
        Some(1793498400877)
    );
    // senza frazione non corrisponde a questa maschera
    assert_eq!(leggi(p, "2026-11-01 02:00:00 x"), None);
    // la virgola e' un'altra maschera (log4j)
    assert_eq!(leggi(p, "2026-11-01 02:00:00,877 x"), None);
    assert_eq!(
        leggi("yyyy-MM-dd HH:mm:ss,SSS", "2026-11-01 02:00:00,877 x"),
        Some(1793498400877)
    );
}

#[test]
fn t_separator_and_the_zone_that_is_read_and_ignored() {
    let p = "yyyy-MM-ddTHH:mm:ssZ";
    for riga in [
        "2026-11-01T02:00:00Z x",
        "2026-11-01T02:00:00+01:00 x",
        "2026-11-01T02:00:00+0100 x",
        "2026-11-01T02:00:00-05:30 x",
        "2026-11-01T02:00:00 x",
    ] {
        assert_eq!(leggi(p, riga), Some(1793498400000), "{riga}");
    }
}

#[test]
fn date_only_time_only_and_the_other_orders() {
    assert_eq!(
        leggi("yyyy-MM-dd", "2026-11-01 una riga"),
        Some(1793491200000)
    );
    assert_eq!(
        leggi("dd/MM/yyyy HH:mm:ss", "31/10/2026 23:59:59 x"),
        Some(1793491199000)
    );
    assert_eq!(
        leggi("yy-MM-dd HH:mm:ss", "26-01-05 08:09:10 x"),
        Some(1767600550000)
    );
    // solo l'orario: i millisecondi del giorno
    let m = maschera("HH:mm:ss.SSS");
    assert!(!m.has_date());
    assert_eq!(m.read(b"02:03:04.567 [x] y"), Some(7384567));
    assert!(maschera("yyyy-MM-dd HH:mm").has_date());
}

#[test]
fn month_names_syslog_and_the_year_that_the_mask_does_not_have() {
    // Apache: il mese a tre lettere, maiuscole indifferenti
    let p = "dd/MMM/yyyy:HH:mm:ss";
    assert_eq!(leggi(p, "01/Nov/2026:02:00:00 +0100"), Some(1793498400000));
    assert_eq!(leggi(p, "01/nov/2026:02:00:00"), Some(1793498400000));
    assert_eq!(leggi(p, "01/NOV/2026:02:00:00"), Some(1793498400000));
    assert_eq!(leggi(p, "01/Foo/2026:02:00:00"), None);
    // syslog: senza anno (di fabbrica il 2000, bisestile), uno o due spazi prima del giorno
    let s = maschera("MMM d HH:mm:ss");
    assert_eq!(
        s.read(b"Nov  1 02:03:04 host sshd[1]: x"),
        Some(973044184000)
    );
    assert_eq!(s.read(b"Nov 1 02:03:04 host"), Some(973044184000));
    assert_eq!(s.read(b"Nov   1 02:03:04 host"), Some(973044184000));
    assert_eq!(s.read(b"Feb 29 00:00:00 host"), Some(951782400000));
    // con l'anno dato
    let s2026 = maschera("MMM d HH:mm:ss").with_year(2026);
    assert_eq!(s2026.read(b"Nov  1 02:03:04 host"), Some(1793498584000));
    assert_eq!(
        s2026.read(b"Feb 29 00:00:00 host"),
        None,
        "il 2026 non e' bisestile"
    );
}

#[test]
fn one_or_two_digit_days_and_exact_widths() {
    let d = maschera("yyyy-MM-d HH:mm:ss");
    assert_eq!(d.read(b"2026-11-1 02:03:04 x"), Some(1793498584000));
    assert_eq!(d.read(b"2026-11-11 02:03:04 x"), Some(1794362584000));
    // dd vuole due cifre
    assert_eq!(leggi("yyyy-MM-dd HH:mm:ss", "2026-11-1 02:03:04 x"), None);
}

#[test]
fn epoch_seconds_and_milliseconds() {
    assert_eq!(leggi("epoch_s", "1793498400 INFO x"), Some(1793498400000));
    assert_eq!(
        leggi("epoch_s", "1793498400.250 INFO x"),
        Some(1793498400250)
    );
    assert_eq!(
        leggi("epoch_ms", "1793498400123 INFO x"),
        Some(1793498400123)
    );
    // il numero deve avere la lunghezza di un epoch: 9-10 cifre per i secondi, 12-13 per i millisecondi
    assert_eq!(leggi("epoch_s", "12345 INFO x"), None);
    assert_eq!(leggi("epoch_s", "17934984001234 x"), None);
    assert_eq!(leggi("epoch_ms", "1793498400 x"), None);
    assert!(maschera("epoch_s").has_date());
}

#[test]
fn impossible_values_do_not_match() {
    let p = "yyyy-MM-dd HH:mm:ss";
    for riga in [
        "2026-13-01 00:00:00",
        "2026-00-10 00:00:00",
        "2026-11-31 00:00:00",
        "2026-02-29 00:00:00",
        "2026-11-00 00:00:00",
        "2026-11-01 24:00:00",
        "2026-11-01 00:60:00",
        "2026-11-01 00:00:61",
    ] {
        assert_eq!(leggi(p, riga), None, "{riga}");
    }
    // i casi limite validi: il 29 febbraio di un anno bisestile, il secondo 60 (vale 59,5)
    assert_eq!(leggi(p, "2024-02-29 12:30:15"), Some(1709209815000));
    assert_eq!(leggi(p, "2026-06-30 23:59:60"), Some(1782863999500));
}

#[test]
fn the_calendar_is_right_over_the_centuries() {
    let p = "yyyy-MM-dd HH:mm:ss.SSS";
    assert_eq!(leggi(p, "1970-01-01 00:00:00.000"), Some(0));
    assert_eq!(leggi(p, "1970-01-01 00:00:01.000"), Some(1000));
    assert_eq!(leggi(p, "1999-12-31 23:59:59.999"), Some(946684799999));
    assert_eq!(leggi(p, "2000-03-01 00:00:00.000"), Some(951868800000));
    assert_eq!(leggi(p, "2100-03-01 00:00:00.000"), Some(4107542400000));
    assert_eq!(
        leggi(p, "2100-02-29 00:00:00.000"),
        None,
        "il 2100 non e' bisestile"
    );
    assert_eq!(leggi(p, "2400-02-29 00:00:00.000"), Some(13574563200000));
    assert_eq!(leggi(p, "1900-01-01 00:00:00.000"), Some(-2208988800000));
}

#[test]
fn lines_that_do_not_match() {
    let p = "yyyy-MM-dd HH:mm:ss";
    assert_eq!(leggi(p, ""), None);
    assert_eq!(leggi(p, "2026-11-01 02:00"), None, "troppo corta");
    assert_eq!(leggi(p, "una riga di testo qualunque"), None);
    assert_eq!(leggi(p, "2026/11/01 02:00:00"), None, "altri separatori");
    // la maschera ancorata all'inizio non trova un orario piu' avanti nella riga
    assert_eq!(leggi(p, "xx 2026-11-01 02:00:00"), None);
    // byte che non sono testo
    assert_eq!(
        TimeMask::parse(p, 0).unwrap().read(&[0xFF, 0xFE, 0x00]),
        None
    );
}

#[test]
fn a_window_lets_the_time_start_later_in_the_line() {
    let m = TimeMask::parse("dd/MMM/yyyy:HH:mm:ss", 40).unwrap();
    assert_eq!(m.within(), 40);
    for riga in [
        r#"127.0.0.1 - - [01/Nov/2026:02:00:00 +0100] "GET / HTTP/1.1" 200 12"#,
        r#"10.20.4.137 - bob [01/Nov/2026:02:00:00 +0100] "GET /a HTTP/1.1" 200 12"#,
        "[01/Nov/2026:02:00:00] corta",
    ] {
        assert_eq!(m.read(riga.as_bytes()), Some(1793498400000), "{riga}");
    }
    // oltre la finestra non lo cerca
    let lontano = format!("{}01/Nov/2026:02:00:00", "x".repeat(60));
    assert_eq!(m.read(lontano.as_bytes()), None);
    assert_eq!(m.pattern(), "dd/MMM/yyyy:HH:mm:ss");
}

#[test]
fn a_bad_pattern_is_refused_with_a_reason() {
    assert_eq!(TimeMask::parse("", 0).err(), Some(MaskError::Incomplete));
    assert_eq!(
        TimeMask::parse("[] - []", 0).err(),
        Some(MaskError::Incomplete)
    );
    // mancano pezzi: l'anno e il mese senza il giorno, la sola ora senza i minuti
    assert_eq!(
        TimeMask::parse("yyyy-MM", 0).err(),
        Some(MaskError::Incomplete)
    );
    assert_eq!(TimeMask::parse("HH", 0).err(), Some(MaskError::Incomplete));
    // lunghezze che non esistono
    assert_eq!(
        TimeMask::parse("yyyyy-MM-dd", 0).err(),
        Some(MaskError::BadToken("yyyyy".to_string()))
    );
    assert_eq!(
        TimeMask::parse("yyyy-MM-dd HH:mm:ss.SS", 0).err(),
        Some(MaskError::BadToken("SS".to_string()))
    );
    assert_eq!(
        TimeMask::parse("yyyy-MMMM-dd", 0).err(),
        Some(MaskError::BadToken("MMMM".to_string()))
    );
    assert!(TimeMask::parse("yyyy-MM-dd HH:mm", 0).is_ok());
    assert!(TimeMask::parse("epoch_ms", 0).is_ok());
}

// ---------- la scrittura ----------

#[test]
fn iso_writes_the_key_for_the_status_bar() {
    assert_eq!(iso(1793498400877, true), "2026-11-01 02:00:00.877");
    // i millisecondi si scrivono solo se ci sono
    assert_eq!(iso(1793498400000, true), "2026-11-01 02:00:00");
    assert_eq!(iso(0, true), "1970-01-01 00:00:00");
    assert_eq!(iso(-2208988800000, true), "1900-01-01 00:00:00");
    // senza data: l'orario del giorno
    assert_eq!(iso(7384567, false), "02:03:04.567");
    assert_eq!(iso(7384000, false), "02:03:04");
    // riletto con la maschera ISO, ritorna la stessa chiave
    let m = maschera("yyyy-MM-dd HH:mm:ss.SSS");
    for k in [
        0i64,
        951868799999,
        1793498400877,
        4107542400000,
        13574563200000,
    ] {
        let s = format!(
            "{}{}",
            iso(k, true),
            if k % 1000 == 0 { ".000" } else { "" }
        );
        assert_eq!(m.read(s.as_bytes()), Some(k), "{s}");
    }
}

// ---------- il riconoscimento ----------

fn righe(v: &[&str]) -> Vec<Vec<u8>> {
    v.iter().map(|s| s.as_bytes().to_vec()).collect()
}

fn rileva(v: &[&str]) -> Option<(String, usize, usize, usize)> {
    let r = righe(v);
    let rif: Vec<&[u8]> = r.iter().map(Vec::as_slice).collect();
    detect(&rif).map(|d| {
        (
            d.mask.pattern().to_string(),
            d.mask.within(),
            d.matched,
            d.total,
        )
    })
}

fn ripeti(riga: &str, n: usize) -> Vec<String> {
    (0..n)
        .map(|i| riga.replace("{}", &format!("{:02}", i % 60)))
        .collect()
}

fn come_str(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

#[test]
fn detects_the_common_formats() {
    let casi: [(&str, &str); 9] = [
        (
            "2026-11-01 02:00:{} [INFO ] [main] Job started",
            "yyyy-MM-dd HH:mm:ss",
        ),
        (
            "2026-11-01 02:00:{}.877 [INFO ] [main] Job started",
            "yyyy-MM-dd HH:mm:ss.SSS",
        ),
        (
            "2026-11-01 02:00:{},877 INFO main Job started",
            "yyyy-MM-dd HH:mm:ss,SSS",
        ),
        ("2026-11-01T02:00:{}.123Z msg", "yyyy-MM-ddTHH:mm:ss.SSS"),
        ("01/11/2026 02:00:{} msg", "dd/MM/yyyy HH:mm:ss"),
        ("2026/11/01 02:00:{} msg", "yyyy/MM/dd HH:mm:ss"),
        ("02:03:{}.567 [x] msg", "HH:mm:ss.SSS"),
        ("17934984{} INFO x", "epoch_s"),
        ("1793498400{}7 INFO x", "epoch_ms"),
    ];
    for (modello, atteso) in casi {
        let v = ripeti(modello, 12);
        let (pattern, within, matched, total) =
            rileva(&come_str(&v)).unwrap_or_else(|| panic!("niente per {modello:?}"));
        assert_eq!(pattern, atteso, "{modello}");
        assert_eq!(within, 0, "{modello}: l'orario sta all'inizio");
        assert_eq!((matched, total), (12, 12), "{modello}");
    }
}

#[test]
fn detects_syslog_and_apache_where_the_time_is_not_at_the_start() {
    let v = vec![
        "Nov  1 02:03:04 host sshd[100]: accepted",
        "Nov  1 02:03:05 host sshd[100]: closed",
        "Nov 11 02:03:06 host cron[7]: run",
        "Nov 11 02:03:07 host cron[7]: done",
    ];
    let (p, within, matched, _) = rileva(&v).unwrap();
    assert_eq!(p, "MMM d HH:mm:ss");
    assert_eq!((within, matched), (0, 4));
    // Apache: gli indirizzi hanno lunghezze diverse, l'orario non e' mai alla stessa colonna
    let v = vec![
        r#"127.0.0.1 - - [01/Nov/2026:02:00:00 +0100] "GET / HTTP/1.1" 200 12"#,
        r#"10.20.4.137 - bob [01/Nov/2026:02:00:01 +0100] "GET /a HTTP/1.1" 200 12"#,
        r#"192.168.50.8 - - [01/Nov/2026:02:00:02 +0100] "POST /b HTTP/1.1" 500 3"#,
        r#"::1 - - [01/Nov/2026:02:00:03 +0100] "GET /c HTTP/1.1" 404 9"#,
    ];
    let (p, within, matched, total) = rileva(&v).unwrap();
    assert_eq!(p, "dd/MMM/yyyy:HH:mm:ss");
    assert!(
        within >= 20,
        "la finestra deve coprire 'xxx.xxx.xxx.xxx - - [': {within}"
    );
    assert_eq!((matched, total), (4, 4));
}

#[test]
fn stack_trace_lines_and_empty_lines_do_not_stop_the_detection() {
    let mut v: Vec<String> = ripeti("2026-11-01 02:00:{}.877 [ERROR] [main] boom", 10);
    v.insert(3, "java.lang.IllegalStateException: boom".to_string());
    v.insert(4, "\tat com.example.Main.run(Main.java:41)".to_string());
    v.insert(5, "\tat com.example.Main.main(Main.java:12)".to_string());
    v.insert(6, String::new());
    let (p, _, matched, total) = rileva(&come_str(&v)).unwrap();
    assert_eq!(p, "yyyy-MM-dd HH:mm:ss.SSS");
    // le righe vuote non contano nel totale
    assert_eq!((matched, total), (10, 13));
}

#[test]
fn nothing_to_detect_gives_none() {
    assert!(rileva(&[]).is_none());
    assert!(rileva(&["", ""]).is_none());
    assert!(rileva(&["hello world", "niente orari qui", "ne' qui"]).is_none());
    // una data impossibile non e' un orario
    assert!(rileva(&["2026-13-45 99:99:99 x", "2026-13-46 99:99:99 y"]).is_none());
}

#[test]
fn it_needs_a_fair_share_of_matching_lines() {
    // 3 righe su 10 con l'orario bastano (le altre possono essere tracce), 2 su 10 no
    let mut v: Vec<String> = ripeti("2026-11-01 02:00:{} [INFO ] x", 3);
    v.extend((0..7).map(|i| format!("\tat riga {i}")));
    assert!(rileva(&come_str(&v)).is_some());
    let mut v: Vec<String> = ripeti("2026-11-01 02:00:{} [INFO ] x", 2);
    v.extend((0..8).map(|i| format!("\tat riga {i}")));
    assert!(rileva(&come_str(&v)).is_none());
}

#[test]
fn the_more_specific_format_wins_a_tie() {
    // tutte le righe hanno i millisecondi: vince la maschera con i millisecondi, non quella senza
    let v = ripeti("2026-11-01 02:00:{}.877 x", 8);
    assert_eq!(rileva(&come_str(&v)).unwrap().0, "yyyy-MM-dd HH:mm:ss.SSS");
    // alcune senza: la maschera senza millisecondi le prende tutte
    let mut v = ripeti("2026-11-01 02:00:{}.877 x", 4);
    v.extend(ripeti("2026-11-01 02:01:{} y", 4));
    let (p, _, matched, total) = rileva(&come_str(&v)).unwrap();
    assert_eq!(p, "yyyy-MM-dd HH:mm:ss");
    assert_eq!((matched, total), (8, 8));
}

#[test]
fn a_detected_mask_reads_the_lines_it_was_detected_on() {
    let v = ripeti("2026-11-01 02:00:{}.877 [INFO ] x", 5);
    let r = righe(&come_str(&v));
    let rif: Vec<&[u8]> = r.iter().map(Vec::as_slice).collect();
    let d = detect(&rif).unwrap();
    assert_eq!(d.mask.read(&r[0]), Some(1793498400877));
    assert_eq!(d.mask.read(&r[1]), Some(1793498401877));
}

// ---------- i log con grandi tabelle, la maschera preferita e i formati noti ----------

use mrp_core::timemask::{detect_with, PRESETS};

#[test]
fn a_log_with_big_tables_between_the_lines_with_a_time_is_still_recognised() {
    // il caso vero: un log di backtest con 5 righe con l'orario in mezzo a 195 righe di tabella (il 2,5%)
    let mut v: Vec<String> = Vec::new();
    for i in 0..5 {
        v.push(format!(
            "2026-11-03 11:14:{:02}.738 +01:00 [INF] BALANCE USDT: 0",
            30 + i
        ));
        for k in 0..39 {
            v.push(format!(
                "|  [SEC-{k:02}] OPEN NEW POSITION   |   NONE --> DOWN   |"
            ));
        }
    }
    let (p, within, matched, total) = rileva(&come_str(&v)).expect("va riconosciuto");
    assert_eq!(p, "yyyy-MM-dd HH:mm:ss.SSS");
    assert_eq!((within, matched, total), (0, 5, 200));
}

#[test]
fn two_lines_with_a_time_in_a_long_text_are_not_enough() {
    let mut v: Vec<String> = ripeti("2026-11-01 02:00:{} [INFO ] x", 2);
    v.extend((0..198).map(|i| format!("riga di tabella {i}")));
    assert!(rileva(&come_str(&v)).is_none());
}

#[test]
fn weak_formats_need_a_good_share_of_the_lines() {
    // dieci cifre all'inizio di 3 righe su 100 (degli id, non degli epoch): niente
    let mut v: Vec<String> = ripeti("17934984{} e' un identificativo", 3);
    v.extend((0..97).map(|i| format!("altra riga {i}")));
    assert!(rileva(&come_str(&v)).is_none());
    // 12:30:45 all'inizio di 3 righe su 100: niente
    let mut v: Vec<String> = ripeti("12:30:{} una riga", 3);
    v.extend((0..97).map(|i| format!("altra riga {i}")));
    assert!(rileva(&come_str(&v)).is_none());
    // ma se sono la meta' si'
    let mut v: Vec<String> = ripeti("12:30:{} una riga", 10);
    v.extend((0..10).map(|i| format!("altra riga {i}")));
    assert_eq!(rileva(&come_str(&v)).unwrap().0, "HH:mm:ss");
}

#[test]
fn the_preferred_mask_wins_when_it_reads_enough_lines_and_otherwise_detection_goes_on() {
    let preferita = TimeMask::parse("yyyy-MM-dd HH:mm:ss", 0).unwrap();
    // il log ha anche i millisecondi: la riconoscita da sola sarebbe quella con i millisecondi, la preferita vince
    let v = ripeti("2026-11-01 02:00:{}.877 x", 8);
    let r = righe(&come_str(&v));
    let rif: Vec<&[u8]> = r.iter().map(Vec::as_slice).collect();
    assert_eq!(
        detect(&rif).unwrap().mask.pattern(),
        "yyyy-MM-dd HH:mm:ss.SSS"
    );
    assert_eq!(
        detect_with(&rif, Some(&preferita)).unwrap().mask.pattern(),
        "yyyy-MM-dd HH:mm:ss"
    );
    // se la preferita non legge niente di questo file, si riconosce da sola
    let v = ripeti("01/11/2026 02:00:{} x", 8);
    let r = righe(&come_str(&v));
    let rif: Vec<&[u8]> = r.iter().map(Vec::as_slice).collect();
    assert_eq!(
        detect_with(&rif, Some(&preferita)).unwrap().mask.pattern(),
        "dd/MM/yyyy HH:mm:ss"
    );
    assert_eq!(
        detect_with(&rif, None).unwrap().mask.pattern(),
        "dd/MM/yyyy HH:mm:ss"
    );
}

#[test]
fn the_known_formats_are_valid_distinct_and_their_examples_are_read_by_them() {
    let mut visti = Vec::new();
    for pr in PRESETS {
        assert!(!visti.contains(&pr.pattern), "{} due volte", pr.pattern);
        visti.push(pr.pattern);
        let m = TimeMask::parse(pr.pattern, pr.within)
            .unwrap_or_else(|e| panic!("{}: {e:?}", pr.pattern));
        assert!(
            m.read(pr.example.as_bytes()).is_some(),
            "{}: l'esempio {:?}",
            pr.pattern,
            pr.example
        );
    }
    // e il primo e' il piu' comune
    assert_eq!(PRESETS[0].pattern, "yyyy-MM-dd HH:mm:ss.SSS");
}

#[test]
fn a_strong_format_is_not_hidden_by_a_weak_one_that_reads_more_lines() {
    // 5 righe con data e ora e 8 con un numero di dieci cifre (epoch debole, ma piu' righe) in 200: vince la forte
    let mut v: Vec<String> = ripeti("2026-11-03 11:14:{}.738 +01:00 [INF] job", 5);
    v.extend(ripeti("rif 17934984{} e basta", 8));
    v.extend((0..187).map(|i| format!("riga di tabella {i}")));
    let (p, _, matched, total) = rileva(&come_str(&v)).expect("la forte va riconosciuta");
    assert_eq!(p, "yyyy-MM-dd HH:mm:ss.SSS");
    assert_eq!((matched, total), (5, 200));
}

// ---------- dove sta l'orario nella riga (per colorarlo) ----------

#[test]
fn find_gives_the_bytes_the_time_occupies_and_the_same_key_as_read() {
    let m = maschera("yyyy-MM-dd HH:mm:ss.SSS");
    let riga = "2026-11-01 02:00:00.877 [INFO ] x";
    assert_eq!(m.find(riga.as_bytes()), Some((0, 23, 1793498400877)));
    assert_eq!(m.read(riga.as_bytes()), Some(1793498400877));
    // il fuso (Z) e' dentro l'orario, il testo dopo no
    let z = maschera("yyyy-MM-dd'T'HH:mm:ss.SSSZ");
    assert_eq!(
        z.find(b"2026-11-01T02:00:00.877Z rest"),
        Some((0, 24, 1793498400877))
    );
    // la finestra: l'orario che comincia piu' avanti ha l'inizio giusto
    let w = TimeMask::parse("yyyy-MM-dd HH:mm:ss", 6).unwrap();
    assert_eq!(
        w.find(b"[xyz] 2026-11-01 02:00:00 end"),
        Some((6, 25, 1793498400000))
    );
    // e fuori dalla finestra, niente
    assert_eq!(m.find(b"xx 2026-11-01 02:00:00.877"), None);
    // l'epoch: le cifre (e la frazione, se c'e') sono l'orario
    let e = maschera("epoch_s");
    assert_eq!(e.find(b"1793498400.877 x"), Some((0, 14, 1793498400877)));
    assert_eq!(e.find(b"1793498400 x"), Some((0, 10, 1793498400000)));
}

#[test]
fn a_log_that_starts_with_the_date_and_minutes_is_recognised_with_the_minutes() {
    // il log di un backtest dove la riga comincia col tempo di simulazione, senza secondi: `2019-01-01 00:00 - MEXC - ...`
    let righe: Vec<String> = (0..30)
        .map(|i| {
            format!(
                "2019-01-{:02} {:02}:00 - MEXC - BTC_USDT - SPOT - MARKET PRICE: 3701,23",
                1 + i / 24,
                i % 24
            )
        })
        .collect();
    let rif: Vec<&[u8]> = righe.iter().map(|r| r.as_bytes()).collect();
    let d = detect(&rif).expect("riconosciuto");
    // con le ore e i minuti, non solo la data (due righe al giorno e' la stessa data: la sola data perderebbe l'ora)
    assert_eq!(d.mask.pattern(), "yyyy-MM-dd HH:mm");
    assert_eq!((d.matched, d.total), (30, 30));
    // 2019-01-01 05:00 in millisecondi dal 1970 (calendar.timegm di Python)
    assert_eq!(d.mask.read(righe[5].as_bytes()), Some(1546318800000));
    // lo stesso con giorno/mese/anno
    let it: Vec<String> = (0..30)
        .map(|i| format!("{:02}/01/2019 {:02}:30 - riga", 1 + i / 24, i % 24))
        .collect();
    let rif: Vec<&[u8]> = it.iter().map(|r| r.as_bytes()).collect();
    assert_eq!(detect(&rif).unwrap().mask.pattern(), "dd/MM/yyyy HH:mm");
    // se la riga ha i secondi vince il formato con i secondi, come prima
    let s: Vec<String> = (0..30)
        .map(|i| format!("2019-01-01 {:02}:00:{:02} riga", i % 24, i))
        .collect();
    let rif: Vec<&[u8]> = s.iter().map(|r| r.as_bytes()).collect();
    assert_eq!(detect(&rif).unwrap().mask.pattern(), "yyyy-MM-dd HH:mm:ss");
}
