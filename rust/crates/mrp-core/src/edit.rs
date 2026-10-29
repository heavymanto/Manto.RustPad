//! Piece table: le modifiche sono una lista di "pezzi" che puntano al file originale (mai copiato)
//! o a un piccolo buffer di aggiunte. La memoria dipende dalle modifiche, non dalla dimensione del file.
//!
//! Forma canonica (invarianti che ogni operazione deve lasciare vere):
//! - nessun pezzo ha `len == 0`;
//! - la somma delle `len` dei pezzi e' `len()`;
//! - due pezzi adiacenti dello stesso buffer e contigui (`a.start + a.len == b.start`) sono fusi in uno.
//!
//! Il file originale non viene mai modificato qui: il salvataggio e' un altro compito.

use crate::source::FileSource;

/// Buffer a cui punta un pezzo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Buf {
    /// Il file originale (posizioni in byte dall'inizio del file).
    Original,
    /// Il buffer delle aggiunte (posizioni in byte dall'inizio del buffer).
    Add,
}

/// Un tratto contiguo di `len` byte che parte da `start` nel buffer `buf`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub buf: Buf,
    pub start: u64,
    pub len: u64,
}

/// Errore di una modifica: l'operazione non ha cambiato nulla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// `pos` (o `pos + len`) cade oltre la fine del documento, che e' lungo `doc_len`.
    /// `len` e' la lunghezza richiesta: `data.len()` per `insert`, il parametro `len` per `delete`.
    OutOfRange { pos: u64, len: u64, doc_len: u64 },
}

/// Documento = sequenza di pezzi sopra un file originale lungo `original_len` byte.
#[derive(Debug, Clone)]
pub struct PieceTable {
    // Lunghezza del file di partenza: nessuna operazione di E1 la legge (la dicono i pezzi),
    // servira' ai compiti successivi (salvataggio). Va marcata perche' rustc ignora i derive
    // nell'analisi del codice morto e `clippy -D warnings` rifiuterebbe il campo.
    #[allow(dead_code)]
    original_len: u64,
    add: Vec<u8>,
    pieces: Vec<Piece>,
}

impl PieceTable {
    /// Documento uguale al file originale: un solo pezzo `Original 0..original_len`
    /// (nessun pezzo se `original_len == 0`).
    pub fn new(original_len: u64) -> Self {
        let pieces = if original_len > 0 {
            vec![Piece {
                buf: Buf::Original,
                start: 0,
                len: original_len,
            }]
        } else {
            Vec::new()
        };
        PieceTable {
            original_len,
            add: Vec::new(),
            pieces,
        }
    }

