//! Diagnostica: costruisce l'indice delle righe di un file e dice quanto ci ha messo, quante righe ha trovato e quante
//! volte la costruzione ha toccato la mappatura (`FileSource::map_accesses`). Il working set del processo si guarda da
//! fuori (con un guardiano che lo ferma se supera un tetto).
//!
//! Uso: cargo run --release -p mrp-core --example idxcheck -- <file>

use mrp_core::{FileSource, LineIndex};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Instant;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("uso: idxcheck <file>");
        std::process::exit(2);
    };
    let src = match FileSource::open(std::path::Path::new(&path)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("non apro {path}: {e}");
            std::process::exit(1);
        }
    };
    let t = Instant::now();
    let progress = AtomicU64::new(0);
    let r = LineIndex::build(&src, &AtomicBool::new(false), &progress);
    match r {
        Ok(idx) => println!(
            "righe={} punti={} tempo={:.1}s accessi_alla_mappatura={}",
            idx.line_count(),
            idx.checkpoint_count(),
            t.elapsed().as_secs_f64(),
            src.map_accesses()
        ),
        Err(_) => println!("annullato"),
    }
}
