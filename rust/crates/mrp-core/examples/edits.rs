//! Misura il costo di MOLTE modifiche sparse: `edits <file> <modifiche>`. Inserisce un byte in una posizione
//! pseudocasuale (sempre lontano dalle altre: il caso peggiore per la lista dei pezzi) e, a ogni scaglione,
//! stampa quanti microsecondi costa in media una modifica e quanti pezzi ci sono. Modifica solo in memoria:
//! il file non si tocca (non si salva).

use mrp_core::Document;
use std::path::Path;
use std::time::Instant;

fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("uso: edits <file> <modifiche>");
    let n: u64 = a
        .next()
        .expect("uso: edits <file> <modifiche>")
        .parse()
        .expect("numero");
    let mut doc = Document::open(Path::new(&path)).expect("apertura");
    let mut seme: u64 = 0x9E37_79B9_7F4A_7C15;
    let scaglione = (n / 10).max(1);
    let mut t = Instant::now();
    for i in 1..=n {
        // xorshift64: la posizione cade ovunque nel documento corrente
        seme ^= seme << 13;
        seme ^= seme >> 7;
        seme ^= seme << 17;
        let pos = seme % (doc.len() + 1);
        doc.insert(pos, b"x").expect("inserimento");
        if i % scaglione == 0 {
            let us = t.elapsed().as_secs_f64() * 1e6 / scaglione as f64;
            println!(
                "dopo {:>9} modifiche: {:>9.1} us per modifica (ultimo scaglione), {:>9} pezzi",
                i,
                us,
                doc.table().pieces().len()
            );
            t = Instant::now();
        }
    }
}
