//! Ricerca sul documento MODIFICATO, in un thread a parte. Il `Document` non si condivide, quindi si prende un
//! `Snapshot`: una copia leggera dello stato (la tabella dei pezzi e un puntatore condiviso al file mappato) che si
//! puo' mandare a un altro thread e che continua a funzionare anche se il documento cambia o viene salvato dopo.
//! La ricerca e' su byte esatti (maiuscole e minuscole contano), a blocchi con sovrapposizione, annullabile.

use crate::document::Document;
use crate::edit::PieceTable;
use crate::encoding::Encoding;
use crate::source::FileSource;
use memchr::memmem;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Byte letti per volta nella ricerca (anche granularita' dell'avanzamento), se `with_chunk`
/// non dice altro: 1 MiB.
const DEFAULT_CHUNK: usize = 1024 * 1024;

/// Perche' una ricerca non e' finita.
#[derive(Debug)]
pub enum SearchError {
    /// Annullata tramite `cancel`.
    Cancelled,
    /// Errore di lettura dal file.
    Io(io::Error),
}

/// Una fotografia del documento (contenuto com'era al momento di `Document::snapshot`).
#[derive(Clone)]
pub struct Snapshot {
    /// Il file mappato, condiviso col documento: un salvataggio sostituisce il puntatore nel
    /// documento, non questo, quindi qui si continua a leggere il contenuto di allora.
    source: Arc<FileSource>,
    /// Copia della tabella dei pezzi (col suo buffer delle aggiunte): le modifiche fatte dopo
    /// alla tabella del documento non la toccano. La memoria dipende dalle modifiche, non dal file.
    table: PieceTable,
    /// Byte letti per volta (>= 1).
    chunk: usize,
    /// Allineamento degli inizi di occorrenza validi: l'unita' dell'a-capo del documento
    /// (1 per UTF-8 e code page, 2 per UTF-16). Un'occorrenza che comincia a una posizione
    /// non allineata e' meta' di un'unita' e non conta.
    align: usize,
    /// Per le codifiche multibyte asiatiche: un'occorrenza vale solo se comincia a un confine di carattere
    /// (il secondo byte di un carattere puo' avere lo stesso valore di un carattere ASCII). Vedi `with_boundary`.
    boundary: Option<Encoding>,
}

// La fotografia deve poter arrivare al thread di ricerca, e piu' ricerche possono partire
// dalla stessa: `Send + Sync` lo impone il compilatore, qui.
const _: () = {
    const fn send_e_sync<T: Send + Sync>() {}
    send_e_sync::<Snapshot>();
};

impl Document {
    /// La fotografia dello stato attuale; `None` se il documento e' chiuso.
    pub fn snapshot(&self) -> Option<Snapshot> {
        // Il file mappato si condivide (un `Arc`), la tabella dei pezzi si copia: da qui in poi
        // il documento puo' cambiare o essere salvato senza effetti sulla fotografia.
        Some(Snapshot {
            source: Arc::clone(self.source()?),
            table: self.table().clone(),
            chunk: DEFAULT_CHUNK,
            align: self.newline().unit(),
            boundary: None,
        })
    }
}

impl Snapshot {
    /// Cambia la dimensione dei blocchi di lettura (>= 1; predefinita 1 MiB): serve ai test per attraversare i confini.
    pub fn with_chunk(self, chunk: usize) -> Snapshot {
        Snapshot {
            chunk: chunk.max(1),
            ..self
        }
    }

    /// Con una codifica multibyte asiatica (Shift_JIS, GBK, Big5, EUC-JP, EUC-KR) le occorrenze che cominciano nel mezzo
    /// di un carattere non contano: la barra rovesciata 0x5C e' anche il secondo byte di molti ideogrammi in Shift_JIS.
    /// Per le altre codifiche non cambia nulla.
    pub fn with_boundary(self, enc: Encoding) -> Snapshot {
        Snapshot {
            boundary: enc.is_multibyte().then_some(enc),
            ..self
        }
    }

