//! Test del junior per la vista filtrata (fase F4): i casi limite del PLAN che il test di accettazione non copre.
//! La riga piu' lunga di un blocco (i blocchi di righe intere la prendono in una volta), il `\r` isolato in mezzo
//! a una riga (non separa nulla: e' un byte come gli altri), il file di sole righe vuote con `invert` (le righe
//! vuote non corrispondono mai), il limite esatto con l'ultima riga senza a-capo e il progresso quando e' il limite
//! a fermare il filtro. Gli attesi sono calcolati a mano mentre il test costruisce i dati.

use mrp_core::document::Document;
use mrp_core::pattern::{FilterStats, Mode, Pattern};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8]) -> TempFile {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p =
            std::env::temp_dir().join(format!("mrp_filter_jun_{}_{}.log", std::process::id(), n));
        std::fs::write(&p, data).unwrap();
        TempFile(p)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn no() -> AtomicBool {
    AtomicBool::new(false)
}

fn prog() -> AtomicU64 {
    AtomicU64::new(0)
}

fn pat(t: &str, m: Mode, case: bool) -> Pattern {
    Pattern::new(t, m, case).unwrap()
}

fn filtra(
    dati: &[u8],
    p: &Pattern,
    invert: bool,
    max: u64,
    chunk: usize,
) -> (Vec<u8>, Vec<u64>, FilterStats) {
    let t = TempFile::new(dati);
    let d = Document::open(&t.0).unwrap();
    let snap = d.snapshot().unwrap().with_chunk(chunk);
    let mut fuori = Vec::new();
    let mut offset = Vec::new();
    let st = snap
        .filter_lines(p, invert, max, &mut fuori, &mut offset, &no(), &prog())
        .unwrap();
    (fuori, offset, st)
}

#[test]
fn a_line_longer_than_the_block_is_written_whole() {
    // una riga di seimila byte con blocchi da 7: i blocchi di righe intere la leggono in una volta, quindi esce
    // intera (col suo offset) o non esce
    let mut dati = Vec::new();
    dati.extend_from_slice(b"prima\n");
    let inizio_lunga = dati.len() as u64;
    dati.extend_from_slice(&vec![b'x'; 3000]);
    dati.extend_from_slice(b"ERROR");
    dati.extend_from_slice(&vec![b'y'; 3000]);
    dati.push(b'\n');
    let fine_lunga = dati.len() as u64;
    dati.extend_from_slice(b"ultima ok\n");
    let p = pat("ERROR", Mode::Text, true);
    let (f, o, st) = filtra(&dati, &p, false, u64::MAX, 7);
    assert_eq!(f, dati[inizio_lunga as usize..fine_lunga as usize]);
    assert_eq!(o, vec![inizio_lunga]);
    assert_eq!(
        st,
        FilterStats {
            lines: 1,
            truncated: false
        }
    );
    // con invert escono le altre due
    let (f, o, st) = filtra(&dati, &p, true, u64::MAX, 7);
    assert_eq!(f, b"prima\nultima ok\n");
    assert_eq!(o, vec![0, fine_lunga]);
    assert_eq!(st.lines, 2);
}

#[test]
fn an_isolated_cr_is_a_byte_of_the_line_like_any_other() {
    // il `\r` da solo non chiude la riga: "uno\rdue" e' una riga sola e il testo cercato puo' attraversarlo
    let dati = b"uno\rdue\nERROR x\n";
    let (f, o, st) = filtra(dati, &pat("o\rd", Mode::Text, true), false, u64::MAX, 4);
    assert_eq!(f, b"uno\rdue\n");
    assert_eq!(o, vec![0]);
    assert_eq!(
        st,
        FilterStats {
            lines: 1,
            truncated: false
        }
    );
    // niente corrisponde a "zzz": con invert escono tutte le righe, il `\r` compreso
    let (f, o, st) = filtra(dati, &pat("zzz", Mode::Text, true), true, u64::MAX, 4);
    assert_eq!(f, dati[..]);
    assert_eq!(o, vec![0, 8]);
    assert_eq!(st.lines, 2);
}

#[test]
fn a_file_of_only_empty_lines_goes_all_out_with_invert() {
    let dati = b"\n\n\n\n";
    // le righe vuote non corrispondono mai: senza invert non esce niente
    let (f, o, st) = filtra(dati, &pat("a", Mode::Text, true), false, u64::MAX, 3);
    assert!(f.is_empty() && o.is_empty());
    assert_eq!(
        st,
        FilterStats {
            lines: 0,
            truncated: false
        }
    );
    // con invert escono tutte, ognuna col suo a-capo e il suo offset
    let (f, o, st) = filtra(dati, &pat("a", Mode::Text, true), true, u64::MAX, 3);
    assert_eq!(f, b"\n\n\n\n");
    assert_eq!(o, vec![0, 1, 2, 3]);
    assert_eq!(
        st,
        FilterStats {
            lines: 4,
            truncated: false
        }
    );
}

#[test]
fn the_exact_limit_with_a_last_line_without_a_newline() {
    let dati = b"ERROR a\nERROR b\nERROR c";
    let p = pat("ERROR", Mode::Text, true);
    // tre righe da scrivere, limite tre: non e' troncato e l'uscita finisce con un a-capo
    let (f, o, st) = filtra(dati, &p, false, 3, 5);
    assert_eq!(f, b"ERROR a\nERROR b\nERROR c\n");
    assert_eq!(o, vec![0, 8, 16]);
    assert_eq!(
        st,
        FilterStats {
            lines: 3,
            truncated: false
        }
    );
    // limite due: la terza riga si vede (per questo e' troncato) ma non si scrive
    let (f, o, st) = filtra(dati, &p, false, 2, 5);
    assert_eq!(f, b"ERROR a\nERROR b\n");
    assert_eq!(o, vec![0, 8]);
    assert_eq!(
        st,
        FilterStats {
            lines: 2,
            truncated: true
        }
    );
}

#[test]
fn the_progress_reaches_the_length_even_when_the_limit_stops_the_filter() {
    let mut dati = Vec::new();
    for i in 0..50 {
        dati.extend_from_slice(format!("ERROR {i}\n").as_bytes());
    }
    let t = TempFile::new(&dati);
    let d = Document::open(&t.0).unwrap();
    let snap = d.snapshot().unwrap().with_chunk(10);
    let prog = AtomicU64::new(0);
    let mut fuori = Vec::new();
    let mut offset = Vec::new();
    let st = snap
        .filter_lines(
            &pat("ERROR", Mode::Text, true),
            false,
            5,
            &mut fuori,
            &mut offset,
            &no(),
            &prog,
        )
        .unwrap();
    assert_eq!(
        st,
        FilterStats {
            lines: 5,
            truncated: true
        }
    );
    assert_eq!(offset.len(), 5);
    // fermato dal limite, ma il progresso vale comunque i byte del documento
    assert_eq!(prog.load(Ordering::Relaxed), dati.len() as u64);
}
