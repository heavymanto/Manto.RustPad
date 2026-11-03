//! Test del junior per la maschera dell'orario (task H2): i casi che il test di accettazione di ARCH
//! (`timemask.rs`) non copre. L'anno delle maschere senza anno, il testo fra apici, la frazione
//! dell'epoch e le chiavi negative di `iso`. I valori attesi sono calcolati a mano col calendario
//! gregoriano (2024-02-29, per esempio: 54 anni dal 1970 con 13 bisestili, piu' 31 di gennaio e 28
//! di febbraio = 19782 giorni = 1_709_164_800_000 millisecondi), mai letti dall'output del codice.

use mrp_core::timemask::{iso, TimeMask};

#[test]
fn with_year_changes_the_year_of_masks_without_one() {
    let m = TimeMask::parse("MMM d HH:mm:ss", 0)
        .unwrap()
        .with_year(2024);
    assert_eq!(m.read(b"Feb 29 00:00:00 x"), Some(1_709_164_800_000));
    // di fabbrica l'anno e' il 2000, che e' bisestile
    let m = TimeMask::parse("MMM d HH:mm:ss", 0).unwrap();
    assert_eq!(m.read(b"Feb 29 00:00:00 x"), Some(951_782_400_000));
    // il 2023 non e' bisestile
    let m = TimeMask::parse("MMM d HH:mm:ss", 0)
        .unwrap()
        .with_year(2023);
    assert_eq!(m.read(b"Feb 29 00:00:00 x"), None);
}

#[test]
fn text_between_single_quotes_is_literal() {
    let m = TimeMask::parse("'yyyy' HH:mm", 0).unwrap();
    assert!(!m.has_date());
    assert_eq!(m.pattern(), "'yyyy' HH:mm");
    // 12:30 sono 45_000_000 millisecondi del giorno
    assert_eq!(m.read(b"yyyy 12:30 x"), Some(45_000_000));
    // un numero non e' il letterale yyyy
    assert_eq!(m.read(b"2026 12:30 x"), None);
}

#[test]
fn the_epoch_fraction_is_optional() {
    let m = TimeMask::parse("epoch_s", 0).unwrap();
    // un miliardo di secondi, con mezzo secondo di frazione
    assert_eq!(m.read(b"1000000000.5 x"), Some(1_000_000_000_500));
    // nove cifre bastano per i secondi, otto no
    assert_eq!(m.read(b"100000000 x"), Some(100_000_000_000));
    assert_eq!(m.read(b"10000000 x"), None);
    // il punto senza cifre dietro resta testo che continua
    assert_eq!(m.read(b"1000000000.info x"), Some(1_000_000_000_000));
}

#[test]
fn iso_writes_negative_keys() {
    assert_eq!(iso(-1, true), "1969-12-31 23:59:59.999");
    assert_eq!(iso(-86_400_000, true), "1969-12-31 00:00:00");
    assert_eq!(iso(-1, false), "23:59:59.999");
}