    /// Vero se `pos` e' un inizio di carattere (sempre, senza `with_boundary`): si guardano i byte prima di `pos` fino
    /// a un inizio sicuro (`Encoding::char_start`) e quelli dopo, per sapere quanto e' lungo il carattere.
    fn is_boundary(&self, pos: u64) -> bool {
        let Some(enc) = self.boundary else {
            return true;
        };
        let da = pos.saturating_sub(enc.back_window() as u64);
        let fine = pos.saturating_add(4).min(self.len());
        let ctx = self.table.read_range(&self.source, da, fine - da);
        let i = (pos - da) as usize;
        i >= ctx.len() || enc.char_start(&ctx, i) == i
    }

    /// I byte `[pos, pos + len)` della fotografia (quelli che ci sono: oltre la fine si taglia).
    pub(crate) fn read_range(&self, pos: u64, len: u64) -> Vec<u8> {
        self.table.read_range(&self.source, pos, len)
    }

    /// I byte letti per volta.
    pub(crate) fn chunk_size(&self) -> usize {
        self.chunk
    }

    /// Tutti gli inizi di `needle` (byte esatti, senza sovrapposizioni, solo se allineati e a un confine di carattere
    /// della codifica), in ordine; si ferma con `Ok(Err(n))` appena sono piu' di `max`. Serve alle sostituzioni di testo
    /// semplice, che nelle codifiche diverse da UTF-8 non passano dal motore delle regole (`pattern`).
    pub fn find_all_literal(
        &self,
        needle: &[u8],
        max: usize,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<Result<Vec<u64>, usize>, SearchError> {
        let mut v: Vec<u64> = Vec::new();
        if needle.is_empty() {
            return Ok(Ok(v));
        }
        let len = self.len();
        let n = needle.len() as u64;
        let chunk = self.chunk as u64;
        let align = self.align as u64;
        let mut pos = 0u64;
        // fino a dove si e' arrivati con l'ultima occorrenza accettata: la prossima non puo' cominciare prima
        let mut libero = 0u64;
        while pos < len {
            if cancel.load(Ordering::Relaxed) {
                return Err(SearchError::Cancelled);
            }
            let next = pos.saturating_add(chunk).min(len);
            let fine = next.saturating_add(n - 1).min(len);
            let block = self.table.read_range(&self.source, pos, fine - pos);
            let mut da = 0usize;
            while let Some(i) = memmem::find(&block[da..], needle) {
                let rel = da + i;
                let inizio = pos + rel as u64;
                // un inizio da `next` in poi lo vede il blocco dopo
                if inizio >= next {
                    break;
                }
                // uno dentro l'occorrenza di prima, sfasato o a meta' di un carattere non conta: si riprova dal byte dopo
                if inizio >= libero && inizio.is_multiple_of(align) && self.is_boundary(inizio) {
                    v.push(inizio);
                    if v.len() > max {
                        return Ok(Err(v.len()));
                    }
                    libero = inizio + n;
                    da = rel + needle.len();
                } else {
                    da = rel + 1;
                }
            }
            progress.fetch_add(next - pos, Ordering::Relaxed);
            pos = next;
        }
        Ok(Ok(v))
    }

    /// Lunghezza della fotografia in byte.
    pub fn len(&self) -> u64 {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Il primo inizio di `needle` che sta in `[from, len)` con l'ago intero dentro il documento e multiplo
    /// dell'allineamento del documento (l'unita' dell'a-capo: le occorrenze sfasate di UTF-16 non contano).
    /// `None` se non c'e', se `needle` e' vuoto o piu' lungo del documento, o se `from` e' oltre la fine.
    /// `progress` cresce dei byte esaminati; `cancel` si controlla prima di ogni blocco.
    pub fn find_next(
        &self,
        needle: &[u8],
        from: u64,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<Option<u64>, SearchError> {
        if needle.is_empty() {
            return Ok(None);
        }
        // Come in `search::find_next`: il cancel viene prima dei controlli che porterebbero a `None`.
        if cancel.load(Ordering::Relaxed) {
            return Err(SearchError::Cancelled);
        }
        let len = self.len();
        let n = needle.len() as u64;
        // Somma satura: con `from` enorme non c'e' nessun inizio ammissibile, senza overflow.
        if from.saturating_add(n) > len {
            return Ok(None);
        }
        let chunk = self.chunk as u64;
        let align = self.align as u64;
        let mut pos = from;
        loop {
            // Il cancel si controlla PRIMA di elaborare il blocco.
            if cancel.load(Ordering::Relaxed) {
                return Err(SearchError::Cancelled);
            }
            // Il blocco avanza di [pos, next); la finestra letta si estende di `n - 1` oltre
            // `next`, cosi' un'occorrenza a cavallo di due blocchi non sfugge (il tratto extra
            // non conta nel progress, che misura solo l'avanzamento). Le occorrenze che
            // cominciano a una posizione non allineata si saltano e si CONTINUA nella stessa
            // finestra, riprendendo da subito dopo l'inizio trovato.
            let next = pos.saturating_add(chunk).min(len);
            let fine = next.saturating_add(n - 1).min(len);
            let block = self.table.read_range(&self.source, pos, fine - pos);
            let mut da = 0usize;
            while let Some(i) = memmem::find(&block[da..], needle) {
                let inizio = pos + (da + i) as u64;
                if inizio.is_multiple_of(align) && self.is_boundary(inizio) {
                    return Ok(Some(inizio));
                }
                da += i + 1;
            }
            progress.fetch_add(next - pos, Ordering::Relaxed);
            if next == len {
                return Ok(None);
            }
            pos = next;
        }
    }

    /// L'ultimo inizio di `needle` che sta prima di `before` (inizio < `before`, ago intero dentro il documento).
    /// `before` oltre la fine vale come la fine. Stesse regole di `find_next` per ago vuoto, allineamento, progress
    /// e cancel.
    pub fn find_prev(
        &self,
        needle: &[u8],
        before: u64,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<Option<u64>, SearchError> {
        if needle.is_empty() {
            return Ok(None);
        }
        // Come in `find_next`: il cancel viene prima dei controlli che porterebbero a `None`.
        if cancel.load(Ordering::Relaxed) {
            return Err(SearchError::Cancelled);
        }
        let len = self.len();
        let n = needle.len() as u64;
        // Con `before == 0` non c'e' nessun inizio ammissibile; con `len < n` nessuna
        // occorrenza sta nel documento (evita anche l'underflow di `len - n` sotto).
        if before == 0 || len < n {
            return Ok(None);
        }
        // Limite ESCLUSO per gli inizi ammessi: `s < before` e `s + n <= len`.
        let mut hi = before.min(len - n + 1);
        let chunk = self.chunk as u64;
        loop {
            // Il cancel si controlla PRIMA di elaborare il blocco.
            if cancel.load(Ordering::Relaxed) {
                return Err(SearchError::Cancelled);
            }
            // Blocco all'indietro [lo, hi): la finestra letta si estende di `n - 1` oltre `hi`
            // per tenere dentro anche le occorrenze che iniziano nel blocco e finiscono dopo
            // `hi`; una che iniziasse a `hi` o oltre non ci sta mai intera, quindi `rfind`
            // restituisce l'ultima occorrenza con inizio in [lo, hi). Le occorrenze non
            // allineate si saltano: la finestra si accorza alla fine dell'occorrenza trovata
            // meno un byte (cosi' restano anche quelle che la sovrappongono) e si riprova.
            let lo = hi.saturating_sub(chunk);
            let fine = hi.saturating_add(n - 1).min(len);
            let block = self.table.read_range(&self.source, lo, fine - lo);
            let mut fino = block.len();
            while let Some(i) = memmem::rfind(&block[..fino], needle) {
                let inizio = lo + i as u64;
                if inizio.is_multiple_of(self.align as u64) && self.is_boundary(inizio) {
                    return Ok(Some(inizio));
                }
                fino = i + needle.len() - 1;
            }
            progress.fetch_add(hi - lo, Ordering::Relaxed);
            if lo == 0 {
                return Ok(None);
            }
            hi = lo;
        }
    }
}