    /// Lunghezza attuale del documento in byte.
    pub fn len(&self) -> u64 {
        // Ciclo semplice, O(pezzi): renderla O(1) e' un compito successivo.
        let mut n = 0u64;
        for p in &self.pieces {
            n += p.len;
        }
        n
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// I pezzi, in ordine di documento, in forma canonica.
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Byte totali ora nel buffer delle aggiunte: cresce a ogni `insert`, non cala con `delete`.
    pub fn add_len(&self) -> u64 {
        self.add.len() as u64
    }

    /// Fetta del buffer delle aggiunte: serve al salvataggio. Fuori dai limiti restituisce una fetta vuota.
    pub(crate) fn add_bytes(&self, start: u64, len: usize) -> &[u8] {
        // Come le altre conversioni u64 -> usize del crate: mai `unwrap`, fuori dai limiti si
        // restituisce il vuoto.
        let s = match usize::try_from(start) {
            Ok(s) => s,
            // Inizio oltre `usize::MAX`.
            Err(_) => return &[],
        };
        let e = match s.checked_add(len) {
            Some(e) => e,
            // Fine oltre `usize::MAX` (restare oltre la fine del buffer lo segnala `get`).
            None => return &[],
        };
        self.add.get(s..e).unwrap_or(&[])
    }

    /// Inserisce `data` alla posizione `pos` (0..=len()). `data` vuoto = nessun effetto (ma `pos` e' comunque controllato).
    /// Errore `OutOfRange` se `pos > len()`; in caso di errore il documento resta identico.
    pub fn insert(&mut self, pos: u64, data: &[u8]) -> Result<(), EditError> {
        // Controllo PRIMA di toccare qualunque cosa: in caso di errore il documento resta identico.
        let doc_len = self.len();
        if pos > doc_len {
            return Err(EditError::OutOfRange {
                pos,
                len: data.len() as u64,
                doc_len,
            });
        }
        if data.is_empty() {
            return Ok(());
        }
        // I byte nuovi vanno in coda al buffer delle aggiunte...
        let start = self.add.len() as u64;
        self.add.extend_from_slice(data);
        // ...e il pezzo nuovo entra davanti al pezzo che inizia esattamente a `pos`.
        let i = self.split_at(pos);
        self.pieces.insert(
            i,
            Piece {
                buf: Buf::Add,
                start,
                len: data.len() as u64,
            },
        );
        self.normalize();
        Ok(())
    }

    /// Cancella `len` byte a partire da `pos`. `len == 0` = nessun effetto (ma `pos <= len()` e' controllato).
    /// Errore `OutOfRange` se `pos + len > len()` (anche per overflow di u64); in caso di errore il documento resta identico.
    pub fn delete(&mut self, pos: u64, len: u64) -> Result<(), EditError> {
        // Controllo PRIMA di toccare qualunque cosa: in caso di errore il documento resta identico.
        let doc_len = self.len();
        let fine = match pos.checked_add(len) {
            Some(fine) if fine <= doc_len => fine,
            _ => return Err(EditError::OutOfRange { pos, len, doc_len }),
        };
        if len == 0 {
            return Ok(());
        }
        // Primo pezzo tolto e primo pezzo tenuto del tratto. Il secondo spezzamento non sposta
        // `i`: agisce solo su pezzi che vengono dopo quello che inizia a `pos`.
        let i = self.split_at(pos);
        let j = self.split_at(fine);
        self.pieces.drain(i..j);
        // Il buffer delle aggiunte NON si accorcia: spariscono i pezzi, non i byte.
        self.normalize();
        Ok(())
    }

    /// Copia fino a `len` byte a partire da `pos` (come `FileSource::read_range`: si adatta ai limiti, mai errore).
    /// `original` e' il file a cui si riferiscono i pezzi `Original`.
    pub fn read_range(&self, original: &FileSource, pos: u64, len: u64) -> Vec<u8> {
        let doc_len = self.len();
        if pos >= doc_len {
            return Vec::new();
        }
        // Quanto leggere al massimo: `len` gia' limitato alla parte di documento che resta
        // (mai il valore grezzo, che puo' essere `u64::MAX` e non deve finire in una riserva).
        let want = len.min(doc_len - pos);
        let mut out = match usize::try_from(want) {
            Ok(n) => Vec::with_capacity(n),
            // Documento solo numerico, piu' lungo di quanto `usize` possa contare: ci si
            // ferma subito con quanto raccolto (niente).
            Err(_) => return Vec::new(),
        };
        let read_end = pos + want; // non va in overflow: e' <= doc_len
        let mut offset = 0u64; // posizione di documento dell'inizio del pezzo corrente
        let mut copied = 0u64; // byte copiati finora
        for p in &self.pieces {
            if copied >= want || offset >= read_end {
                break; // letto tutto quello che serviva, o la lettura e' gia' finita
            }
            let piece_end = offset + p.len; // fine del pezzo nel documento
            if piece_end > pos {
                // Il pezzo interessa (tutto o in parte): `skip` byte saltati dentro il pezzo,
                // `take` byte da copiare.
                let skip = pos.max(offset) - offset;
                let take = piece_end.min(read_end) - offset - skip;
                if take > 0 {
                    // Conversioni u64 -> usize: falliscono solo su un documento piu' grande
                    // di `usize::MAX`; ci si ferma con i byte gia' raccolti (mai panic).
                    let n = match usize::try_from(take) {
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    let base = p.start + skip;
                    let chunk = match p.buf {
                        // Il file originale si legge a fetta: `read_range` e' gia' limitato
                        // ai bordi e non da' mai errore.
                        Buf::Original => original.read_range(base, n),
                        Buf::Add => match usize::try_from(base) {
                            Ok(b) => &self.add[b..b + n],
                            Err(_) => break,
                        },
                    };
                    out.extend_from_slice(chunk);
                    copied += take;
                }
            }
            offset = piece_end;
        }
        out
    }

    /// Indice del pezzo che INIZIA esattamente alla posizione di documento `pos`, spezzando in
    /// due il pezzo che la contiene se `pos` cade nel suo mezzo. Se `pos == len()` restituisce
    /// `pieces.len()`. Presuppone `pos <= len()`.
    fn split_at(&mut self, pos: u64) -> usize {
        // Prima cerca, senza toccare il vettore, il pezzo che contiene `pos` nel mezzo.
        let mut offset = 0u64; // posizione di documento dell'inizio del pezzo corrente
        let mut cut = None;
        for (i, p) in self.pieces.iter().enumerate() {
            if offset == pos {
                return i; // un pezzo inizia gia' esattamente a `pos`
            }
            if offset + p.len > pos {
                cut = Some((i, pos - offset));
                break;
            }
            offset += p.len;
        }
        match cut {
            // Nessun pezzo contiene `pos`: e' `pos == len()`, dopo l'ultimo pezzo.
            None => self.pieces.len(),
            Some((i, c)) => {
                // La parte sinistra del pezzo resta al suo posto, la destra e' un pezzo
                // nuovo che inizia esattamente a `pos`.
                let p = self.pieces[i];
                self.pieces[i] = Piece {
                    buf: p.buf,
                    start: p.start,
                    len: c,
                };
                self.pieces.insert(
                    i + 1,
                    Piece {
                        buf: p.buf,
                        start: p.start + c,
                        len: p.len - c,
                    },
                );
                i + 1
            }
        }
    }

    /// Riporta i pezzi in forma canonica: elimina i pezzi con `len == 0` e fonde le coppie
    /// adiacenti dello stesso buffer e contigue (`a.start + a.len == b.start`), in un solo
    /// passaggio su un vettore nuovo.
    fn normalize(&mut self) {
        let mut out: Vec<Piece> = Vec::with_capacity(self.pieces.len());
        for p in self.pieces.drain(..) {
            if p.len == 0 {
                continue;
            }
            if let Some(last) = out.last_mut() {
                if last.buf == p.buf && last.start + last.len == p.start {
                    last.len += p.len;
                    continue;
                }
            }
            out.push(p);
        }
        self.pieces = out;
    }
}
