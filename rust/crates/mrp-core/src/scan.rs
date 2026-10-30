//! Navigazione per righe visuali, senza indice e senza mai scansionare piu' di `max_scan` byte per chiamata.
//!
//! Una "riga visuale" (row) e' una riga vera (termina con l'a-capo del documento) oppure un pezzo di riga troppo
//! lunga, tagliato dopo `max_scan` byte. Cosi' un file con una sola riga da 1 GB non blocca l'interfaccia.
//! `max_scan == 0` si tratta come un'unita' dell'a-capo (si progredisce sempre). Le funzioni leggono dal documento
//! MODIFICATO a blocchi, mai tutto. L'a-capo e' quello del documento (`Document::newline`): con UTF-16 e' un'unita'
//! di 2 byte ALLINEATA, quindi le letture partono a posizioni multiple dell'unita' e un `pos` non allineato si
//! arrotonda per difetto.

use crate::document::Document;

/// Quanti byte al massimo si esaminano per cercare un a-capo (1 MiB).
pub const DEFAULT_MAX_SCAN: u64 = 1 << 20;

/// Dove comincia la riga dopo quella che inizia in `pos`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Inizio della riga vera successiva (l'unita' dopo un a-capo che non e' l'ultima del documento).
    Line(u64),
    /// La riga e' stata tagliata: la riga visuale successiva comincia qui, a meta' di una riga vera.
    Cut(u64),
    /// Non c'e' una riga successiva (ultima riga, o `pos` oltre la fine).
    End,
}

/// Dimensione dei blocchi di lettura: 64 KiB rendono le letture rare senza copiare mai troppo alla volta
/// (e sono pari, quindi con l'unita' a 16 bit i blocchi partono sempre allineati).
const CHUNK: u64 = 64 * 1024;

/// `max_scan == 0` si tratta come un'unita': si progredisce sempre. Il risultato e' `max_scan` arrotondato per
/// difetto a un multiplo di `unit`, cosi' i tagli delle righe lunghe cadono su confini di unita'.
fn max_effettivo(max_scan: u64, unit: u64) -> u64 {
    (max_scan - max_scan % unit).max(unit)
}

/// Il primo a-capo in `[da, fine)`, leggendo a blocchi in avanti (`fine` non oltre la fine del documento). I blocchi
/// sono lunghi `CHUNK` (pari) o meno e partono da `da`, che i chiamanti passano allineato all'unita' dell'a-capo:
/// cosi' le posizioni trovate hanno l'allineamento del documento.
fn primo_nl(doc: &Document, da: u64, fine: u64) -> Option<u64> {
    let nl = doc.newline();
    let mut p = da;
    while p < fine {
        let n = CHUNK.min(fine - p);
        let blocco = doc.read_range(p, n);
        if blocco.is_empty() {
            // difensivo: oltre la fine del documento (o documento chiuso) non c'e' nessun a-capo
            return None;
        }
        if let Some(i) = nl.find(&blocco) {
            return Some(p + i as u64);
        }
        p += blocco.len() as u64;
    }
    None
}

/// L'ultimo a-capo in `[da, fine)`, leggendo a blocchi all'indietro (`fine` non oltre la fine del documento).
/// `fine` si arrotonda prima per difetto a un multiplo dell'unita' (un byte finale spaiato non e' un a-capo)
/// e i blocchi partono allineati; il blocco all'indietro non scende mai sotto `da`.
fn ultimo_nl(doc: &Document, da: u64, fine: u64) -> Option<u64> {
    let nl = doc.newline();
    let unit = nl.unit() as u64;
    let mut fino = fine - fine % unit;
    while fino > da {
        let n = CHUNK.min(fino - da);
        let inizio = fino - n;
        let blocco = doc.read_range(inizio, n);
        if blocco.is_empty() {
            // difensivo, come sopra
            return None;
        }
        if let Some(i) = nl.rfind(&blocco) {
            return Some(inizio + i as u64);
        }
        fino = inizio;
    }
    None
}

/// Dall'inizio di una riga `pos`: esamina `[pos, pos + max_scan)` (limitato alla fine del documento). Trovato un
/// a-capo all'indice `i`: `Line(i + unit)` se `i + unit < len`, altrimenti `End`. Non trovato: `Cut(pos + max_scan)`
/// se `pos + max_scan < len`, altrimenti `End`. `pos >= len`: `End`.
pub fn next_row(doc: &Document, pos: u64, max_scan: u64) -> Next {
    let len = doc.len();
    if pos >= len {
        return Next::End;
    }
    let unit = doc.newline().unit() as u64;
    let max = max_effettivo(max_scan, unit);
    let fine = len.min(pos.saturating_add(max));
    match primo_nl(doc, pos, fine) {
        // trovato: la prossima riga vera comincia dopo l'a-capo, a meno che non sia l'ultima unita'
        Some(i) => {
            if i + unit < len {
                Next::Line(i + unit)
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

/// Fine della riga che inizia in `pos`: l'indice del primo a-capo in `[pos, pos + max_scan)` se c'e', altrimenti
/// `min(len, pos + max_scan)`. Mai meno di `min(pos, len)`. L'a-capo non fa parte della riga. `pos` si limita a
/// `len` e si arrotonda per difetto a un multiplo dell'unita' dell'a-capo.
pub fn row_end(doc: &Document, pos: u64, max_scan: u64) -> u64 {
    let len = doc.len();
    let unit = doc.newline().unit() as u64;
    let max = max_effettivo(max_scan, unit);
    let pos = pos.min(len);
    let pos = pos - pos % unit;
    let fine = len.min(pos.saturating_add(max));
    match primo_nl(doc, pos, fine) {
        Some(i) => i,
        None => fine.max(pos),
    }
}

/// Inizio della riga che contiene `pos` (`pos` limitato a `len` e arrotondato per difetto a un multiplo dell'unita'
/// dell'a-capo; l'a-capo appartiene alla riga che chiude). Cerca l'ultimo a-capo in `[pos - max_scan, pos)`: trovato
/// all'indice `i` -> `(i + unit, true)`; non trovato e `pos <= max_scan` -> `(0, true)`; non trovato e
/// `pos > max_scan` -> `(pos - max_scan, false)` (taglio: il `false` dice che non e' un inizio di riga vera).
pub fn row_start(doc: &Document, pos: u64, max_scan: u64) -> (u64, bool) {
    let len = doc.len();
    let unit = doc.newline().unit() as u64;
    let pos = pos.min(len);
    let pos = pos - pos % unit;
    let max = max_effettivo(max_scan, unit);
    let da = pos.saturating_sub(max);
    match ultimo_nl(doc, da, pos) {
        // l'a-capo chiude la riga che lo contiene: quella di pos comincia subito dopo
        Some(i) => (i + unit, true),
        None => {
            if pos <= max {
                (0, true)
            } else {
                (pos - max, false)
            }
        }
    }
}

/// Inizio della riga visuale prima di quella che contiene `pos`: `None` se `pos == 0` (dopo il limite a `len` e
/// l'arrotondamento per difetto a un multiplo dell'unita' dell'a-capo), altrimenti
/// `row_start(doc, pos - unit, max_scan).0`.
pub fn prev_row_start(doc: &Document, pos: u64, max_scan: u64) -> Option<u64> {
    let unit = doc.newline().unit() as u64;
    let pos = pos.min(doc.len());
    let pos = pos - pos % unit;
    if pos == 0 {
        None
    } else {
        Some(row_start(doc, pos - unit, max_scan).0)
    }
}
