//! Test del junior per la colorazione dei log (fase F2): casi oltre a quelli di `tests/loghl.rs` — la frazione
//! lunga, il fuso con l'ora in eccesso, l'indirizzo dopo una virgola e tre in fila, la riga di sole cifre e punti.
//! Gli attesi sono contati a mano sui byte della riga, mai copiati dall'output del codice sotto test.

use mrp_core::loghl::{highlight, line_level, Kind, Span};

fn s(start: usize, end: usize, kind: Kind) -> Span {
    Span { start, end, kind }
}

#[test]
fn a_long_fraction_stays_in_the_time_span() {
    // "09:00:53" (8 byte) + "." + nove cifre = 18 byte
    assert_eq!(
        highlight("09:00:53.123456789 end"),
        vec![s(0, 18, Kind::Time)]
    );
}

#[test]
fn a_timezone_with_a_big_hour_is_part_of_the_time() {
    // "23:59:59" (8 byte) + "+14:00" (6 byte) = 14
    assert_eq!(highlight("23:59:59+14:00 fine"), vec![s(0, 14, Kind::Time)]);
}

#[test]
fn an_address_after_a_comma_is_found() {
    // "ip: " = 4 byte, "10.20.30.40" = 11 byte: la virgola e' un "dopo" valido
    assert_eq!(
        highlight("ip: 10.20.30.40, altro"),
        vec![s(4, 15, Kind::Ip)]
    );
}

#[test]
fn three_addresses_in_a_row() {
    // "1.2.3.4" = 7 byte in 0..7, "5.6.7.8" = 7 in 8..15, "9.10.11.12" = 10 byte in 16..26
    assert_eq!(
        highlight("1.2.3.4 5.6.7.8 9.10.11.12"),
        vec![s(0, 7, Kind::Ip), s(8, 15, Kind::Ip), s(16, 26, Kind::Ip)]
    );
}

#[test]
fn a_line_of_only_digits_and_dots_is_nothing() {
    // "12.34.56.78" avrebbe il "dopo" sbagliato (".9" dietro) e dopo un punto non comincia mai un token
    assert_eq!(highlight("12.34.56.78.90.12.34"), vec![]);
}

#[test]
fn an_incomplete_timezone_stays_out_even_after_a_fraction() {
    // "03:04:05" + ".6" + "-0530" = 15 byte
    assert_eq!(highlight("03:04:05.6-0530 x"), vec![s(0, 15, Kind::Time)]);
    // il segno da solo non e' un fuso: lo span finisce con la frazione
    assert_eq!(highlight("03:04:05.6- x"), vec![s(0, 10, Kind::Time)]);
}

#[test]
fn a_date_followed_by_a_t_without_a_time_is_nothing() {
    // il `T` non porta a un orario e la data sola ha il "dopo" non valido (una lettera)
    assert_eq!(highlight("2026-10-31Treno"), vec![]);
}

#[test]
fn the_level_of_a_line_skips_the_spans_that_are_not_levels() {
    assert_eq!(
        highlight("09:00:53 WARN"),
        vec![s(0, 8, Kind::Time), s(9, 13, Kind::Warn)]
    );
    assert_eq!(line_level("09:00:53 WARN"), Some(Kind::Warn));
}
