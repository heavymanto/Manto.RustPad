//! I casi di prova del generatore: ogni scenario fa quello che dice, ed e' riconosciuto dal motore di Manto.RustPad (maschera
//! dell'orario, livelli). Scritti da ARCH.

use mrp_core::loghl::{first_level, Kind};
use mrp_core::timemask::{detect, TimeMask};
use mrp_loggen::{generate, Config, Scenario, Stats};

fn genera(scenario: Scenario, bytes: u64, seed: u64) -> (Vec<u8>, Stats) {
    let mut cfg = Config::new(scenario, bytes);
    cfg.seed = seed;
    genera_con(&cfg)
}

fn genera_con(cfg: &Config) -> (Vec<u8>, Stats) {
    let mut out = Vec::new();
    let st = generate(cfg, &mut out, &mut |_| {}).unwrap();
    (out, st)
}

fn righe(dati: &[u8]) -> Vec<&[u8]> {
    dati.split(|&b| b == b'\n').collect()
}

/// FNV-1a a 64 bit: un'impronta stabile dei byte (non c'e' una libreria di hash, e non serve).
fn impronta(dati: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in dati {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// La maschera riconosciuta sulle prime 300 righe (come fa l'App) e le chiavi di tutte le righe che la maschera legge.
fn orari(dati: &[u8]) -> (Option<TimeMask>, Vec<i64>) {
    let tutte = righe(dati);
    let campione: Vec<&[u8]> = tutte.iter().take(300).copied().collect();
    let Some(d) = detect(&campione) else {
        return (None, Vec::new());
    };
    let chiavi = tutte
        .iter()
        .filter_map(|l| d.mask.read(l.strip_suffix(b"\r").unwrap_or(l)))
        .collect();
    (Some(d.mask), chiavi)
}

#[test]
fn the_same_options_give_the_same_bytes_and_another_seed_does_not() {
    for s in Scenario::ALL {
        let (a, sa) = genera(s, 200_000, 1);
        let (b, sb) = genera(s, 200_000, 1);
        assert_eq!(a, b, "{}", s.name());
        assert_eq!(sa, sb);
        let (c, _) = genera(s, 200_000, 2);
        assert_ne!(a, c, "{}: un altro seme da un altro file", s.name());
    }
}

#[test]
fn the_bytes_are_pinned_so_a_change_of_the_generator_is_noticed() {
    // la prima riga di `service` e le impronte dei primi 256 KiB di ogni scenario (seme 1): se cambiano, i file generati da
    // versioni diverse non sono piu' gli stessi, e i numeri misurati su di essi non si confrontano piu'
    let (d, _) = genera(Scenario::Service, 1000, 1);
    let prima = String::from_utf8(righe(&d)[0].to_vec()).unwrap();
    assert_eq!(
        prima,
        "2026-01-01 00:00:00.070 +01:00 [ERR] Deadlock detected on inventory, transaction 12278575 rolled back"
    );
    let attese: [(Scenario, u64); 12] = [
        (Scenario::Service, 0xd89f0089d5981110),
        (Scenario::Bursts, 0x6ff697c3f980a138),
        (Scenario::Syslog, 0xe44f7b6f8610fd47),
        (Scenario::Access, 0xda5cf599d0ce0b94),
        (Scenario::Json, 0x657ddb5d58b91b50),
        (Scenario::Epoch, 0xd0f0d3aaa84bdd60),
        (Scenario::Unordered, 0xd64efdae058d8ec6),
        (Scenario::LongLines, 0xdab65c802bf3d0ec),
        (Scenario::Crlf, 0xf75e0237011378f9),
        (Scenario::Unicode, 0x9326bd68c04a3bf5),
        (Scenario::Report, 0x86fa9d2244cb8ddd),
        (Scenario::Text, 0x581e9c215799422a),
    ];
    for (s, atteso) in attese {
        // le raffiche di `bursts` non ci sono nei primi 256 KiB (sono poche ore): per quello scenario un ritmo lento e 2 MB
        let (d, _) = if s == Scenario::Bursts {
            let mut cfg = Config::new(s, 2_000_000);
            cfg.rate = 0.5;
            genera_con(&cfg)
        } else {
            genera(s, 256 * 1024, 1)
        };
        let h = impronta(&d);
        assert_eq!(h, atteso, "{}: 0x{h:016x}", s.name());
    }
}

#[test]
fn it_stops_on_a_whole_entry_just_after_the_size_and_counts_its_lines() {
    for s in Scenario::ALL {
        let (d, st) = genera(s, 300_000, 3);
        assert_eq!(st.bytes, d.len() as u64, "{}", s.name());
        assert!(st.bytes >= 300_000, "{}: {}", s.name(), st.bytes);
        // la voce piu' lunga: un carico di 200 KB (longlines), una pila di chiamate o una tabella
        assert!(st.bytes < 300_000 + 260_000, "{}: {}", s.name(), st.bytes);
        let fine: &[u8] = if s == Scenario::Crlf { b"\r\n" } else { b"\n" };
        assert!(d.ends_with(fine), "{}: finisce a meta' riga", s.name());
        let a_capo = d.iter().filter(|&&b| b == b'\n').count() as u64;
        assert_eq!(st.lines, a_capo, "{}", s.name());
        assert!(st.entries <= st.lines && st.entries > 0);
        assert!(
            std::str::from_utf8(&d).is_ok(),
            "{}: non e' UTF-8",
            s.name()
        );
        assert!(!d.contains(&0), "{}", s.name());
    }
}

#[test]
fn the_time_mask_of_each_scenario_is_the_one_the_engine_knows() {
    let attese = [
        (Scenario::Service, Some("yyyy-MM-dd HH:mm:ss.SSS")),
        (Scenario::Bursts, Some("yyyy-MM-dd HH:mm:ss.SSS")),
        (Scenario::Unordered, Some("yyyy-MM-dd HH:mm:ss.SSS")),
        (Scenario::LongLines, Some("yyyy-MM-dd HH:mm:ss.SSS")),
        (Scenario::Crlf, Some("yyyy-MM-dd HH:mm:ss.SSS")),
        (Scenario::Unicode, Some("yyyy-MM-dd HH:mm:ss.SSS")),
        (Scenario::Report, Some("yyyy-MM-dd HH:mm:ss.SSS")),
        (Scenario::Json, Some("yyyy-MM-ddTHH:mm:ss.SSS")),
        (Scenario::Access, Some("dd/MMM/yyyy:HH:mm:ss")),
        (Scenario::Syslog, Some("MMM d HH:mm:ss")),
        (Scenario::Epoch, Some("epoch_ms")),
        (Scenario::Text, None),
    ];
    for (s, pattern) in attese {
        let (d, _) = genera(s, 200_000, 1);
        let (mask, chiavi) = orari(&d);
        assert_eq!(
            mask.as_ref().map(|m| m.pattern().to_string()).as_deref(),
            pattern,
            "{}",
            s.name()
        );
        assert_eq!(chiavi.is_empty(), pattern.is_none(), "{}", s.name());
    }
}

#[test]
fn the_times_never_go_back_except_in_the_unordered_scenario() {
    for s in Scenario::ALL {
        let (d, _) = genera(s, 1_000_000, 1);
        let (mask, chiavi) = orari(&d);
        if mask.is_none() {
            continue;
        }
        let indietro = chiavi.windows(2).filter(|w| w[1] < w[0]).count();
        if s == Scenario::Unordered {
            // il 3% delle righe e' in ritardo: dopo una riga in ritardo il tempo sale di nuovo, quindi circa il 6% dei passi
            // e' un salto (indietro, poi avanti); si controlla solo che ce ne siano
            assert!(
                indietro * 100 > chiavi.len(),
                "unordered: solo {indietro} passi indietro su {}",
                chiavi.len()
            );
        } else {
            assert_eq!(indietro, 0, "{}: gli orari tornano indietro", s.name());
        }
        // e il file copre piu' di un'ora, o la timeline non avrebbe niente da mostrare
        let (primo, ultimo) = (chiavi.iter().min().unwrap(), chiavi.iter().max().unwrap());
        assert!(ultimo > primo, "{}", s.name());
    }
}

#[test]
fn the_service_log_has_five_levels_traces_and_rare_messages() {
    let (d, _) = genera(Scenario::Service, 2_000_000, 1);
    let testo = String::from_utf8(d).unwrap();
    let mut conti = [0u64; 4];
    for l in testo.lines() {
        match first_level(l) {
            Some(Kind::Debug) => conti[0] += 1,
            Some(Kind::Info) => conti[1] += 1,
            Some(Kind::Warn) => conti[2] += 1,
            Some(Kind::Error) => conti[3] += 1,
            _ => {}
        }
    }
    assert!(conti.iter().all(|&c| c > 0), "{conti:?}");
    // i livelli di una giornata normale: l'informazione domina, gli errori sono pochi
    let tot: u64 = conti.iter().sum();
    assert!(conti[1] * 100 > tot * 60, "{conti:?}");
    assert!(conti[3] * 100 < tot * 15, "{conti:?}");
    assert!(
        testo.contains("[FATAL] ") || testo.contains("[ERR] "),
        "niente errori"
    );
    assert!(
        testo.lines().any(|l| l.starts_with("   at ")),
        "niente pila di chiamate"
    );
    let rari = [
        "Checksum mismatch",
        "certificate has expired",
        "Replica ",
        "Clock skew",
        "Unusual login",
        "Maintenance window",
        "Data corruption",
    ];
    let quanti = testo
        .lines()
        .filter(|l| rari.iter().any(|r| l.contains(r)))
        .count();
    assert!(
        quanti >= 1 && quanti * 200 < testo.lines().count(),
        "rari: {quanti}"
    );
}

#[test]
fn the_error_storms_are_visible_and_stronger_in_the_bursts_scenario() {
    // un ritmo basso: 2 MB sono una trentina d'ore, con una raffica ogni 12 ore circa
    let mut cfg = Config::new(Scenario::Bursts, 2_000_000);
    cfg.rate = 0.5;
    let (d, _) = genera_con(&cfg);
    let testo = String::from_utf8(d).unwrap();
    let (mask, _) = orari(testo.as_bytes());
    let mask = mask.unwrap();
    // gli errori per finestra di 10 minuti: nel peggiore sono la maggioranza, nel resto la minoranza
    let mut finestre: std::collections::BTreeMap<i64, (u64, u64)> = Default::default();
    for l in testo.lines() {
        let Some(k) = mask.read(l.as_bytes()) else {
            continue;
        };
        let e = finestre.entry(k / 600_000).or_default();
        e.1 += 1;
        if first_level(l) == Some(Kind::Error) {
            e.0 += 1;
        }
    }
    let frazione = |e: &(u64, u64)| e.0 as f64 / e.1 as f64;
    let pieni: Vec<&(u64, u64)> = finestre.values().filter(|e| e.1 >= 20).collect();
    let peggiore = pieni.iter().map(|e| frazione(e)).fold(0.0, f64::max);
    let mut tutte: Vec<f64> = pieni.iter().map(|e| frazione(e)).collect();
    tutte.sort_by(f64::total_cmp);
    let mediana = tutte[tutte.len() / 2];
    assert!(
        peggiore > 0.45,
        "la raffica peggiore ha solo il {peggiore:.2} di errori"
    );
    assert!(mediana < 0.12, "la mediana e' del {mediana:.2}");
}

#[test]
fn the_crlf_scenario_has_windows_endings_and_the_others_do_not() {
    let (d, _) = genera(Scenario::Crlf, 100_000, 1);
    assert_eq!(
        d.iter().filter(|&&b| b == b'\n').count(),
        d.windows(2).filter(|w| w == b"\r\n").count()
    );
    for s in Scenario::ALL {
        if s != Scenario::Crlf {
            let (d, _) = genera(s, 100_000, 1);
            assert!(!d.contains(&b'\r'), "{}", s.name());
        }
    }
}

#[test]
fn the_long_lines_scenario_has_lines_from_2_kb_up_to_about_200_kb() {
    let (d, _) = genera(Scenario::LongLines, 4_000_000, 1);
    let lunghe: Vec<usize> = righe(&d)
        .iter()
        .map(|l| l.len())
        .filter(|&n| n > 2000)
        .collect();
    assert!(lunghe.len() >= 5, "{}", lunghe.len());
    let massimo = *lunghe.iter().max().unwrap();
    assert!(massimo > 20_000 && massimo < 215_000, "{massimo}");
    // le altre righe restano normali
    let normali = righe(&d).iter().filter(|l| l.len() < 300).count();
    assert!(normali * 10 > righe(&d).len() * 9);
    // senza lo scenario non ce ne sono
    let (d, _) = genera(Scenario::Service, 4_000_000, 1);
    assert!(righe(&d).iter().all(|l| l.len() < 1000));
}

#[test]
fn the_unicode_scenario_has_accents_cjk_cyrillic_and_emoji() {
    let (d, _) = genera(Scenario::Unicode, 1_000_000, 1);
    let t = String::from_utf8(d).unwrap();
    for pezzo in [
        "è",
        "ö",
        "ユーザー",
        "用户",
        "Пользователь",
        "🚀",
        "José",
        "李雷",
    ] {
        assert!(t.contains(pezzo), "manca {pezzo:?}");
    }
    // ma la maggior parte e' ancora ASCII
    assert!(t.is_char_boundary(t.len()));
    assert!(t.lines().filter(|l| l.is_ascii()).count() * 2 > t.lines().count());
}

#[test]
fn the_report_scenario_mixes_tables_and_timeless_lines() {
    let (d, _) = genera(Scenario::Report, 1_000_000, 1);
    let t = String::from_utf8(d).unwrap();
    let senza_orario = t.lines().filter(|l| !l.starts_with("20")).count();
    assert!(senza_orario * 5 > t.lines().count(), "{senza_orario}");
    assert!(t.lines().any(|l| l.starts_with('|') && l.ends_with('|')));
    assert!(t
        .lines()
        .any(|l| l.len() == 96 && l.bytes().all(|b| b == b'-')));
    assert!(t
        .lines()
        .any(|l| l.len() == 96 && l.bytes().all(|b| b == b'#')));
    assert!(t.contains(" REPORT 20"));
}

#[test]
fn the_text_scenario_has_no_digits_and_no_times() {
    let (d, _) = genera(Scenario::Text, 300_000, 1);
    assert!(!d.iter().any(|b| b.is_ascii_digit()));
    assert!(
        d.split(|&b| b == b'\n').any(|l| l.is_empty()),
        "nessun paragrafo"
    );
    let (mask, _) = orari(&d);
    assert!(mask.is_none());
}

#[test]
fn start_day_and_rate_decide_where_and_how_long_the_log_runs() {
    let mut cfg = Config::new(Scenario::Service, 200_000);
    cfg.start_day = mrp_loggen::parse_day("2031-03-15").unwrap();
    let (d, a) = genera_con(&cfg);
    assert!(d.starts_with(b"2031-03-15 00:00:00."));
    cfg.rate = 2.0;
    let (_, b) = genera_con(&cfg);
    // le stesse voci, ma dieci volte piu' diradate
    let durata = |s: &Stats| s.last_ms - s.first_ms;
    assert!(durata(&b) > durata(&a) * 5, "{} {}", durata(&a), durata(&b));
}

#[test]
fn the_progress_callback_is_called_for_big_outputs() {
    let cfg = Config::new(Scenario::Service, 70 << 20);
    let mut volte = Vec::new();
    // si butta via quello che si scrive: la dimensione e' quella che conta
    struct Nulla;
    impl std::io::Write for Nulla {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let st = generate(&cfg, &mut Nulla, &mut |n| volte.push(n)).unwrap();
    assert!(st.bytes >= 70 << 20);
    assert!(volte.len() >= 2, "{volte:?}");
    assert!(volte.windows(2).all(|w| w[1] > w[0]));
}

/// Quante voci per finestra di `ms` millisecondi ci sono nel file (le chiavi di `orari` sono in millisecondi).
fn per_finestra(chiavi: &[i64], ms: i64) -> std::collections::BTreeMap<i64, u64> {
    let mut m = std::collections::BTreeMap::new();
    for k in chiavi {
        *m.entry(k.div_euclid(ms)).or_insert(0u64) += 1;
    }
    m
}

#[test]
fn the_weekend_is_quieter_than_the_working_days() {
    // dal lunedi' 2026-01-05, un ritmo lento: 1,5 MB sono circa dieci giorni
    let mut cfg = Config::new(Scenario::Service, 1_500_000);
    cfg.start_day = mrp_loggen::parse_day("2026-01-05").unwrap();
    cfg.rate = 0.02;
    let (d, _) = genera_con(&cfg);
    let (_, chiavi) = orari(&d);
    let giorni = per_finestra(&chiavi, 86_400_000);
    let primo = *giorni.keys().next().unwrap();
    // i giorni interi: dal lunedi' (primo) per i primi sette (l'ultimo del file e' a meta')
    let conti: Vec<u64> = (0..7).map(|i| giorni[&(primo + i)]).collect();
    let feriali = conti[..5].iter().sum::<u64>() as f64 / 5.0;
    let festivi = conti[5..].iter().sum::<u64>() as f64 / 2.0;
    assert!(
        festivi < feriali * 0.75,
        "feriali {feriali}, festivi {festivi}: {conti:?}"
    );
    assert!(
        festivi > feriali * 0.4,
        "feriali {feriali}, festivi {festivi}: {conti:?}"
    );
    // ed e' sabato e domenica che sono piu' calmi, non altri due giorni
    for (i, &n) in conti[..5].iter().enumerate() {
        assert!(
            n as f64 > festivi,
            "il giorno {i} ({n}) non e' sopra i festivi ({festivi})"
        );
    }
}

#[test]
fn an_error_storm_makes_the_log_denser_too() {
    // ritmo lento, otto giorni circa: ci sono due o tre raffiche, ognuna con un ritmo 6 volte piu' alto
    let mut cfg = Config::new(Scenario::Service, 3_000_000);
    cfg.rate = 0.05;
    let (d, _) = genera_con(&cfg);
    let (_, chiavi) = orari(&d);
    let finestre = per_finestra(&chiavi, 600_000);
    let mut conti: Vec<u64> = finestre.values().copied().collect();
    conti.sort_unstable();
    let mediana = conti[conti.len() / 2] as f64;
    let massimo = *conti.last().unwrap() as f64;
    assert!(
        massimo > mediana * 3.2,
        "massimo {massimo}, mediana {mediana}"
    );
}
