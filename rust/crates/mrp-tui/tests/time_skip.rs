//! La data dentro la riga (F1c): il suggerimento "in queste righe c'e' anche un'altra data", la scelta di quale data della riga vale
//! (`dialog_time_set_skip`) e l'anteprima che mostra le righe che il formato legge. Scritti da ARCH: trenta righe
//! `2026-11-03 11:19:SS.350 +01:00 [INF] XC4 funding @ DATA rate=1`, con DATA ogni otto ore dal 2019-09-10 08:00 (a mano), piu' righe di
//! tabella che ripetono sempre la stessa data e righe senza date.

use mrp_tui::app::{App, AppConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Il testo e gli offset delle righe con il funding (le 30 righe numerate 0..29 dentro `offset[i]`, le altre non contano).
fn log(con_funding: bool, con_tabella: bool) -> (String, Vec<u64>) {
    let mut dati = String::new();
    let mut offset = Vec::new();
    for i in 0..30u32 {
        // una riga senza date e una di tabella con due date (la seconda sempre uguale), prima di ogni riga di funding
        if con_tabella {
            dati.push_str("| tabella | senza data |\n");
            dati.push_str("2018-12-31 00:00 # 2019-01-01 00:00 | open date |\n");
        }
        offset.push(dati.len() as u64);
        let ore = 8 + 8 * i;
        let (giorno, ora) = (10 + ore / 24, ore % 24);
        if con_funding {
            dati.push_str(&format!(
                "2026-11-03 11:19:{i:02}.350 +01:00 [INF] XC4 funding @ 2019-09-{giorno:02} {ora:02}:00 rate=1\n"
            ));
        } else {
            dati.push_str(&format!(
                "2026-11-03 11:19:{i:02}.350 +01:00 [INF] BALANCE USDT: {i}\n"
            ));
        }
    }
    offset.push(dati.len() as u64);
    (dati, offset)
}

fn app(con_funding: bool, con_tabella: bool) -> (App, Vec<u64>, PathBuf) {
    let (dati, offset) = log(con_funding, con_tabella);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("mrp_tskip_{}_{}.log", std::process::id(), n));
    std::fs::write(&p, dati).unwrap();
    let cfg = AppConfig {
        chrome: false,
        ..AppConfig::default()
    };
    let mut a = App::open(&p, cfg).unwrap();
    a.resize(120, 24);
    assert!(a.wait_idle(Duration::from_secs(20)));
    (a, offset, p)
}

