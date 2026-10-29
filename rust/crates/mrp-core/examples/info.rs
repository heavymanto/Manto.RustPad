//! Esempio: informazioni su un file e sul suo indice delle righe.
//!
//! Uso: `cargo run --release -p mrp-core --example info -- <file>`
//!
//! Stampa percorso e dimensione; costruisce l'indice delle righe mentre un thread stampa
//! su stderr la percentuale di avanzamento letta da `progress`; poi numero di righe, punti
//! memorizzati, tempo e velocita'; infine posizione e primi 40 byte delle righe 0, meta'
//! e ultima.

use mrp_core::{FileSource, LineIndex};
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Tick del thread di avanzamento: stampa subito la percentuale e poi una volta ogni 10
/// tick (500 ms), ma controlla `stop` a ogni tick per terminare appena finisce la
/// costruzione.
const TICK: Duration = Duration::from_millis(50);

fn main() -> ExitCode {
    // Se manca l'argomento stampa l'uso ed esce con codice 2.
    let Some(percorso) = std::env::args().nth(1) else {
        eprintln!("uso: cargo run --release -p mrp-core --example info -- <file>");
        return ExitCode::from(2);
    };
    let src = match FileSource::open(Path::new(&percorso)) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("errore aprendo {percorso}: {e}");
            return ExitCode::from(1);
        }
    };
    let n_byte = src.len();
    println!("percorso: {percorso}");
    println!("byte: {n_byte}");

    // Costruzione dell'indice: `LineIndex::build` incrementa `progress`, il thread legge
    // quel contatore e stampa la percentuale; `stop` chiude il thread a lavoro finito.
    let cancel = AtomicBool::new(false);
    let progress = AtomicU64::new(0);
    let stop = AtomicBool::new(false);
    let inizio = Instant::now();
    let (costruito, tempo) = thread::scope(|scope| {
        scope.spawn(|| {
            let mut tick: u32 = 0;
            while !stop.load(Ordering::Relaxed) {
                if tick.is_multiple_of(10) && n_byte > 0 {
                    let fatto = progress.load(Ordering::Relaxed);
                    let pct = fatto as f64 * 100.0 / n_byte as f64;
                    eprintln!("progresso: {pct:.0}%");
                }
                thread::sleep(TICK);
                tick += 1;
            }
        });
        let r = LineIndex::build(&src, &cancel, &progress);
        // Il tempo si misura a costruzione finita, prima di aspettare il thread.
        let t = inizio.elapsed();
        stop.store(true, Ordering::Relaxed);
        (r, t)
    });
    let idx = match costruito {
        Ok(idx) => idx,
        Err(_) => {
            eprintln!("errore: costruzione dell'indice annullata");
            return ExitCode::from(1);
        }
    };

    let n_righe = idx.line_count();
    let n_punti = idx.checkpoint_count();
    println!("righe: {n_righe}");
    println!("punti: {n_punti}");
    let secondi = tempo.as_secs_f64();
    println!("tempo: {secondi:.3} s");
    if secondi > 0.0 {
        let velocita = n_byte as f64 / (1024.0 * 1024.0) / secondi;
        println!("MiB/s: {velocita:.1}");
    } else {
        // Indicizzato in meno di un tick di clock: velocita' non misurabile.
        println!("MiB/s: n/d");
    }

    // Righe 0, meta' (line_count / 2) e ultima: si saltano le inesistenti (file vuoto) e
    // le ripetizioni (file con una o due righe). Posizione in byte e primi 40 byte.
    let mut stampata: Option<u64> = None;
    for riga in [0, n_righe / 2, n_righe.saturating_sub(1)] {
        if riga >= n_righe || stampata == Some(riga) {
            continue;
        }
        if let Some(pos) = idx.offset_of_line(&src, riga) {
            let testo = String::from_utf8_lossy(src.read_range(pos, 40));
            println!("riga {riga}: posizione {pos}: \"{testo}\"");
            stampata = Some(riga);
        }
    }
    ExitCode::SUCCESS
}
