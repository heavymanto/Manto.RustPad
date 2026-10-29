//! Ricerca di una sequenza di byte, avanti e indietro, a blocchi e annullabile.

use crate::source::FileSource;
use memchr::memmem;
use std::sync::atomic::{AtomicBool, Ordering};

/// Esito di una ricerca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchResult {
    /// Posizione in byte dell'inizio della corrispondenza.
    Found(u64),
    NotFound,
    Cancelled,
}

/// Parametri della ricerca.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Byte esaminati per volta (>= 1). Predefinito: 8 MiB. Una corrispondenza puo' attraversare due blocchi.
    pub chunk_bytes: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        SearchOptions {
            chunk_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Converte una posizione in byte in un indice valido per la fetta `bytes` (stesso modello
/// del privato `slice_index` di `lines.rs`, non riutilizzabile da qui). Le posizioni della
/// ricerca sono sempre dentro la fetta; il limite difensivo evita un panic con indici
/// incoerenti. `usize::try_from` e non `as`, che su 32 bit troncherebbe.
fn slice_index(bytes: &[u8], pos: u64) -> usize {
    usize::try_from(pos).unwrap_or(bytes.len()).min(bytes.len())
}

/// Prima corrispondenza con inizio `s >= from` (le corrispondenze possono sovrapporsi).
/// - `pattern` vuoto: `NotFound`.
/// - Se `cancel` e' vero (controllato prima di ogni blocco, anche il primo): `Cancelled`.
/// - Una corrispondenza deve stare tutta dentro il file.
pub fn find_next(
    src: &FileSource,
    pattern: &[u8],
    from: u64,
    opts: &SearchOptions,
    cancel: &AtomicBool,
) -> SearchResult {
    if pattern.is_empty() {
        return SearchResult::NotFound;
    }
    // Il cancel viene prima del controllo di lunghezza: per un file vuoto (o con `from`
    // oltre la fine) `from + n > len` e' sempre vero, ma un annullamento vince lo stesso.
    if cancel.load(Ordering::Relaxed) {
        return SearchResult::Cancelled;
    }
    let bytes = src.bytes();
    let len = bytes.len() as u64;
    let n = pattern.len() as u64;
    // Somma satura: con `from` enorme non c'e' nessun inizio ammissibile, senza overflow.
    if from.saturating_add(n) > len {
        return SearchResult::NotFound;
    }
    let chunk = opts.chunk_bytes.max(1) as u64;
    let mut pos = from;
    loop {
        // Il cancel si controlla PRIMA di elaborare il blocco.
        if cancel.load(Ordering::Relaxed) {
            return SearchResult::Cancelled;
        }
        // Il tratto extra `n - 1` oltre il blocco tiene dentro la finestra anche una
        // corrispondenza a cavallo di due blocchi: la prima trovata e' la piu' vicina.
        let end = pos.saturating_add(chunk).saturating_add(n - 1).min(len);
        let a = slice_index(bytes, pos);
        let b = slice_index(bytes, end);
        if let Some(i) = memmem::find(&bytes[a..b], pattern) {
            return SearchResult::Found(pos + i as u64);
        }
        if end == len {
            return SearchResult::NotFound;
        }
        pos += chunk;
    }
}

/// Ultima corrispondenza con inizio `s < before` (stesse regole di `find_next`).
pub fn find_prev(
    src: &FileSource,
    pattern: &[u8],
    before: u64,
    opts: &SearchOptions,
    cancel: &AtomicBool,
) -> SearchResult {
    if pattern.is_empty() {
        return SearchResult::NotFound;
    }
    // Come in `find_next`: il cancel viene prima dei controlli che porterebbero a `NotFound`.
    if cancel.load(Ordering::Relaxed) {
        return SearchResult::Cancelled;
    }
    let bytes = src.bytes();
    let len = bytes.len() as u64;
    let n = pattern.len() as u64;
    // Con `before == 0` non c'e' nessun inizio ammissibile; con `len < n` nessuna
    // corrispondenza sta nel file (evita anche l'underflow di `len - n` sotto).
    if before == 0 || len < n {
        return SearchResult::NotFound;
    }
    // Limite ESCLUSO per gli inizi ammessi: `s < before` e `s + n <= len`.
    let mut hi = before.min(len - n + 1);
    let chunk = opts.chunk_bytes.max(1) as u64;
    loop {
        // Il cancel si controlla PRIMA di elaborare il blocco.
        if cancel.load(Ordering::Relaxed) {
            return SearchResult::Cancelled;
        }
        // Blocco all'indietro [lo, hi): la finestra estesa di `n - 1` tiene dentro anche le
        // corrispondenze che attraversano il bordo sinistro del blocco.
        let lo = hi.saturating_sub(chunk);
        let fine = hi.saturating_add(n - 1).min(len);
        let a = slice_index(bytes, lo);
        let b = slice_index(bytes, fine);
        if let Some(i) = memmem::rfind(&bytes[a..b], pattern) {
            return SearchResult::Found(lo + i as u64);
        }
        if lo == 0 {
            return SearchResult::NotFound;
        }
        hi = lo;
    }
}