#[test]
fn a_second_date_that_changes_and_goes_forward_is_suggested_with_the_text_before_it() {
    let (mut a, _, p) = app(true, true);
    a.open_time_dialog();
    // la tabella ripete sempre 2019-01-01 00:00 (non si suggerisce); il funding cambia ogni riga e va avanti
    let s = a.time_suggestion().expect("un suggerimento").clone();
    assert_eq!(s.pattern, "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(s.skip, 0);
    assert_eq!(s.example, "2019-09-10 08:00");
    assert_eq!(s.lines, 30);
    let _ = std::fs::remove_file(p);
}

#[test]
fn no_suggestion_without_a_second_date_that_varies() {
    // solo l'ora di scrittura all'inizio e la tabella con la data sempre uguale
    let (mut a, _, p) = app(false, true);
    a.open_time_dialog();
    assert_eq!(a.time_suggestion(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn using_the_suggestion_then_ok_filters_on_the_date_inside_the_line() {
    let (mut a, offset, p) = app(true, true);
    // il formato riconosciuto e' l'ora di scrittura: il 2019 non c'e'
    assert!(a.apply_time_range("2019-09-11", "2019-09-11").is_err());
    a.open_time_dialog();
    a.dialog_time_use_suggestion();
    assert_eq!(a.time_field_text(0), "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(a.time_field_text(1), "200");
    a.dialog_set_field(2, "2019-09-11");
    a.dialog_set_field(3, "2019-09-11");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None);
    // l'11 settembre ha tre righe di funding (00:00, 08:00, 16:00): le righe 2, 3 e 4
    assert_eq!(a.time_window(), Some((offset[2], offset[5])));
    assert_eq!(
        a.time_mask_pattern().as_deref(),
        Some("'@ 'yyyy-MM-dd HH:mm")
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_date_number_in_the_line_reads_the_second_date_without_writing_a_literal() {
    let (mut a, offset, p) = app(true, false);
    a.open_time_dialog();
    assert_eq!(a.time_mask_skip_draft(), 0);
    a.dialog_set_field(0, "yyyy-MM-dd HH:mm");
    // la seconda data sta piu' avanti: "entro quanti caratteri" sale da solo
    a.dialog_time_set_skip(1);
    assert_eq!(a.time_mask_skip_draft(), 1);
    assert_eq!(a.time_field_text(1), "200");
    a.dialog_set_field(2, "2019-09-11");
    a.dialog_set_field(3, "2019-09-11");
    a.dialog_submit();
    assert_eq!(a.dialog_kind(), None, "{}", a.time_error_text());
    assert_eq!(a.time_mask_skip(), 1);
    // le righe della tabella (una sola data) e quelle senza date non hanno la seconda: il filtro cade sulle righe di funding
    assert_eq!(a.time_window(), Some((offset[2], offset[5])));
    // e il barra di stato lo dice
    assert_eq!(
        a.status().time_format.as_deref(),
        Some("yyyy-MM-dd HH:mm #2")
    );
    // una scelta per questo file: non diventa il formato standard
    assert_eq!(a.time_default(), None);
    // riconoscila di nuovo: torna la prima data
    a.open_time_dialog();
    a.dialog_time_auto();
    assert_eq!(a.time_mask_skip_draft(), 0);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_preview_shows_the_lines_the_format_reads_around_the_date() {
    let (mut a, _, p) = app(true, true);
    a.open_time_dialog();
    // il formato riconosciuto legge le righe di funding come tutte le altre con l'ora di scrittura: sono le prime del file con orario
    a.dialog_set_field(0, "'@ 'yyyy-MM-dd HH:mm");
    a.dialog_set_field(1, "200");
    let anteprima = a.time_mask_preview();
    assert_eq!(anteprima.len(), 3);
    // le prime tre righe che LEGGONO il formato (le prime del file, tabella e righe senza "@" no), con l'orario che ne esce
    assert_eq!(anteprima[0].1, "2019-09-10 08:00:00");
    assert_eq!(anteprima[1].1, "2019-09-10 16:00:00");
    assert_eq!(anteprima[2].1, "2019-09-11 00:00:00");
    // la riga si mostra attorno alla data: comincia con i puntini e contiene la data
    assert!(
        anteprima[0].0.starts_with('\u{2026}') && anteprima[0].0.contains("2019-09-10 08:00"),
        "{:?}",
        anteprima[0]
    );
    // un pattern che non legge niente: le prime tre righe, "(nessun orario)"
    a.dialog_set_field(0, "'ZZ 'yyyy-MM-dd HH:mm");
    let niente = a.time_mask_preview();
    assert_eq!(niente.len(), 3);
    assert!(
        niente.iter().all(|(_, letto)| letto == "(no time)"),
        "{niente:?}"
    );
    assert!(niente[0].0.starts_with("| tabella"));
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_list_has_the_time_in_use_then_the_dates_inside_the_lines_then_the_rules_that_read_something()
{
    use mrp_tui::app::SourceKind;
    let (mut a, offset, p) = app(true, false);
    // una regola che il file legge e una che non legge niente
    a.set_time_rules(vec![
        ("'funding @ 'yyyy-MM-dd".to_string(), 200, 0),
        ("'ZZZ 'yyyy-MM-dd".to_string(), 200, 0),
    ]);
    a.open_time_dialog();
    let fonti = a.time_sources().to_vec();
    // il tempo in uso, la data dopo "@ " e la regola che legge (non quella che non legge)
    assert_eq!(fonti.len(), 3, "{fonti:?}");
    assert_eq!(fonti[0].kind, SourceKind::Start);
    assert_eq!(fonti[0].pattern, "yyyy-MM-dd HH:mm:ss.SSS");
    assert_eq!(fonti[0].lines, 30);
    assert_eq!(fonti[1].kind, SourceKind::After("@ ".to_string()));
    assert_eq!(fonti[1].pattern, "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(
        (fonti[1].lines, fonti[1].example.as_str()),
        (30, "@ 2019-09-10 08:00")
    );
    assert_eq!(fonti[2].kind, SourceKind::Rule);
    assert_eq!(fonti[2].pattern, "'funding @ 'yyyy-MM-dd");
    assert_eq!(fonti[2].lines, 30);
    // quello scritto adesso e' il primo; sceglierne un altro cambia i campi; un formato a mano non e' nell'elenco
    assert_eq!(a.time_source_selected(), Some(0));
    a.dialog_time_pick_source(1);
    assert_eq!(a.time_source_selected(), Some(1));
    assert_eq!(a.time_field_text(0), "'@ 'yyyy-MM-dd HH:mm");
    assert_eq!(a.time_field_text(1), "200");
    a.dialog_set_field(2, "2019-09-11");
    a.dialog_set_field(3, "2019-09-11");
    a.dialog_submit();
    assert_eq!(a.time_window(), Some((offset[2], offset[5])));
    // un tempo dell'elenco non e' una regola nuova: le regole sono quelle di prima
    assert_eq!(a.time_rules().len(), 2);
    a.open_time_dialog();
    a.dialog_set_field(0, "yyyy-MM-dd");
    assert_eq!(a.time_source_selected(), None);
    let _ = std::fs::remove_file(p);
}

#[test]
fn the_list_keeps_its_order_whatever_time_is_in_use() {
    let (mut a, offset, p) = app(true, false);
    a.open_time_dialog();
    let prima: Vec<(String, usize)> = a
        .time_sources()
        .iter()
        .map(|s| (s.pattern.clone(), s.skip))
        .collect();
    assert_eq!(prima.len(), 2);
    // la riga 1 e' la data dopo "@ ": sceglierla e dare OK
    a.dialog_time_pick_source(1);
    a.dialog_set_field(2, "2019-09-11");
    a.dialog_set_field(3, "2019-09-11");
    a.dialog_submit();
    assert_eq!(a.time_window(), Some((offset[2], offset[5])));
    // riaperta, l'elenco e' lo stesso nello stesso ordine e la scelta e' ancora la riga 1 (non e' passata in cima)
    a.open_time_dialog();
    let dopo: Vec<(String, usize)> = a
        .time_sources()
        .iter()
        .map(|s| (s.pattern.clone(), s.skip))
        .collect();
    assert_eq!(dopo, prima);
    assert_eq!(a.time_source_selected(), Some(1));
    let _ = std::fs::remove_file(p);
}
