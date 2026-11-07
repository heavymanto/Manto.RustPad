//! La barra dei tasti in fondo allo schermo: i tre modi (Alt+V, Alt+E, Alt+T) ci sono sempre, in testa; a 80 colonne restano anche F1, F2, F3, F5, F10 (e F9 se le etichette sono corte).

use mrp_tui::app::{App, AppConfig};
use mrp_tui::ui::render_lines;
use std::path::PathBuf;

fn app(name: &str) -> (App, PathBuf) {
    let p = std::env::temp_dir().join(format!("mrp_footer_test_{}_{}", std::process::id(), name));
    std::fs::write(&p, b"abc\n").unwrap();
    let a = App::open(&p, AppConfig::default()).unwrap();
    (a, p)
}

#[test]
fn at_80_columns_every_key_fits_with_single_spaces() {
    let (mut a, p) = app("a.txt");
    a.resize(80, 10);
    let riga = render_lines(&mut a, 80, 10)[9].trim_end().to_string();
    assert_eq!(
        riga,
        " Alt+V View Alt+E Edit Alt+T Tail F1 Help F2 Save F3 Find F5 Go F9 Enc F10 Quit"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn with_room_to_spare_the_keys_keep_their_two_spaces() {
    let (mut a, p) = app("b.txt");
    a.resize(120, 10);
    let riga = render_lines(&mut a, 120, 10)[9].trim_end().to_string();
    assert_eq!(
        riga,
        " Alt+V View  Alt+E Edit  Alt+T Tail  F1 Help  F2 Save  F3 Find  F5 Go  F6 Theme  F7 Lang  F8 Col  F9 Enc  F10 Quit"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn too_narrow_for_everything_the_line_is_cut_with_two_spaces() {
    let (mut a, p) = app("c.txt");
    a.resize(30, 10);
    let riga = render_lines(&mut a, 30, 10)[9].trim_end().to_string();
    assert_eq!(riga, " Alt+V View  Alt+E Edit  Alt+T");
    let _ = std::fs::remove_file(p);
}

#[test]
fn every_language_fits_all_keys_up_to_f10_in_80_columns() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrp-core/lang");
    let (langs, avvisi) = mrp_core::lang::registry_in(Some(&dir));
    assert!(avvisi.is_empty(), "{avvisi:?}");
    assert_eq!(langs.len(), 7);
    for l in &langs {
        let p = std::env::temp_dir().join(format!(
            "mrp_footer_lang_{}_{}.txt",
            std::process::id(),
            l.code
        ));
        std::fs::write(
            &p, b"abc
",
        )
        .unwrap();
        let cfg = AppConfig {
            lang_code: Some(l.code.clone()),
            langs: langs.clone(),
            ..AppConfig::default()
        };
        let mut a = App::open(&p, cfg).unwrap();
        a.resize(80, 10);
        let riga = render_lines(&mut a, 80, 10)[9].clone();
        for tasto in ["Alt+V", "Alt+E", "Alt+T", "F1", "F2", "F3", "F5", "F10"] {
            assert!(
                riga.contains(&format!("{tasto} ")),
                "{}: manca {tasto} in {riga:?}",
                l.code
            );
        }
        let _ = std::fs::remove_file(p);
    }
}
