//! Conversione di codifica a flusso: il contenuto attuale di un documento (modifiche non salvate comprese) si
//! rilegge a blocchi nella codifica di partenza e si riscrive in un altro file con la codifica di arrivo.
//! Come per il salvataggio, la scrittura va su un file temporaneo nella cartella della destinazione e una
//! rinomina lo mette al posto del file giusto: se qualcosa va storto (errore, annullamento, carattere che la
//! codifica di arrivo non sa scrivere) la destinazione resta com'era e non resta nessun temporaneo. E come per
//! tutto il resto del crate, il documento non viene mai letto per intero: un blocco alla volta, con una piccola
//! guardia oltre la fine del blocco per non spezzare i caratteri fra due blocchi.

use crate::document::Document;
use crate::encoding::Encoding;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// Byte di guardia letti oltre la fine di ogni blocco: un carattere dura al massimo 4 byte e `first_char` non
/// guarda mai oltre i 4, quindi il carattere che comincia poco prima della fine del blocco si decodifica sempre
/// intero con la guardia, e il blocco dopo riparte da quello successivo.
const GUARDIA: u64 = 8;

/// Blocco minimo di lettura: i valori piu' piccoli salgono a 8.
const BLOCCO_MINIMO: usize = 8;

/// Capacita' del buffer di scrittura dell'uscita.
const BUF_CAPACITY: usize = 64 * 1024;

/// Contatore dei file temporanei creati dal processo: tiene distinti i nomi, conversione dopo conversione.
static CONV_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Opzioni della conversione.
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// Byte di documento letti per blocco (i valori sotto 8 salgono a 8): in memoria non sta mai piu' di
    /// `block_bytes` + 8 byte di documento alla volta.
    pub block_bytes: usize,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        // Il blocco predefinito: 1 MiB.
        ConvertOptions {
            block_bytes: 1 << 20,
        }
    }
}

/// Il lavoro fatto da una conversione riuscita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertStats {
    /// Caratteri scritti (il BOM di destinazione non e' un carattere e non si conta).
    pub chars: u64,
    /// Byte del file scritto, BOM di destinazione compreso.
    pub bytes_out: u64,
}

/// Perche' una conversione non e' andata a buon fine. In ogni caso il file di destinazione non e' stato toccato
/// (se c'era, e' come prima) e non resta nessun file temporaneo.
#[derive(Debug)]
pub enum ConvertError {
    /// Annullato tramite `cancel`.
    Cancelled,
    /// Il primo carattere che la codifica di arrivo non sa scrivere: `offset` e' la sua posizione in byte nel
    /// documento (BOM sorgente compreso).
    Unrepresentable { offset: u64, ch: char },
    /// Errore di sistema (cartella della destinazione che non esiste, disco pieno...).
    Io(io::Error),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::Cancelled => write!(f, "conversione annullata"),
            ConvertError::Unrepresentable { offset, ch } => write!(
                f,
                "il carattere {ch} all'offset {offset} non si puo' scrivere in questa codifica"
            ),
            ConvertError::Io(e) => write!(f, "errore di I/O: {e}"),
        }
    }
}

impl std::error::Error for ConvertError {}

impl From<io::Error> for ConvertError {
    fn from(e: io::Error) -> Self {
        ConvertError::Io(e)
    }
}

/// Il BOM da scrivere in testa al file di destinazione, `None` per le codifiche che non lo hanno: le code page
/// e le multibyte non prendono mai un BOM, anche quando e' chiesto.
fn bom_di_arrivo(to: Encoding) -> Option<&'static [u8]> {
    match to {
        Encoding::Utf8 => Some(&[0xEF, 0xBB, 0xBF]),
        Encoding::Utf16Le => Some(&[0xFF, 0xFE]),
        Encoding::Utf16Be => Some(&[0xFE, 0xFF]),
        Encoding::Utf32Le => Some(&[0xFF, 0xFE, 0x00, 0x00]),
        Encoding::Utf32Be => Some(&[0x00, 0x00, 0xFE, 0xFF]),
        Encoding::Single(_) | Encoding::Multi(_) => None,
    }
}

