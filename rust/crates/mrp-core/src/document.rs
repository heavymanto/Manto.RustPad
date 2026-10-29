//! Documento modificabile: un file (mappato, mai copiato) + una `PieceTable` con le modifiche.
//! Il salvataggio scrive un file temporaneo accanto all'originale, lo sincronizza su disco e lo
//! rinomina sopra l'originale: se qualcosa va storto (errore, annullamento, file cambiato da
//! altri) l'originale resta com'e'.

use crate::edit::{Buf, EditError, PieceTable};
use crate::source::FileSource;
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

/// File + modifiche.
pub struct Document {
    path: PathBuf,
    /// `None` solo se un salvataggio e' riuscito a rinominare ma non a riaprire (documento chiuso, lungo 0).
    source: Option<FileSource>,
    table: PieceTable,
    stamp: Stamp,
    /// Vero se c'e' almeno una modifica non salvata.
    dirty: bool,
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
        }
        Ok(())
    }

    /// Come `PieceTable::delete`. Una cancellazione di `len == 0` non segna il documento come modificato.
    pub fn delete(&mut self, pos: u64, len: u64) -> Result<(), EditError> {
        self.table.delete(pos, len)?;
        // Una cancellazione di lunghezza zero non ha effetto: il documento non risulta modificato.
        if len > 0 {
            self.dirty = true;
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
                Ok(())
            }
            Err(e) => {
                // I dati sono al sicuro su disco, ma senza sorgente il documento e' chiuso.
                self.source = None;
                self.table = PieceTable::new(0);
                self.dirty = false;
                Err(SaveError::Io(e))
            }
        }
    }
}
