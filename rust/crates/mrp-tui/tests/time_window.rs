//! La finestra "Tempo" dell'`App` (solo con i controlli nativi, `chrome = false`): l'estremo finale del filtro e' compreso (copre
//! tutta l'unita' che si scrive), il filtro si accende e si spegne senza perdere le date, OK e' tutto-o-niente, il calendario scrive
//! nel formato del file. Scritti da ARCH: gli offset delle righe si calcolano a mano sul testo di partenza.

use mrp_tui::app::{App, AppConfig, DialogKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Cento righe, una al minuto dalle 02:00:00 del 2026-11-01; gli offset di ogni riga.
fn app() -> (App, Vec<u64>, PathBuf) {
    let mut dati = String::new();
    let mut offset = Vec::new();
    for i in 0..100u32 {
        offset.push(dati.len() as u64);
        dati.push_str(&format!(
            "2026-11-01 {:02}:{:02}:00 [INFO ] riga {i}\n",
            2 + i / 60,
            i % 60
        ));
    }
    offset.push(dati.len() as u64);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_twin_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(100, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, offset, p)
}

#[test]
fn the_end_of_the_filter_is_included_and_covers_the_unit_that_was_written() {
    let (mut a, offset, p) = app();
    // "02:20" copre tutto il minuto delle 02:20
    a.apply_time_range("02:10", "02:20").unwrap();
    assert_eq!(a.time_window(), Some((offset[10], offset[21])));
    // lo stesso minuto da una parte e dall'altra e' una riga sola
    a.apply_time_range("02:30", "02:30").unwrap();
    assert_eq!(a.time_window(), Some((offset[30], offset[31])));
    // la sola data copre tutto il giorno: le cento righe
    a.apply_time_range("", "2026-11-01").unwrap();
    assert_eq!(a.time_window(), Some((0, offset[100])));
    // con i secondi l'unita' e' il secondo
    a.apply_time_range("2026-11-01 02:10:00", "2026-11-01 02:12:00")
        .unwrap();
    assert_eq!(a.time_window(), Some((offset[10], offset[13])));
    // l'inizio dopo la fine dell'unita' dell'estremo finale e' un errore
    assert!(a.apply_time_range("02:30", "02:29").is_err());
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_filter_switches_on_and_off_without_losing_the_dates() {
    let (mut a, offset, p) = app();
    // senza date ricordate il primo giro apre la finestra
    a.toggle_time_filter();
    assert_eq!(a.dialog_kind(), Some(DialogKind::Time));
    a.dialog_cancel();
    // scritte le date e dato OK, il filtro e' acceso
    a.open_time_dialog();
    a.dialog_set_field(2, "02:10");
    a.dialog_set_field(3, "02:19");
    a.dialog_submit();
    assert_eq!(a.time_window(), Some((offset[10], offset[20])));
    assert!(a.time_filter_on());
    // spento: il file e' intero, le date restano
    a.toggle_time_filter();
    assert!(!a.time_filter_on());
    assert_eq!(a.time_window(), None);
    a.open_time_dialog();
    assert!(!a.time_check());
    assert_eq!(a.time_field_text(2), "02:10");
    assert_eq!(a.time_field_text(3), "02:19");
    a.dialog_cancel();
    // acceso di nuovo con le stesse date, senza aprire niente
    a.toggle_time_filter();
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.time_window(), Some((offset[10], offset[20])));
    let _ = std::fs::remove_file(p);
}

#[test]
fn ok_is_all_or_nothing_a_bad_filter_leaves_the_format_as_it_was() {
    let (mut a, _, p) = app();
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss")
    );
    a.open_time_dialog();
    // un formato valido (la sola data) e un filtro che non si capisce
    a.dialog_set_field(0, "yyyy-MM-dd");
    a.dialog_set_field(2, "foo");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), Some(DialogKind::Time), "resta aperta");
    assert!(a.time_error_text().contains("foo"));
    // il formato e' quello di prima e non e' diventato quello standard
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("yyyy-MM-dd HH:mm:ss")
    );
    assert_eq!(a.time_default(), None);
    // corretto il filtro, passano tutti e due
    a.dialog_set_field(2, "2026-11-01");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.time_mask_pattern().as_deref(), Some("yyyy-MM-dd"));
    assert_eq!(a.time_default(), Some(("yyyy-MM-dd".to_string(), 0)));
    let _ = std::fs::remove_file(p);
}