/// Percorso del prossimo temporaneo per convertire in `target`: `<nome>.<pid>.<n>.mrp-conv`, nella stessa
/// cartella (la rinomina finale deve restare sullo stesso filesystem).
fn temp_path(target: &Path) -> PathBuf {
    let n = CONV_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = OsString::new();
    // Il ripiego serve solo a restare senza `unwrap` quando il target non ha un nome file.
    name.push(target.file_name().unwrap_or_else(|| OsStr::new("file")));
    name.push(format!(".{}.{}.mrp-conv", std::process::id(), n));
    target.with_file_name(name)
}

/// File temporaneo della conversione: finche' e' armato, chi lo abbandona (errore, annullamento, carattere non
/// rappresentabile, rinomina rifiutata) lo cancella uscendo dallo scope. Si disarma solo dopo una rinomina
/// riuscita. (Stesso trucco del `TempGuard` del salvataggio, in `document.rs`.)
struct TempGuard {
    path: PathBuf,
    armed: bool,
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        if self.armed {
            // L'errore si ignora: nel caso peggiore resta un file da cancellare a mano, la destinazione e'
            // salva.
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Converte il contenuto attuale di `doc` (modifiche non salvate comprese), letto con la codifica `from`, nel
/// file `target` scritto con la codifica `to`. Restituisce quanti caratteri e quanti byte ha scritto.
///
/// I passi, come nel salvataggio:
/// 1. la destinazione puo' essere anche il file aperto dal documento: si legge dalla mappatura e si scrive su un
///    temporaneo, e la rinomina finale lo sostituisce (dopo, il documento va riaperto da chi chiama);
/// 2. `cancel` gia' vero: `Cancelled`, senza creare nessun file (poi si controlla prima di ogni blocco);
/// 3. il BOM sorgente si salta: non e' un carattere, non si conta e non si riscrive (i suoi byte restano pero'
///    nel conto di `progress`);
/// 4. file temporaneo NELLA STESSA CARTELLA della destinazione, creato con `create_new`, di nome
///    `<nome>.<pid>.<n>.mrp-conv`; se non si arriva alla rinomina lo cancella il `TempGuard` all'uscita;
/// 5. se `bom` e' chiesto e la codifica di arrivo e' UTF-8, UTF-16 o UTF-32, il suo BOM si scrive per primo;
/// 6. un blocco alla volta: si chiedono `block_bytes` byte (almeno 8) piu' 8 byte di guardia (o quel che resta
///    del documento), si decodificano con `from.first_char` i caratteri che COMINCIANO prima della fine del
///    blocco (la guardia serve a decodificare intero l'ultimo, che puo' sporgere oltre) e si riscrivono con
///    `to.encode`; il primo carattere che la destinazione non sa scrivere ferma tutto con
///    `Unrepresentable { offset, ch }`, dove l'offset e' in byte nel documento, BOM sorgente compreso;
/// 7. `sync_all` del temporaneo, che si chiude PRIMA della rinomina; ultimo controllo di `cancel`;
/// 8. `std::fs::rename` del temporaneo sopra `target`, sostituendolo se c'e' gia'; se fallisce, il guard
///    cancella il temporaneo e la destinazione resta com'era.
///
/// `progress` cresce dei byte del documento consumati a ogni blocco (BOM sorgente compreso) e a fine lavoro
/// vale `doc.len()`. Il documento non cambia: resta aperto, con le sue modifiche, e il file di partenza e'
/// intatto.
// la firma a otto parametri e' quella imposta dal PLAN: uno in piu' del limite di clippy
#[allow(clippy::too_many_arguments)]
pub fn convert_document(
    doc: &Document,
    from: Encoding,
    to: Encoding,
    bom: bool,
    target: &Path,
    opts: &ConvertOptions,
    cancel: &AtomicBool,
    progress: &AtomicU64,
) -> Result<ConvertStats, ConvertError> {
    // 1. La destinazione puo' essere anche il file aperto dal documento ("converti questo file"): si legge dal
    //    documento (la mappatura del file com'e') e si scrive su un temporaneo, e la rinomina finale sostituisce il
    //    file senza toccare la mappatura (come fa il salvataggio).
    // 2. Annullato prima ancora di cominciare: nessun file creato. Nei blocchi il controllo si ripete prima
    //    di leggere e scrivere.
    if cancel.load(Ordering::Relaxed) {
        return Err(ConvertError::Cancelled);
    }
    let blocco = opts.block_bytes.max(BLOCCO_MINIMO);
    let len = doc.len();
    // 3. Il BOM sorgente si salta (solo all'inizio del documento): il piu' lungo e' di 4 byte.
    let head = doc.read_range(0, 4);
    let salto = from.bom_len(&head) as u64;
    let mut pos = salto;
    progress.fetch_add(salto, Ordering::Relaxed);
    // 4. Temporaneo nella stessa cartella della destinazione, dichiarato PRIMA del writer: cosi', uscendo per
    //    un errore, prima si chiude il file e poi il guard lo cancella.
    let mut guard = TempGuard {
        path: temp_path(target),
        armed: true,
    };
    let file = File::create_new(&guard.path)?;
    let mut out = BufWriter::with_capacity(BUF_CAPACITY, file);
    let mut stats = ConvertStats {
        chars: 0,
        bytes_out: 0,
    };
    // 5. Il BOM della destinazione, se chiesto e se la codifica di arrivo ne ha uno: conta nei byte del file,
    //    non nei caratteri.
    if bom {
        if let Some(b) = bom_di_arrivo(to) {
            out.write_all(b)?;
            stats.bytes_out += b.len() as u64;
        }
    }
    // 6. I blocchi. `want` e' sempre o il blocco con la sua guardia o il resto del documento: la lettura
    //    finisce o nella guardia o nella vera fine del file, cosi' un carattere troncato dai byte letti e'
    //    troncato davvero.
    while pos < len {
        if cancel.load(Ordering::Relaxed) {
            return Err(ConvertError::Cancelled);
        }
        let want = (blocco as u64).saturating_add(GUARDIA).min(len - pos);
        let data = doc.read_range(pos, want);
        // Difesa: la lettura chiedeva esattamente `want` byte dentro la lunghezza del documento, e meno byte
        // vorrebbe dire un documento incoerente con la sua tabella dei pezzi. Non puo' succedere, ma qui si
        // fallisce forte invece di scrivere un file troncato (come fa il salvataggio).
        if data.len() as u64 != want {
            return Err(ConvertError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "documento piu' corto del previsto durante la conversione",
            )));
        }
        let fine = data.len().min(blocco); // fine del blocco corrente, nei byte letti
        let mut i = 0usize; // byte consumati di questo blocco
        while i < fine {
            // `first_char` risponde `None` solo su una fetta vuota, e `i < fine <= data.len()` la esclude;
            // il ramo resta per non fidarsi del contratto altrui.
            let Some((c, n)) = from.first_char(&data[i..]) else {
                break;
            };
            let scritti = match to.encode(c) {
                Some(b) => b,
                // Il primo carattere che la destinazione non sa scrivere ferma tutto: l'offset e' la
                // posizione del carattere nel documento, BOM sorgente compreso.
                None => {
                    return Err(ConvertError::Unrepresentable {
                        offset: pos + i as u64,
                        ch: c,
                    })
                }
            };
            out.write_all(&scritti)?;
            stats.bytes_out += scritti.len() as u64;
            stats.chars += 1;
            i += n;
        }
        pos += i as u64;
        progress.fetch_add(i as u64, Ordering::Relaxed);
    }
    // 7. L'uscita e' completa: si scarica il buffer, si sincronizza il file su disco e lo si chiude PRIMA
    //    della rinomina. Ultimo controllo di `cancel`, come nel salvataggio.
    let file = out.into_inner().map_err(|e| e.into_error())?;
    file.sync_all()?;
    drop(file);
    if cancel.load(Ordering::Relaxed) {
        return Err(ConvertError::Cancelled);
    }
    // 8. La rinomina mette il temporaneo al posto della destinazione, sostituendola se c'e' gia'.
    std::fs::rename(&guard.path, target)?;
    // 9. Rinomina riuscita: il temporaneo non esiste piu', il guard si disarma.
    guard.armed = false;
    Ok(stats)
}
