//! I tratti di sfondo e di testo con cui si disegna una riga della griglia (E31): funzioni pure sulle celle. Attesi
//! scritti a mano da ARCH.

use mrp_gui::paint::{colore, tratti_sfondo, tratti_testo};
use mrp_tui::cells::Cell;

fn c(simbolo: &str, fg: [u8; 3], bg: [u8; 3]) -> Cell {
    Cell {
        simbolo: simbolo.to_string(),
        fg,
        bg,
        grassetto: false,
        sottolineato: false,
    }
}

const B: [u8; 3] = [0, 0, 0];
const R: [u8; 3] = [255, 0, 0];
const W: [u8; 3] = [255, 255, 255];

#[test]
fn background_runs_group_equal_colours() {
    let riga = [
        c("a", W, B),
        c("b", W, B),
        c("c", W, R),
        c("d", W, R),
        c("e", W, B),
    ];
    assert_eq!(tratti_sfondo(&riga), vec![(0, 2, B), (2, 4, R), (4, 5, B)]);
    assert!(tratti_sfondo(&[]).is_empty());
    assert_eq!(tratti_sfondo(&[c(" ", W, R)]), vec![(0, 1, R)]);
}

#[test]
fn text_runs_join_ascii_of_the_same_colour_and_skip_spaces() {
    let riga = [
        c("a", W, B),
        c("b", W, B),
        c(" ", W, B),
        c("c", W, B),
        c("d", R, B),
    ];
    // "ab", poi lo spazio interrompe, "c" da solo (anche se dello stesso colore), "d" rosso
    assert_eq!(
        tratti_testo(&riga),
        vec![
            (0, "ab".to_string(), W, false),
            (3, "c".to_string(), W, false),
            (4, "d".to_string(), R, false),
        ]
    );
}

#[test]
fn wide_and_non_ascii_characters_stay_in_their_own_cell() {
    // 日 occupa due celle: la prima ha il simbolo, la seconda e' vuota; 'x' viene dopo
    let riga = [
        c("a", W, B),
        c("\u{65e5}", W, B),
        c("", W, B),
        c("x", W, B),
        c("\u{e9}", W, B),
        c("y", W, B),
    ];
    assert_eq!(
        tratti_testo(&riga),
        vec![
            (0, "a".to_string(), W, false),
            (1, "\u{65e5}".to_string(), W, false),
            (3, "x".to_string(), W, false),
            (4, "\u{e9}".to_string(), W, false),
            (5, "y".to_string(), W, false),
        ]
    );
}

#[test]
fn bold_breaks_a_run() {
    let mut b = c("b", W, B);
    b.grassetto = true;
    let riga = [c("a", W, B), b, c("c", W, B)];
    let t = tratti_testo(&riga);
    assert_eq!(t.len(), 3);
    assert!(t[1].3);
}

#[test]
fn colours_convert_to_egui() {
    let e = colore([1, 2, 3]);
    assert_eq!((e.r(), e.g(), e.b(), e.a()), (1, 2, 3, 255));
}
