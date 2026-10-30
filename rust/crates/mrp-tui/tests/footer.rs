//! La barra dei tasti in fondo allo schermo: tutte le voci (F8 compreso) devono starci a 80 colonne.

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
        " F1 Help F2 Save F3 Find F4 Edit F5 Goto F6 Theme F7 Lang F8 Col F10 Quit"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn with_room_to_spare_the_keys_keep_their_two_spaces() {
    let (mut a, p) = app("b.txt");
    a.resize(100, 10);
    let riga = render_lines(&mut a, 100, 10)[9].trim_end().to_string();
    assert_eq!(
        riga,
        " F1 Help  F2 Save  F3 Find  F4 Edit  F5 Goto  F6 Theme  F7 Lang  F8 Col  F10 Quit"
    );
    let _ = std::fs::remove_file(p);
}

#[test]
fn too_narrow_for_everything_the_line_is_cut_with_two_spaces() {
    let (mut a, p) = app("c.txt");
    a.resize(30, 10);
    let riga = render_lines(&mut a, 30, 10)[9].trim_end().to_string();
    assert_eq!(riga, " F1 Help  F2 Save  F3 Find  F4");
    let _ = std::fs::remove_file(p);
}
