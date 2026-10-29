//! Navigazione per righe visuali, senza indice e senza mai scansionare piu' di `max_scan` byte per chiamata.
//!
//! Una "riga visuale" (row) e' una riga vera (termina con `\n`) oppure un pezzo di riga troppo lunga, tagliato dopo
//! `max_scan` byte. Cosi' un file con una sola riga da 1 GB non blocca l'interfaccia. `max_scan == 0` si tratta come 1
//! (si progredisce sempre). Le funzioni leggono dal documento MODIFICATO a blocchi, mai tutto.

use crate::document::Document;
use memchr::{memchr, memrchr};

/// Quanti byte al massimo si esaminano per cercare un `\n` (1 MiB).
pub const DEFAULT_MAX_SCAN: u64 = 1 << 20;

/// Dove comincia la riga dopo quella che inizia in `pos`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Inizio della riga vera successiva (il byte dopo un `\n` che non e' l'ultimo del documento).
    Line(u64),
    /// La riga e' stata tagliata: la riga visuale successiva comincia qui, a meta' di una riga vera.
    Cut(u64),
    /// Non c'e' una riga successiva (ultima riga, o `pos` oltre la fine).
    End,
}

/// Dimensione dei blocchi di lettura: 64 KiB rendono le letture rare senza copiare mai troppo alla volta.
const CHUNK: u64 = 64 * 1024;

/// `max_scan == 0` si tratta come 1: si progredisce sempre.
fn max_effettivo(max_scan: u64) -> u64 {
    max_scan.max(1)
}

/// Il primo `\n` in `[da, fine)`, leggendo a blocchi in avanti (`fine` non oltre la fine del documento).
fn primo_nl(doc: &Document, da: u64, fine: u64) -> Option<u64> {
    let mut p = da;
    while p < fine {
        let n = CHUNK.min(fine - p);
        let blocco = doc.read_range(p, n);
        if blocco.is_empty() {
            // difensivo: oltre la fine del documento (o documento chiuso) non c'e' nessun a-capo
            return None;
        }
        if let Some(i) = memchr(b'\n', &blocco) {
            return Some(p + i as u64);
        }
        p += blocco.len() as u64;
    }
    None
}

/// L'ultimo `\n` in `[da, fine)`, leggendo a blocchi all'indietro (`fine` non oltre la fine del documento).
fn ultimo_nl(doc: &Document, da: u64, fine: u64) -> Option<u64> {
    let mut fino = fine;
    while fino > da {
        let n = CHUNK.min(fino - da);
        let inizio = fino - n;
        let blocco = doc.read_range(inizio, n);
        if blocco.is_empty() {
            // difensivo, come sopra
            return None;
        }
        if let Some(i) = memrchr(b'\n', &blocco) {
            return Some(inizio + i as u64);
        }
        fino = inizio;
    }
    None
}

/// Dall'inizio di una riga `pos`: esamina `[pos, pos + max_scan)` (limitato alla fine del documento). Trovato un `\n`
/// all'indice `i`: `Line(i + 1)` se `i + 1 < len`, altrimenti `End`. Non trovato: `Cut(pos + max_scan)` se
/// `pos + max_scan < len`, altrimenti `End`. `pos >= len`: `End`.
pub fn next_row(doc: &Document, pos: u64, max_scan: u64) -> Next {
    let len = doc.len();
    if pos >= len {
        return Next::End;
    }
    let max = max_effettivo(max_scan);
    let fine = len.min(pos.saturating_add(max));
    match primo_nl(doc, pos, fine) {
        // trovato: la prossima riga vera comincia dopo l'a-capo, a meno che non sia l'ultimo byte
        Some(i) => {
            if i + 1 < len {
                Next::Line(i + 1)
            } else {
                Next::End
            }
        }
        // non trovato entro max_scan byte: la riga si taglia, se dopo resta ancora documento
        None => {
            let taglio = pos.saturating_add(max);
            if taglio < len {
                Next::Cut(taglio)
            } else {
                Next::End
            }
        }
    }
}

/// Fine della riga che inizia in `pos`: l'indice del primo `\n` in `[pos, pos + max_scan)` se c'e', altrimenti
/// `min(len, pos + max_scan)`. Mai meno di `min(pos, len)`. Il `\n` non fa parte della riga.
pub fn row_end(doc: &Document, pos: u64, max_scan: u64) -> u64 {
    let len = doc.len();
    let max = max_effettivo(max_scan);
    let fine = len.min(pos.saturating_add(max));
    let inizio = pos.min(len);
    match primo_nl(doc, inizio, fine) {
        Some(i) => i,
        None => fine.max(inizio),
    }
}

/// Inizio della riga che contiene `pos` (`pos` limitato a `len`; il `\n` appartiene alla riga che chiude). Cerca
/// l'ultimo `\n` in `[pos - max_scan, pos)`: trovato all'indice `i` -> `(i + 1, true)`; non trovato e `pos <= max_scan`
/// -> `(0, true)`; non trovato e `pos > max_scan` -> `(pos - max_scan, false)` (taglio: il `false` dice che non e'
/// un inizio di riga vera).
pub fn row_start(doc: &Document, pos: u64, max_scan: u64) -> (u64, bool) {
    let len = doc.len();
    let pos = pos.min(len);
    let max = max_effettivo(max_scan);
    let da = pos.saturating_sub(max);
    match ultimo_nl(doc, da, pos) {
        // l'a-capo chiude la riga che lo contiene: quella di pos comincia subito dopo
        Some(i) => (i + 1, true),
        None => {
            if pos <= max {
                (0, true)
            } else {
                (pos - max, false)
            }
        }
    }
}

/// Inizio della riga visuale prima di quella che contiene `pos`: `None` se `pos == 0` (dopo il limite a `len`),
/// altrimenti `row_start(doc, pos - 1, max_scan).0`.
pub fn prev_row_start(doc: &Document, pos: u64, max_scan: u64) -> Option<u64> {
    let pos = pos.min(doc.len());
    if pos == 0 {
        None
    } else {
        Some(row_start(doc, pos - 1, max_scan).0)
    }
}
