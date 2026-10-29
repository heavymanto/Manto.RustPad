//! Sorgente di byte di sola lettura, mappata in memoria.

use memmap2::Mmap;
use std::fs::File;
use std::io;
use std::path::Path;

/// File aperto in sola lettura. Non copia il contenuto: le pagine vengono caricate dal sistema operativo quando servono.
pub struct FileSource {
    /// Mappatura del file; `None` se il file e' vuoto, perche' `Mmap::map` fallisce sui file vuoti.
    map: Option<Mmap>,
    /// Il file aperto, tenuto per le letture esplicite (precaricamento): vedi `read_at`.
    file: File,
}

impl FileSource {
    /// Apre `path`. Errore se non esiste, non e' leggibile o non e' un file regolare.
    /// Un file vuoto e' valido (lunghezza 0).
    pub fn open(path: &Path) -> io::Result<FileSource> {
        let file = File::open(path)?;
        let md = file.metadata()?;
        if !md.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "non e' un file regolare",
            ));
        }
        let len = md.len();
        if len > usize::MAX as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "file piu' grande di usize::MAX",
            ));
        }
        let map = if len == 0 {
            None
        } else {
            // SAFETY: il file e' aperto in sola lettura; se un altro processo lo accorcia
            // durante l'uso il sistema operativo puo' segnalare un errore di accesso
            // (rischio noto, accettato).
            Some(unsafe { Mmap::map(&file)? })
        };
        Ok(FileSource { map, file })
    }

    /// Lettura esplicita a una posizione (non sposta nessun cursore condiviso). Serve al
    /// precaricamento: su un disco meccanico una lettura grande e sequenziale e' molto piu'
    /// veloce dei page fault della mappatura (misurato: 149 MB/s contro 28-39 MB/s). Le pagine
    /// lette finiscono nella cache di sistema, dove la mappatura le trova gia' pronte.
    pub(crate) fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;
            self.file.seek_read(buf, offset)
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;
            self.file.read_at(buf, offset)
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = (buf, offset, &self.file);
            Err(io::Error::from(io::ErrorKind::Unsupported))
        }
    }

    /// Lunghezza in byte.
    pub fn len(&self) -> u64 {
        self.map.as_ref().map_or(0, |m| m.len() as u64)
    }

    /// Vero se il file e' vuoto.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Tutto il contenuto come fetta di byte (mappatura, non copia).
    pub fn bytes(&self) -> &[u8] {
        self.map.as_ref().map_or(&[], |m| &m[..])
    }

    /// Fino a `len` byte a partire da `start`. Mai un errore: `start` oltre la fine da' una fetta vuota,
    /// `len` oltre la fine viene accorciato.
    pub fn read_range(&self, start: u64, len: usize) -> &[u8] {
        let file_len = self.len();
        if start >= file_len {
            return &[];
        }
        let end = start.saturating_add(len as u64).min(file_len);
        // `open` garantisce file_len <= usize::MAX e start < end <= file_len: le conversioni
        // non falliscono mai, ma fuori dai test non si usa `unwrap` (regola del crate).
        match (usize::try_from(start), usize::try_from(end)) {
            (Ok(s), Ok(e)) => &self.bytes()[s..e],
            _ => &[],
        }
    }
}
