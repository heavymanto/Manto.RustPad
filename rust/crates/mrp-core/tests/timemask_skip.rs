//! L'occorrenza dell'orario nella riga (`TimeMask::with_skip`): quando la data che interessa non e' la prima della riga (un log con
//! l'ora della scrittura all'inizio e la data dei dati dentro: `... XC4 funding @ 2022-01-01 00:00 ...`). Scritti da ARCH: le
//! chiavi sono calendar.timegm di Python, le posizioni sono i byte contati a mano.

use mrp_core::timemask::TimeMask;

const RIGA: &str =
    "2026-11-03 11:19:23.350 +01:00 [INF] XC4 funding @ 2022-01-01 00:00 \u{2014} rate=0,0100%";
/// 2022-01-01 00:00:00 e 2026-11-03 11:19:00 in millisecondi dal 1970.
const K_2022: i64 = 1640995200000;
const K_2026: i64 = 1793704740000;

fn m(p: &str, entro: usize, salta: usize) -> TimeMask {
    TimeMask::parse(p, entro).unwrap().with_skip(salta)
}

#[test]
fn the_first_occurrence_is_the_default_and_the_second_is_the_embedded_date() {
    // di fabbrica la prima: l'ora della scrittura all'inizio della riga
    let prima = m("yyyy-MM-dd HH:mm", 120, 0);
    assert_eq!(prima.skip(), 0);
    assert_eq!(prima.find(RIGA.as_bytes()), Some((0, 16, K_2026)));
    // la seconda: la data dei dati, che comincia al byte 51 e finisce al 67
    let seconda = m("yyyy-MM-dd HH:mm", 120, 1);
    assert_eq!(seconda.skip(), 1);
    assert_eq!(seconda.find(RIGA.as_bytes()), Some((51, 67, K_2022)));
    assert_eq!(seconda.read(RIGA.as_bytes()), Some(K_2022));
    // la terza non c'e'
    assert_eq!(m("yyyy-MM-dd HH:mm", 120, 2).find(RIGA.as_bytes()), None);
}

#[test]
fn a_line_with_fewer_dates_has_no_time_for_a_later_occurrence() {
    let sola = "2026-11-03 11:14:33.738 +01:00 [INF] BALANCE BTC: 0";
    // 11:14 e' cinque minuti prima di 11:19
    assert_eq!(
        m("yyyy-MM-dd HH:mm", 120, 0).read(sola.as_bytes()),
        Some(K_2026 - 5 * 60_000)
    );
    assert_eq!(m("yyyy-MM-dd HH:mm", 120, 1).read(sola.as_bytes()), None);
}

#[test]
fn occurrences_count_left_to_right_without_overlapping_and_within_the_window() {
    let tre = "a 2026-11-03 11:19 b 2022-01-01 00:00 c 2019-09-10 08:00 d";
    // posizioni 2, 21 e 40; chiavi di Python
    let k_2019 = 1568102400000;
    assert_eq!(
        m("yyyy-MM-dd HH:mm", 120, 0)
            .find(tre.as_bytes())
            .map(|x| x.0),
        Some(2)
    );
    assert_eq!(
        m("yyyy-MM-dd HH:mm", 120, 1).find(tre.as_bytes()),
        Some((21, 37, K_2022))
    );
    assert_eq!(
        m("yyyy-MM-dd HH:mm", 120, 2).find(tre.as_bytes()),
        Some((40, 56, k_2019))
    );
    // la finestra limita dove puo' cominciare un'occorrenza: la terza comincia in 40
    assert_eq!(m("yyyy-MM-dd HH:mm", 39, 2).find(tre.as_bytes()), None);
    assert_eq!(
        m("yyyy-MM-dd HH:mm", 40, 2)
            .find(tre.as_bytes())
            .map(|x| x.0),
        Some(40)
    );
    // senza sovrapporsi: "11:19:23" ha un solo "HH:mm" (dopo i minuti vengono i secondi, non un altro orario)
    let ora = "11:19:23 e basta";
    assert_eq!(
        m("HH:mm", 20, 0).find(ora.as_bytes()).map(|x| (x.0, x.1)),
        Some((0, 5))
    );
    assert_eq!(m("HH:mm", 20, 1).find(ora.as_bytes()), None);
}

#[test]
fn skip_zero_reads_exactly_like_before_and_epoch_works_too() {
    for p in ["yyyy-MM-dd HH:mm:ss.SSS", "yyyy-MM-dd", "HH:mm:ss"] {
        let a = TimeMask::parse(p, 40).unwrap();
        let b = a.clone().with_skip(0);
        assert_eq!(a, b);
        assert_eq!(a.read(RIGA.as_bytes()), b.read(RIGA.as_bytes()));
    }
    // l'epoch: due numeri nella riga, la seconda occorrenza e' il secondo
    let e = TimeMask::parse("epoch_s", 60).unwrap().with_skip(1);
    assert_eq!(
        e.find(b"1793498400 poi 1793498460 fine"),
        Some((15, 25, 1793498460000))
    );
    assert_eq!(e.skip(), 1);
}
