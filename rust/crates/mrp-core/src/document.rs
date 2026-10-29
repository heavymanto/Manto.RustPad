//! Documento modificabile: un file (mappato, mai copiato) + una `PieceTable` con le modifiche.
//! Il salvataggio scrive un file temporaneo accanto all'originale, lo sincronizza su disco e lo
//! rinomina sopra l'originale: se qualcosa va storto (errore, annullamento, file cambiato da
//! altri) l'originale resta com'e'.

use crate::edit::{Buf, EditError, Piece, PieceTable};
use crate::lines::{BuildOptions, Cancelled, LineIndex};
use crate::source::FileSource;
use memchr::memchr_iter;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::SystemTime;

/// Byte copiati per volta durante il salvataggio (anche granularita' di annullamento e di avanzamento).
const SAVE_CHUNK: usize = 4 * 1024 * 1024;

/// Perche' un salvataggio non e' andato a buon fine. In ogni caso l'originale non e' stato toccato
/// (tranne il caso `Io` dopo la rinomina, vedi `Document::save`) e non resta nessun file temporaneo.
#[derive(Debug)]
pub enum SaveError {
    /// Annullato tramite `cancel`.
    Cancelled,
    /// Il file su disco non e' piu' quello aperto (lunghezza o data di modifica diverse, o sparito):
    /// non si sovrascrive il lavoro di qualcun altro. Il documento resta com'e', con le sue modifiche.
    SourceChanged,
    /// Il documento e' chiuso: un salvataggio precedente ha rinominato il file ma non e' riuscito a riaprirlo.
    Closed,
    /// Errore di sistema (cartella non scrivibile, disco pieno, rinomina rifiutata...).
    Io(io::Error),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Cancelled => write!(f, "salvataggio annullato"),
            SaveError::SourceChanged => write!(f, "il file su disco e' cambiato dopo l'apertura"),
            SaveError::Closed => write!(f, "documento chiuso"),
            SaveError::Io(e) => write!(f, "errore di I/O: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<io::Error> for SaveError {
    fn from(e: io::Error) -> Self {
        SaveError::Io(e)
    }
}

/// Com'era il file quando l'abbiamo aperto (o salvato l'ultima volta): serve a riconoscere modifiche esterne.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

/// Fotografia di un file tramite i suoi metadati: lunghezza e data di ultima modifica (se leggibile).
fn stamp_of(md: &std::fs::Metadata) -> Stamp {
    Stamp {
        len: md.len(),
        modified: md.modified().ok(),
    }
}

/// Contatore dei file temporanei creati dal processo: tiene distinti i nomi, salvataggio dopo salvataggio.
static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Percorso del prossimo temporaneo per salvare `doc`: `.<nome file>.mrp-tmp-<pid>-<n>`, nella
/// stessa cartella (la rinomina finale deve restare sullo stesso filesystem).
fn temp_path(doc: &Path) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = OsString::from(".");
    // Il percorso di un documento e' canonico, quindi il nome file c'e' sempre: il ripiego serve
    // solo a restare senza `unwrap`.
    name.push(doc.file_name().unwrap_or_else(|| OsStr::new("file")));
    name.push(format!(".mrp-tmp-{}-{}", std::process::id(), n));
    doc.with_file_name(name)
}

