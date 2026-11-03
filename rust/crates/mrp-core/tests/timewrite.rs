//! Scrivere una chiave nel formato della maschera (`TimeMask::write`), l'unita' del formato (`unit_ms`) e le funzioni di calendario
//! (`civil`, `key_from_civil`, `days_in_month`, `weekday`). Scritti da ARCH: i valori attesi sono calcolati con `calendar` di Python
//! (un calcolo indipendente), mai dal codice.

use mrp_core::timemask::{civil, days_in_month, key_from_civil, weekday, TimeMask, PRESETS};

fn m(p: &str) -> TimeMask {
    TimeMask::parse(p, 0).unwrap_or_else(|e| panic!("{p:?}: {e:?}"))
}

#[test]
fn write_gives_the_key_in_the_format_of_the_mask() {
    let k = 1793498400877; // 2026-11-01 02:00:00.877
    assert_eq!(
        m("yyyy-MM-dd HH:mm:ss.SSS").write(k),
        "2026-11-01 02:00:00.877"
    );
    assert_eq!(m("yyyy-MM-dd HH:mm:ss").write(k), "2026-11-01 02:00:00");
    assert_eq!(m("dd/MM/yy HH:mm").write(k), "01/11/26 02:00");
    assert_eq!(m("yyyy-MM-dd").write(k), "2026-11-01");
    // il mese in lettere (inglese) e il giorno a una cifra
    let k2 = 1772719629005; // 2026-03-05 14:07:09.005
    assert_eq!(m("dd/MMM/yyyy:HH:mm:ss").write(k2), "05/Mar/2026:14:07:09");
    assert_eq!(m("d MMM yyyy").write(k2), "5 Mar 2026");
    // la sola ora: l'ora del giorno
    assert_eq!(m("HH:mm:ss.SSS").write(k2), "14:07:09.005");
    // epoch
    assert_eq!(m("epoch_s").write(k), "1793498400");
    assert_eq!(m("epoch_ms").write(k), "1793498400877");
    // un pattern con un letterale fra apici
    assert_eq!(m("yyyy-MM-dd'T'HH:mm:ss").write(k), "2026-11-01T02:00:00");
}

#[test]
fn what_write_gives_read_reads_back_for_every_known_format() {
    // l'orario letto dall'esempio, riscritto e riletto: stessa chiave (a meno di cio' che il formato non scrive)
    for p in PRESETS {
        let mask = TimeMask::parse(p.pattern, p.within).unwrap();
        let k = mask.read(p.example.as_bytes()).expect(p.pattern);
        let scritto = mask.write(k);
        assert_eq!(
            mask.read(scritto.as_bytes()),
            Some(k),
            "{}: {scritto:?}",
            p.pattern
        );
    }
}

#[test]
fn the_unit_is_the_smallest_thing_the_mask_writes() {
    assert_eq!(m("yyyy-MM-dd HH:mm:ss.SSS").unit_ms(), 1);
    assert_eq!(m("yyyy-MM-dd HH:mm:ss").unit_ms(), 1000);
    assert_eq!(m("yyyy-MM-dd HH:mm").unit_ms(), 60_000);
    assert_eq!(m("yyyy-MM-dd HH").unit_ms(), 3_600_000);
    assert_eq!(m("yyyy-MM-dd").unit_ms(), 86_400_000);
    assert_eq!(m("epoch_s").unit_ms(), 1000);
    assert_eq!(m("epoch_ms").unit_ms(), 1);
}

#[test]
fn the_calendar_functions_agree_with_python() {
    // (anno, mese, giorno) -> mezzanotte in ms e giorno della settimana (lunedi' = 0)
    for (a, mo, g, key, sett) in [
        (2026, 11, 1, 1793491200000_i64, 6),
        (2024, 2, 29, 1709164800000, 3),
        (1970, 1, 1, 0, 3),
        (1969, 12, 31, -86400000, 2),
        (2000, 3, 1, 951868800000, 2),
        (2100, 2, 28, 4107456000000, 6),
    ] {
        assert_eq!(key_from_civil(a, mo, g), key, "{a}-{mo}-{g}");
        assert_eq!(civil(key), (a, mo, g), "{a}-{mo}-{g}");
        // qualunque ora del giorno e' lo stesso giorno
        assert_eq!(civil(key + 86_399_999), (a, mo, g));
        assert_eq!(weekday(key), sett, "{a}-{mo}-{g}");
        assert_eq!(weekday(key + 43_200_000), sett);
    }
    // i giorni dei mesi, bisestili compresi (il 2100 non lo e', il 2000 si')
    assert_eq!(days_in_month(2024, 2), 29);
    assert_eq!(days_in_month(2023, 2), 28);
    assert_eq!(days_in_month(2100, 2), 28);
    assert_eq!(days_in_month(2000, 2), 29);
    assert_eq!(days_in_month(2026, 4), 30);
    assert_eq!(days_in_month(2026, 12), 31);
}
