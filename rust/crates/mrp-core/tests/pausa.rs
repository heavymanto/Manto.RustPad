//! L'indicizzazione si puo' mettere in pausa (l'utente salta nel file e il disco deve essere libero): in pausa non legge
//! piu' niente e non avanza; tolta la pausa riparte e arriva allo stesso indice di una costruzione senza pausa.
//! Scritti da ARCH.

use mrp_core::lines::{BuildOptions, LineIndex};
use mrp_core::newline::Newline;
use mrp_core::source::FileSource;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Un file da 70 MiB (sopra la soglia del precaricamento, 64 MiB): 700.000 righe da 100 byte.
fn file_grande() -> (std::path::PathBuf, u64) {
    let p = std::env::temp_dir().join(format!("mrp_pausa_{}.txt", std::process::id()));
    let mut w = std::io::BufWriter::new(std::fs::File::create(&p).unwrap());
    let riga = format!("{}\n", "x".repeat(99));
    for _ in 0..720_000 {
        w.write_all(riga.as_bytes()).unwrap();
    }
    w.flush().unwrap();
    (p, 720_000)
}

#[test]
fn a_paused_build_reads_nothing_and_goes_on_when_the_pause_ends() {
    let (p, righe) = file_grande();
    let src = Arc::new(FileSource::open(&p).unwrap());
    let cancel = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(AtomicU64::new(0));
    let pausa = Arc::new(AtomicBool::new(true));
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let (src, cancel, progress, pausa) =
            (src.clone(), cancel.clone(), progress.clone(), pausa.clone());
        std::thread::spawn(move || {
            let r = LineIndex::build_pausable(
                &src,
                Newline::Byte,
                &BuildOptions::default(),
                &cancel,
                &progress,
                &pausa,
            );
            let _ = tx.send(r.map(|i| i.line_count()));
        });
    }
    // in pausa fin dall'inizio: dopo mezzo secondo non e' avanzato di un byte e non e' finito
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(progress.load(Ordering::Relaxed), 0);
    assert!(rx.try_recv().is_err());
    // tolta la pausa finisce, con le righe giuste
    pausa.store(false, Ordering::Relaxed);
    let inizio = Instant::now();
    let conteggio = loop {
        match rx.try_recv() {
            Ok(r) => break r.unwrap(),
            Err(_) if inizio.elapsed() > Duration::from_secs(60) => {
                cancel.store(true, Ordering::Relaxed);
                panic!("non riparte dopo la pausa");
            }
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    assert_eq!(conteggio, righe);
    assert_eq!(progress.load(Ordering::Relaxed), src.len());
    let _ = std::fs::remove_file(p);
}

#[test]
fn cancel_wins_over_a_pause() {
    let (p, _) = file_grande();
    let src = Arc::new(FileSource::open(&p).unwrap());
    let cancel = Arc::new(AtomicBool::new(false));
    let pausa = Arc::new(AtomicBool::new(true));
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let (src, cancel, pausa) = (src.clone(), cancel.clone(), pausa.clone());
        std::thread::spawn(move || {
            let r = LineIndex::build_pausable(
                &src,
                Newline::Byte,
                &BuildOptions::default(),
                &cancel,
                &AtomicU64::new(0),
                &pausa,
            );
            let _ = tx.send(r.is_err());
        });
    }
    std::thread::sleep(Duration::from_millis(200));
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(10)),
        Ok(true),
        "annullata anche in pausa"
    );
    let _ = std::fs::remove_file(p);
}