#[test]
fn with_the_box_off_ok_shows_the_whole_file_and_with_no_limits_nothing_is_hidden() {
    let (mut a, _, p) = app();
    a.apply_time_range("02:10", "02:19").unwrap();
    a.open_time_dialog();
    assert!(a.time_check());
    // casella spenta: OK toglie il filtro, le date restano
    a.dialog_time_set_check(false);
    a.dialog_submit();
    assert_eq!(a.time_window(), None);
    // casella accesa ma tutte e due le date vuote: niente da nascondere, nessuna finestra
    a.open_time_dialog();
    a.dialog_time_set_check(true);
    a.dialog_time_clear_field(2);
    a.dialog_time_clear_field(3);
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None);
    assert_eq!(a.time_window(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_calendar_opens_on_the_right_month_and_writes_the_format_of_the_file() {
    let (mut a, _, p) = app();
    a.open_time_dialog();
    // senza data scritta: il mese del primo orario del file (Dal) o dell'ultimo (Al)
    let da = a.time_calendar(2).unwrap();
    assert_eq!((da.month, da.selected), ((2026, 11), None));
    // il file copre il solo 1 novembre (dalle 02:00 alle 03:39), scrive i secondi (unita' 1000) e il campo vuoto parte da mezzanotte /
    // fine giornata
    assert_eq!(
        (da.first, da.last),
        (Some((2026, 11, 1)), Some((2026, 11, 1)))
    );
    assert_eq!((da.unit_ms, da.time_ms), (1000, 0));
    let al = a.time_calendar(3).unwrap();
    assert_eq!(
        (al.month, al.selected, al.time_ms),
        ((2026, 11), None, 86_399_999)
    );
    assert_eq!(a.time_calendar(1), None);
    // un giorno scelto: Dal da mezzanotte e Al a fine giornata, scritti come le righe; ma il giorno della prima (ultima) riga parte dall'ora
    // della prima (ultima) riga, cosi' su un log di un giorno solo un clic non lascia il filtro largo come il file
    a.dialog_time_pick(2, 2026, 10, 30);
    a.dialog_time_pick(3, 2026, 11, 2);
    assert_eq!(a.time_field_text(2), "2026-10-30 00:00:00");
    assert_eq!(a.time_field_text(3), "2026-11-02 23:59:59");
    a.dialog_time_pick(2, 2026, 11, 1);
    assert_eq!(
        a.time_field_text(2),
        "2026-11-01 00:00:00",
        "tiene l'ora gia' scritta"
    );
    a.dialog_time_clear_field(2);
    a.dialog_time_clear_field(3);
    a.dialog_time_pick(2, 2026, 11, 1);
    a.dialog_time_pick(3, 2026, 11, 1);
    assert_eq!(a.time_field_text(2), "2026-11-01 02:00:00");
    assert_eq!(a.time_field_text(3), "2026-11-01 03:39:00");
    assert!(a.time_check());
    // con una data scritta il calendario la mostra e un nuovo giorno tiene l'ora gia' scritta
    let info = a.time_calendar(2).unwrap();
    assert_eq!(
        (info.month, info.selected),
        ((2026, 11), Some((2026, 11, 1)))
    );
    a.dialog_set_field(2, "2026-10-15 08:30:00");
    a.dialog_time_pick(2, 2026, 10, 20);
    assert_eq!(a.time_field_text(2), "2026-10-20 08:30:00");
    assert_eq!(a.time_calendar(2).unwrap().month, (2026, 10));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_mask_with_only_the_hour_has_no_calendar() {
    let (mut a, _, p) = app();
    a.set_time_mask("HH:mm:ss", 11).unwrap();
    a.open_time_dialog();
    assert_eq!(a.time_calendar(2), None);
    // e il pick non scrive niente
    a.dialog_time_pick(2, 2026, 11, 1);
    assert_eq!(a.time_field_text(2), "");
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_time_of_day_is_written_into_the_field_keeping_the_date() {
    let (mut a, offset, p) = app();
    a.open_time_dialog();
    // un campo vuoto: l'ora si scrive sul giorno della prima riga (Dal) o dell'ultima (Al)
    a.dialog_time_set_time(2, 2 * 3_600_000 + 10 * 60_000);
    assert_eq!(a.time_field_text(2), "2026-11-01 02:10:00");
    a.dialog_time_set_time(3, 2 * 3_600_000 + 19 * 60_000 + 7 * 1000);
    assert_eq!(a.time_field_text(3), "2026-11-01 02:19:07");
    assert!(a.time_check());
    // con la data gia' scritta l'ora cambia e la data resta; fuori misura si porta nel giorno
    a.dialog_set_field(2, "2026-10-15 08:30:00");
    a.dialog_time_set_time(2, 9 * 3_600_000);
    assert_eq!(a.time_field_text(2), "2026-10-15 09:00:00");
    a.dialog_time_set_time(2, 99 * 3_600_000);
    assert_eq!(a.time_field_text(2), "2026-10-15 23:59:59");
    // e con OK il filtro e' quello scritto: dalle 02:10 alle 02:19 (compresa)
    a.dialog_set_field(2, "2026-11-01 02:10:00");
    a.dialog_set_field(3, "2026-11-01 02:19:59");
    a.dialog_submit();
    assert_eq!(a.time_window(), Some((offset[10], offset[20])));
    let _ = std::fs::remove_file(p);
}

#[test]
fn a_range_outside_the_file_says_what_the_file_covers() {
    let (mut a, offset, p) = app();
    a.apply_time_range("02:10", "02:19").unwrap();
    // un giorno che il file non ha: il messaggio dice da quando a quando va il file; la finestra di prima resta
    let e = a.apply_time_range("2026-11-03", "").unwrap_err();
    assert_eq!(
        e,
        "No lines from 2026-11-03 to \u{2026}: this file goes from 2026-11-01 02:00:00 to 2026-11-01 03:39:00"
    );
    assert_eq!(a.time_window(), Some((offset[10], offset[20])));
    // la fine prima della prima riga: lo stesso
    let e = a.apply_time_range("", "2026-10-30 23:59:59").unwrap_err();
    assert!(
        e.starts_with("No lines from \u{2026} to 2026-10-30 23:59:59: this file goes from"),
        "{e}"
    );
    let _ = std::fs::remove_file(p);
}
