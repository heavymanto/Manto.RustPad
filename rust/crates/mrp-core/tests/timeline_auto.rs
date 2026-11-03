//! Il passo della timeline scelto da solo (`timeline::auto_step`): il piu' lungo fra giorno, ora e dieci minuti che da' almeno dodici
//! bucket, altrimenti il minuto. Scritti da ARCH: le date sono a mano (novembre 2026), i conteggi dei bucket si fanno a mente.

use mrp_core::timeline::{auto_step, Step, BUCKET_MIN};

/// 2026-11-01 00:00:00.000 in millisecondi dal 1970 (calendar.timegm di Python).
const MEZZANOTTE: i64 = 1793491200000;
const ORA: i64 = 3_600_000;
const GIORNO: i64 = 86_400_000;

#[test]
fn a_long_log_goes_by_days_and_a_short_one_by_a_finer_step() {
    assert_eq!(BUCKET_MIN, 12);
    // due settimane (15 giorni toccati): giorni
    assert_eq!(
        auto_step(MEZZANOTTE + 5 * ORA, MEZZANOTTE + 14 * GIORNO + ORA),
        Step::Day
    );
    // dodici giorni toccati esatti: ancora giorni; undici: le ore
    assert_eq!(auto_step(MEZZANOTTE, MEZZANOTTE + 11 * GIORNO), Step::Day);
    assert_eq!(
        auto_step(MEZZANOTTE, MEZZANOTTE + 10 * GIORNO + 23 * ORA),
        Step::Hour
    );
    // tre giorni: le ore (72 bucket)
    assert_eq!(
        auto_step(MEZZANOTTE, MEZZANOTTE + 3 * GIORNO - 1),
        Step::Hour
    );
    // cinque ore: sei ore toccate non bastano, dieci minuti si' (31 bucket)
    assert_eq!(
        auto_step(MEZZANOTTE + 10 * ORA, MEZZANOTTE + 15 * ORA),
        Step::TenMinutes
    );
    // sette minuti (qui il log vero: 11:14 -> 11:21): i minuti
    let k = MEZZANOTTE + 2 * GIORNO + 11 * ORA + 14 * 60_000;
    assert_eq!(auto_step(k + 33_738, k + 7 * 60_000 + 22_236), Step::Minute);
    // un solo istante: i minuti
    assert_eq!(auto_step(k, k), Step::Minute);
}
