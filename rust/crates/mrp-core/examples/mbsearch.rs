//! Misura la ricerca con le codifiche multibyte: quanto costa scartare le occorrenze che cadono nel mezzo di un carattere.
//! Uso: cargo run --release -p mrp-core --example mbsearch -- <file> <codifica> <ago>
//! Cerca <ago> (scritto nella codifica) dall'inizio fino alla fine e stampa quante occorrenze di byte ci sono, quante
//! cominciano a un confine e quanto ci vuole con e senza il controllo dei confini. Attenzione: il conteggio riparte da
//! ogni occorrenza (come fa il TUI a ogni Invio), quindi senza confini, con molte occorrenze, e' lento per colpa del
//! ciclo e non della ricerca; con i confini le occorrenze nel mezzo di un carattere si scartano dentro una sola chiamata.

use mrp_core::encoding::Encoding;
use mrp_core::Document;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("uso: mbsearch <file> <codifica> <ago>");
        std::process::exit(2);
    }
    let e = Encoding::by_name(&args[2]).expect("codifica");
    let ago = e
        .encode_str(&args[3])
        .expect("ago scrivibile nella codifica");
    let doc = Document::open(Path::new(&args[1])).expect("apertura");
    let cancel = AtomicBool::new(false);
    for (nome, con_confini) in [("senza confini", false), ("con confini   ", true)] {
        let mut snap = doc.snapshot().expect("snapshot");
        if con_confini {
            snap = snap.with_boundary(e);
        }
        let t = Instant::now();
        let mut trovate = 0u64;
        let mut da = 0u64;
        let progress = AtomicU64::new(0);
        while let Some(p) = snap
            .find_next(&ago, da, &cancel, &progress)
            .expect("ricerca")
        {
            trovate += 1;
            da = p + 1;
        }
        println!(
            "{nome}: {trovate} occorrenze in {:.2} s",
            t.elapsed().as_secs_f64()
        );
    }
}
