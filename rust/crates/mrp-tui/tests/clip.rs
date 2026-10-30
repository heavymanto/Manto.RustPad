//! Test delle funzioni pure degli appunti del terminale (E17): base64 (RFC 4648) e la sequenza OSC 52. Gli attesi sono
//! i vettori di prova dell'RFC 4648 (sezione 10) e calcoli a mano.

use mrp_tui::clip::{base64, osc52, OSC52_LIMIT};

#[test]
fn base64_matches_the_rfc_4648_vectors() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foob"), "Zm9vYg==");
    assert_eq!(base64(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn base64_handles_all_byte_values() {
    // 0x00 0x01 0x02 -> "AAEC"; 0xFB 0xFF 0xBF usa i simboli '+' e '/' -> "+/+/"; 0xFF -> "/w=="
    assert_eq!(base64(&[0, 1, 2]), "AAEC");
    assert_eq!(base64(&[0xFB, 0xFF, 0xBF]), "+/+/");
    assert_eq!(base64(&[0xFF]), "/w==");
    // 300 byte: la lunghezza e' 4 * ceil(300 / 3)
    let v: Vec<u8> = (0..300u32).map(|i| (i % 256) as u8).collect();
    assert_eq!(base64(&v).len(), 400);
}

#[test]
fn osc52_wraps_the_base64_in_the_clipboard_escape_sequence() {
    assert_eq!(osc52(b"foo").as_deref(), Some("\x1b]52;c;Zm9v\x07"));
    assert_eq!(osc52(b"").as_deref(), Some("\x1b]52;c;\x07"));
}

#[test]
fn osc52_refuses_what_terminals_would_cut() {
    assert_eq!(OSC52_LIMIT, 75_000);
    assert!(osc52(&vec![b'x'; OSC52_LIMIT]).is_some());
    assert_eq!(osc52(&vec![b'x'; OSC52_LIMIT + 1]), None);
}