/// File temporaneo del salvataggio: finche' e' armato, chi lo abbandona (errore, annullamento,
/// rinomina rifiutata) lo cancella uscendo dallo scope. Si disarma solo dopo una rinomina riuscita.
struct TempGuard {
    path: PathBuf,
    armed: bool,
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        if self.armed {
            // L'errore si ignora: nel caso peggiore resta un file da cancellare a mano,
            // l'originale e' salvo.
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Tabella derivata dall'indice del file originale e dai pezzi, ricostruita al primo uso dopo
/// ogni cambiamento della tabella dei pezzi: per ogni pezzo, dove inizia nel documento e
/// quanti `\n` del documento lo precedono. Entrambi i vettori sono lunghi `pezzi + 1`.
struct Prefix {
    /// `doc_start[i]` = posizione nel documento dell'inizio del pezzo `i`; l'ultimo elemento
    /// e' la fine dell'ultimo pezzo, cioe' `len()`.
    doc_start: Vec<u64>,
    /// `nl_before[i]` = numero di `\n` del documento prima del pezzo `i`; l'ultimo elemento
    /// e' il totale dei `\n` del documento.
    nl_before: Vec<u64>,
}

/// Quanti `\n` stanno nei primi `within` byte del pezzo `p` (`within <= p.len`):
/// - pezzo `Add`: `memchr_iter` sulla fetta del buffer delle aggiunte;
/// - pezzo `Original`: differenza di `newlines_before` agli estremi del tratto del file
///   (il costo dipende dalle righe fra due punti dell'indice, non dalla dimensione del file).
///
/// `cache` ricorda solo i conteggi dei pezzi INTERI (`within == p.len`): le chiavi restano
/// vere finche' l'indice e' valido, perche' il file originale non cambia e il buffer delle
/// aggiunte cresce solo in coda (i byte di un pezzo non vengono mai riscritti). I conteggi
/// parziali non si mettono in cache: la chiave non li distingue.
fn newlines_in_piece(
    idx: &LineIndex,
    src: &FileSource,
    table: &PieceTable,
    cache: &mut HashMap<Piece, u64>,
    p: Piece,
    within: u64,
) -> u64 {
    let intero = within == p.len;
    if intero {
        if let Some(&n) = cache.get(&p) {
            return n;
        }
    }
    let n = match p.buf {
        Buf::Add => match usize::try_from(within) {
            Ok(len) => memchr_iter(b'\n', table.add_bytes(p.start, len)).count() as u64,
            // Difensivo: il buffer delle aggiunte e' un `Vec<u8>`, quindi un pezzo `Add`
            // piu' lungo di `usize::MAX` non puo' esistere; si risponde 0 senza panic.
            Err(_) => 0,
        },
        Buf::Original => match (
            idx.newlines_before(src, p.start + within),
            idx.newlines_before(src, p.start),
        ) {
            // Il pezzo sta dentro il file e il conteggio cresce con l'offset: la differenza
            // e' esatta e non va in underflow. Il ramo `_` e' difensivo per un indice
            // incoerente con questo `src` (non puo' accadere con le invarianti vere).
            (Some(dopo), Some(prima)) => dopo - prima,
            _ => 0,
        },
    };
    if intero {
        cache.insert(p, n);
    }
    n
}

/// File + modifiche.
pub struct Document {
    path: PathBuf,
    /// `None` solo se un salvataggio e' riuscito a rinominare ma non a riaprire (documento chiuso, lungo 0).
    source: Option<FileSource>,
    table: PieceTable,
    stamp: Stamp,
    /// Vero se c'e' almeno una modifica non salvata.
    dirty: bool,
    /// Indice righe del FILE ORIGINALE (non del documento modificato): vedi `build_lines`.
    lines: Option<LineIndex>,
    /// Cache dei conteggi: quanti `\n` contiene ciascun pezzo. Le chiavi restano vere finche'
    /// `lines` e' valido (file originale e buffer delle aggiunte non cambiano sotto i pezzi).
    nl_of_piece: HashMap<Piece, u64>,
    /// Tabella derivata dai pezzi (posizioni nel documento e `\n` cumulati), ricostruita al
    /// primo uso dopo l'ultima modifica: vedi `ensure_prefix`.
    prefix: Option<Prefix>,
}

impl Document {
    /// Apre `path` (percorso reso canonico: un collegamento simbolico si risolve, il salvataggio sostituisce il file vero).
    /// Errori come `FileSource::open`. Un file vuoto e' valido.
    pub fn open(path: &Path) -> io::Result<Document> {
        // Percorso canonico: i collegamenti simbolici si risolvono e il salvataggio sostituisce
        // il file vero, non il collegamento.
        let canon = path.canonicalize()?;
        let source = FileSource::open(&canon)?;
        let stamp = stamp_of(&std::fs::metadata(&canon)?);
        let table = PieceTable::new(source.len());
        Ok(Document {
            path: canon,
            source: Some(source),
            table,
            stamp,
            dirty: false,
            lines: None,
            nl_of_piece: HashMap::new(),
            prefix: None,
        })
    }

    /// Percorso canonico del file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lunghezza attuale del documento (con le modifiche).
    pub fn len(&self) -> u64 {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Vero se ci sono modifiche non salvate.
    pub fn is_modified(&self) -> bool {
        self.dirty
    }

    /// La tabella dei pezzi (sola lettura), per chi deve conoscere la struttura.
    pub fn table(&self) -> &PieceTable {
        &self.table
    }

    /// Come `PieceTable::insert`. Un inserimento di `data` vuoto non segna il documento come modificato.
    pub fn insert(&mut self, pos: u64, data: &[u8]) -> Result<(), EditError> {
        self.table.insert(pos, data)?;
        // Un inserimento vuoto non ha effetto: il documento non risulta modificato.
        if !data.is_empty() {
            self.dirty = true;
            // Le posizioni dei pezzi nel documento sono cambiate: la tabella derivata si
            // ricostruisce al prossimo uso (la cache dei conteggi resta: i byte no).
            self.prefix = None;
        }
        Ok(())
    }

    /// Come `PieceTable::delete`. Una cancellazione di `len == 0` non segna il documento come modificato.
    pub fn delete(&mut self, pos: u64, len: u64) -> Result<(), EditError> {
        self.table.delete(pos, len)?;
        // Una cancellazione di lunghezza zero non ha effetto: il documento non risulta modificato.
        if len > 0 {
            self.dirty = true;
            // Come `insert`: la tabella derivata si ricostruisce al prossimo uso.
            self.prefix = None;
        }
        Ok(())
    }

    /// Come `PieceTable::read_range` (si adatta ai limiti, mai errore). Documento chiuso: vuoto.
    pub fn read_range(&self, pos: u64, len: u64) -> Vec<u8> {
        match &self.source {
            // Documento chiuso: non resta niente da leggere.
            None => Vec::new(),
            Some(source) => self.table.read_range(source, pos, len),
        }
    }

    /// Costruisce l'indice righe del file originale (parametri predefiniti), per `line_count`,
    /// `offset_of_line` e `line_at_offset`. Annullabile; `progress` come in `LineIndex::build`.
    /// Se annullato, il documento resta senza indice (`has_lines()` falso). Documento chiuso: non fa nulla, `Ok`.
    pub fn build_lines(
        &mut self,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<(), Cancelled> {
        self.build_lines_with(&BuildOptions::default(), cancel, progress)
    }

    /// Come `build_lines` con parametri espliciti (serve ai test per avere molti punti su file piccoli).
    pub fn build_lines_with(
        &mut self,
        opts: &BuildOptions,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<(), Cancelled> {
        // Documento chiuso: niente sorgente, niente indice da costruire.
        let source = match &self.source {
            None => return Ok(()),
            Some(source) => source,
        };
        // Se la costruzione e' annullata il vecchio indice (se c'era, ancora valido) resta al
        // suo posto: lo si sostituisce, con cache e tabella derivata azzerate, solo quando la
        // nuova costruzione e' riuscita.
        let idx = LineIndex::build_with(source, opts, cancel, progress)?;
        self.lines = Some(idx);
        self.nl_of_piece.clear();
        self.prefix = None;
        Ok(())
    }

    /// Vero se l'indice righe e' costruito e valido. Le modifiche lo lasciano valido; un salvataggio riuscito
    /// lo toglie (il file e' cambiato): va ricostruito con `build_lines`.
    pub fn has_lines(&self) -> bool {
        self.lines.is_some()
    }

    /// Numero di righe DEL DOCUMENTO modificato (stessa convenzione di `lines.rs`: `\n` piu' 1 se il documento
    /// e' non vuoto e non finisce con `\n`). `None` se l'indice righe non e' costruito.
    pub fn line_count(&mut self) -> Option<u64> {
        self.ensure_prefix();
        let prefix = self.prefix.as_ref()?;
        // Totale dei `\n` del documento: l'ultimo elemento di `nl_before` c'e' sempre.
        let nl_totali = *prefix.nl_before.last()?;
        if !self.is_empty() {
            // Il documento non finisce con `\n`: l'ultima riga non e' terminata e va contata.
            let ultimo = self.read_range(self.len() - 1, 1);
            if !ultimo.is_empty() && ultimo[0] != b'\n' {
                return Some(nl_totali + 1);
            }
        }
        Some(nl_totali)
    }

    /// Posizione in byte, nel documento modificato, dell'inizio della riga `line` (da 0).
    /// `None` se manca l'indice o `line >= line_count()`.
    pub fn offset_of_line(&mut self, line: u64) -> Option<u64> {
        if line >= self.line_count()? {
            return None;
        }
        if line == 0 {
            return Some(0);
        }
        // `line_count` ha gia' ricostruito la tabella derivata: il `?` e' solo difensivo.
        let prefix = self.prefix.as_ref()?;
        // Pezzo che contiene il `line`-esimo `\n` del documento: l'ultimo pezzo con meno di
        // `line` a-capo prima di se'. `nl_before[0] == 0 < line`, quindi l'indice e' un pezzo vero.
        let i = prefix.nl_before.partition_point(|&n| n < line) - 1;
        let p = self.table.pieces()[i];
        // Il `\n` cercato e' il j-esimo del pezzo, con j contato da 1.
        let j = line - prefix.nl_before[i];
        let idx = self.lines.as_ref()?;
        let src = self.source.as_ref()?;
        let in_pezzo = match p.buf {
            Buf::Add => {
                // Posizione del j-esimo `\n` del pezzo, piu' uno: la riga inizia subito dopo.
                let len = usize::try_from(p.len).ok()?;
                let n = usize::try_from(j - 1).ok()?;
                let rel = memchr_iter(b'\n', self.table.add_bytes(p.start, len)).nth(n)?;
                rel as u64 + 1
            }
            Buf::Original => {
                // Nel file originale il `\n` cercato e' il g-esimo: l'inizio della riga sta
                // subito dopo, oppure a fine file se quel `\n` e' l'ultimo byte.
                let g = idx.newlines_before(src, p.start)? + j;
                let nel_file = if g < idx.line_count() {
                    idx.offset_of_line(src, g)?
                } else {
                    src.len()
                };
                nel_file.checked_sub(p.start)?
            }
        };
        Some(prefix.doc_start[i] + in_pezzo)
    }

    /// Riga del documento modificato che contiene il byte in `pos` (il `\n` finale appartiene alla riga che chiude).
    /// `None` se manca l'indice o `pos >= len()`.
    pub fn line_at_offset(&mut self, pos: u64) -> Option<u64> {
        self.ensure_prefix();
        let prefix = self.prefix.as_ref()?;
        if pos >= self.len() {
            return None;
        }
        // Pezzo che contiene `pos`: l'ultimo che inizia a posizione <= `pos` (`doc_start[0] == 0
        // <= pos`, quindi l'indice e' sempre un pezzo vero).
        let i = prefix.doc_start.partition_point(|&d| d <= pos) - 1;
        let p = self.table.pieces()[i];
        let idx = self.lines.as_ref()?;
        let src = self.source.as_ref()?;
        // `\n` del pezzo prima di `pos` (tratto parziale: niente cache), piu' quelli dei
        // pezzi che precedono.
        let nl = newlines_in_piece(
            idx,
            src,
            &self.table,
            &mut self.nl_of_piece,
            p,
            pos - prefix.doc_start[i],
        );
        Some(prefix.nl_before[i] + nl)
    }

    /// Se l'indice e' valido ma la tabella derivata manca (prima interrogazione dopo
    /// l'apertura, dopo una modifica o dopo la costruzione dell'indice), la ricostruisce con
    /// un giro sui pezzi. Il costo dipende dal numero di pezzi, non dalla dimensione del file.
    fn ensure_prefix(&mut self) {
        if self.prefix.is_some() {
            return;
        }
        let Some(idx) = &self.lines else {
            return;
        };
        let Some(src) = &self.source else {
            return;
        };
        // La cache cresce a ogni pezzo nuovo e le voci dei pezzi cancellati restano: oltre
        // il doppio dei pezzi attuali (piu' un margine) si svuota prima di ricostruire.
        if self.nl_of_piece.len() > 2 * self.table.pieces().len() + 1024 {
            self.nl_of_piece.clear();
        }
        let mut prefix = Prefix {
            doc_start: Vec::with_capacity(self.table.pieces().len() + 1),
            nl_before: Vec::with_capacity(self.table.pieces().len() + 1),
        };
        let mut pos = 0u64; // posizione nel documento dell'inizio del pezzo corrente
        let mut nl = 0u64; // `\n` del documento prima del pezzo corrente
        for p in self.table.pieces() {
            prefix.doc_start.push(pos);
            prefix.nl_before.push(nl);
            nl += newlines_in_piece(idx, src, &self.table, &mut self.nl_of_piece, *p, p.len);
            pos += p.len;
        }
        // Un elemento in piu' dei pezzi: la fine del documento e il totale dei `\n`.
        prefix.doc_start.push(pos);
        prefix.nl_before.push(nl);
        self.prefix = Some(prefix);
    }

    /// Salva su disco il documento com'e' ora. Se non ci sono modifiche non fa nulla (`Ok`, `progress` invariato).
    ///
    /// Ordine, da rispettare:
    /// 1. documento chiuso -> `Closed`; nessuna modifica -> `Ok(())`; `cancel` vero -> `Cancelled`;
    /// 2. il file su disco deve avere ancora lunghezza e data di `stamp`, altrimenti `SourceChanged`
    ///    (file sparito = `SourceChanged`; ogni altro errore di sistema = `Io`);
    /// 3. file temporaneo NELLA STESSA CARTELLA, creato con `create_new`, di nome
    ///    `.<nome>.mrp-tmp-<pid>-<n>`; se non si arriva alla rinomina viene cancellato;
    /// 4. i pezzi si scrivono in ordine, a blocchi di `SAVE_CHUNK`: `Original` con `FileSource::read_at`
    ///    (letture esplicite, non page fault) e un buffer di `SAVE_CHUNK` byte, `Add` dal buffer delle aggiunte;
    ///    prima di ogni blocco si guarda `cancel`; dopo ogni blocco `progress` cresce dei byte scritti
    ///    (alla fine `progress` vale la lunghezza del nuovo file);
    /// 5. `sync_all` del temporaneo, copia dei permessi dell'originale, ultimo controllo di `cancel`;
    /// 6. `std::fs::rename` del temporaneo sopra `path`, con la mappatura ancora attiva (misurato su Windows 11:
    ///    rinominare sopra un file mappato funziona, troncarlo no); se fallisce si cancella il temporaneo e si
    ///    restituisce `Io`: il documento resta com'era, modificato, e l'originale intatto;
    /// 7. a rinomina riuscita: su Unix `sync_all` della cartella (se fallisce, si ignora); si apre `path` con
    ///    `FileSource::open` (la vecchia mappatura si rilascia sostituendola), la tabella torna a un solo pezzo
    ///    `Original` lungo quanto il nuovo file, `stamp` si rilegge, `dirty = false`.
    ///    Se l'apertura fallisce: dati al sicuro su disco, ma il documento diventa chiuso (`source = None`,
    ///    tabella lunga 0) e si restituisce `Io`.
    ///
    /// Non si conserva il proprietario del file (solo i permessi).
    pub fn save(&mut self, cancel: &AtomicBool, progress: &AtomicU64) -> Result<(), SaveError> {
        // 1. Chiuso, senza modifiche, gia' annullato: in quest'ordine, senza toccare nulla.
        let source = match &self.source {
            None => return Err(SaveError::Closed),
            Some(source) => source,
        };
        if !self.dirty {
            return Ok(());
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(SaveError::Cancelled);
        }

        // 2. Il file su disco deve essere ancora quello aperto: lo si fotografa di nuovo e lo si
        //    confronta con lo `stamp` dell'apertura. I permessi servono al passo 5.
        let md = match std::fs::metadata(&self.path) {
            Ok(md) => md,
            // Sparito: e' stato rimosso (comunque cambiato) da altri.
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(SaveError::SourceChanged),
            Err(e) => return Err(SaveError::Io(e)),
        };
        let perms = md.permissions();
        if stamp_of(&md) != self.stamp {
            return Err(SaveError::SourceChanged);
        }

        // 3. Temporaneo nella stessa cartella: se non si arriva alla rinomina lo cancella il
        //    `TempGuard` all'uscita da qui.
        let tmp = temp_path(&self.path);
        let mut file = File::create_new(&tmp)?;
        let mut guard = TempGuard {
            path: tmp,
            armed: true,
        };

        // 4. I pezzi in ordine, a blocchi di `SAVE_CHUNK`: `Original` si legge dal file con
        //    letture esplicite (non page fault della mappatura), `Add` dal buffer delle aggiunte.
        let mut buf = vec![0u8; SAVE_CHUNK];
        for p in self.table.pieces() {
            let mut done = 0u64; // byte gia' scritti di questo pezzo
            while done < p.len {
                if cancel.load(Ordering::Relaxed) {
                    return Err(SaveError::Cancelled);
                }
                let rest = p.len - done;
                // Il blocco e' gia' limitato da `SAVE_CHUNK`: la conversione non fallisce mai
                // davvero, e il ripiego tiene il conto senza `unwrap`.
                let block = SAVE_CHUNK.min(usize::try_from(rest).unwrap_or(SAVE_CHUNK));
                match p.buf {
                    Buf::Original => {
                        // `read_at` puo' leggere meno del richiesto: si ripete finche' il blocco
                        // non e' pieno. Uno zero in anticipo vuol dire file accorciato da altri
                        // mentre si salvava (lo `stamp` era ancora buono).
                        let base = p.start + done;
                        let mut filled = 0usize;
                        while filled < block {
                            match source.read_at(&mut buf[filled..block], base + filled as u64) {
                                Ok(0) => {
                                    return Err(SaveError::Io(io::Error::new(
                                        io::ErrorKind::UnexpectedEof,
                                        "file accorciato da altri durante il salvataggio",
                                    )))
                                }
                                Ok(n) => filled += n,
                                Err(e) => return Err(SaveError::Io(e)),
                            }
                        }
                        file.write_all(&buf[..block])?;
                        progress.fetch_add(block as u64, Ordering::Relaxed);
                    }
                    Buf::Add => {
                        // I pezzi `Add` stanno sempre dentro il buffer delle aggiunte: la fetta
                        // e' lunga quanto il blocco richiesto.
                        let bytes = self.table.add_bytes(p.start + done, block);
                        // Difesa: una fetta piu' corta del blocco vorrebbe dire scrivere un file
                        // troncato senza accorgersene. Non puo' succedere, ma qui si fallisce forte.
                        if bytes.len() != block {
                            return Err(SaveError::Io(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "buffer delle aggiunte incoerente con la tabella dei pezzi",
                            )));
                        }
                        file.write_all(bytes)?;
                        progress.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                    }
                }
                done += block as u64;
            }
        }

        // 5. Il temporaneo e' completo: si sincronizza su disco, prende i permessi dell'originale
        //    e si chiude PRIMA della rinomina. Ultimo controllo di `cancel`.
        file.sync_all()?;
        file.set_permissions(perms)?;
        drop(file);
        if cancel.load(Ordering::Relaxed) {
            return Err(SaveError::Cancelled);
        }

        // 6. La rinomina sostituisce l'originale con la mappatura ancora attiva (su Windows si
        //    puo' rinominare sopra un file mappato, non troncarlo). Se fallisce, il guard ancora
        //    armato cancella il temporaneo: il documento resta com'era, modificato.
        if let Err(e) = std::fs::rename(&guard.path, &self.path) {
            return Err(SaveError::Io(e));
        }

        // 7. Rinomina riuscita: il temporaneo non esiste piu', il guard si disarma.
        guard.armed = false;
        #[cfg(unix)]
        {
            // Su Unix si sincronizza anche la cartella, perché la rinomina risulti duratura; se
            // fallisce non e' un errore del salvataggio e si ignora.
            if let Some(parent) = self.path.parent() {
                if let Ok(dir) = File::open(parent) {
                    let _ = dir.sync_all();
                }
            }
        }
        match FileSource::open(&self.path) {
            Ok(new_source) => {
                let stamp = match std::fs::metadata(&self.path) {
                    Ok(md) => stamp_of(&md),
                    Err(e) => return Err(SaveError::Io(e)),
                };
                // La sostituzione rilascia la vecchia mappatura; la tabella torna a un solo
                // pezzo `Original` lungo quanto il file appena scritto.
                self.table = PieceTable::new(new_source.len());
                self.source = Some(new_source);
                self.stamp = stamp;
                self.dirty = false;
                // Il file su disco e' cambiato: l'indice del vecchio file non vale piu'.
                self.lines = None;
                self.nl_of_piece.clear();
                self.prefix = None;
                Ok(())
            }
            Err(e) => {
                // I dati sono al sicuro su disco, ma senza sorgente il documento e' chiuso.
                // La rinomina e' comunque riuscita: l'indice del vecchio file non vale piu'.
                self.source = None;
                self.table = PieceTable::new(0);
                self.dirty = false;
                self.lines = None;
                self.nl_of_piece.clear();
                self.prefix = None;
                Err(SaveError::Io(e))
            }
        }
    }
}
