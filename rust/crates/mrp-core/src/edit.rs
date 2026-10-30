//! Piece table: le modifiche sono una lista di "pezzi" che puntano al file originale (mai copiato)
//! o a un piccolo buffer di aggiunte. La memoria dipende dalle modifiche, non dalla dimensione del file.
//!
//! Forma canonica (invarianti che ogni operazione deve lasciare vere):
//! - nessun pezzo ha `len == 0`;
//! - la somma delle `len` dei pezzi e' `len()`;
//! - due pezzi adiacenti dello stesso buffer e contigui (`a.start + a.len == b.start`) sono fusi in uno.
//!
//! I pezzi stanno in una lista a blocchi: ogni blocco tiene al piu' `cap` pezzi con la somma dei
//! loro byte in cache, cosi' una modifica tocca solo i blocchi che la incrociano e il suo costo
//! e' O(blocchi + cap), non O(pezzi). Invarianti dei blocchi, vere dopo ogni operazione: nessun
//! blocco vuoto; ogni blocco ha al piu' `cap` pezzi; due blocchi vicini sommano piu' di `cap / 2`
//! pezzi (se fossero piu' piccoli si fonderebbero); i contatori (`count`, `total` e i `bytes` dei
//! blocchi) sono sempre esatti. Un documento senza pezzi non ha blocchi.
//!
//! Il file originale non viene mai modificato qui: il salvataggio e' un altro compito.

use crate::source::FileSource;

/// Pezzi per blocco della lista, come li usa `PieceTable::new` (chi vuole blocchi piu' piccoli
/// chiama `with_chunk_capacity`).
pub const DEFAULT_CHUNK_CAPACITY: usize = 512;

/// Buffer a cui punta un pezzo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Buf {
    /// Il file originale (posizioni in byte dall'inizio del file).
    Original,
    /// Il buffer delle aggiunte (posizioni in byte dall'inizio del buffer).
    Add,
}

/// Un tratto contiguo di `len` byte che parte da `start` nel buffer `buf`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Una modifica al vettore dei pezzi, registrata per l'annulla/ripeti: al posto dei pezzi
/// `removed`, a partire dall'indice `index` nel vettore com'era prima, ora ci sono `inserted`.
/// Applicarla trasforma il vettore di prima in quello di adesso, invertirla fa il contrario.
/// Contiene solo pezzi (riferimenti ai buffer), mai copie di dati del documento.
#[derive(Debug, Clone)]
pub struct Splice {
    /// Indice del primo pezzo sostituito, nel vettore com'era PRIMA della modifica.
    index: usize,
    /// I pezzi che c'erano prima.
    removed: Vec<Piece>,
    /// I pezzi che ci sono dopo.
    inserted: Vec<Piece>,
}

/// La differenza minima fra il vettore dei pezzi prima e dopo una modifica: si tolgono il
/// prefisso e il suffisso comuni e quel che resta e' lo `Splice` che trasforma `prima` in
/// `dopo` esattamente, pezzo per pezzo (se entrambi sono in forma canonica, lo resta anche
/// il risultato).
fn splice_between(prima: &[Piece], dopo: &[Piece]) -> Splice {
    // Prefisso comune: i pezzi uguali in testa non entrano nello splice.
    let mut pre = 0;
    while pre < prima.len() && pre < dopo.len() && prima[pre] == dopo[pre] {
        pre += 1;
    }
    // Suffisso comune, senza sconfinare nel prefisso gia' tolto.
    let mut suf = 0;
    while suf < prima.len() - pre
        && suf < dopo.len() - pre
        && prima[prima.len() - 1 - suf] == dopo[dopo.len() - 1 - suf]
    {
        suf += 1;
    }
    Splice {
        index: pre,
        removed: prima[pre..prima.len() - suf].to_vec(),
        inserted: dopo[pre..dopo.len() - suf].to_vec(),
    }
}

/// Forma canonica di una finestra di pezzi (quello che `normalize` garantiva su tutta la lista,
/// ristretto ai soli pezzi che una modifica riscrive): butta i pezzi con `len == 0` e fonde le
/// coppie adiacenti dello stesso buffer e contigue (`a.start + a.len == b.start`).
fn canonicalize(finestra: &[Piece]) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::with_capacity(finestra.len());
    for p in finestra.iter().copied() {
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
    out
}

/// Un blocco della lista dei pezzi: al piu' `cap` pezzi, con i byte totali in cache cosi' chi
/// cerca una posizione di documento lo salta tutto d'un fiato.
#[derive(Debug, Clone)]
struct Chunk {
    pieces: Vec<Piece>,
    /// Somma delle `len` dei pezzi del blocco.
    bytes: u64,
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
    /// I pezzi in ordine di documento, a blocchi (vedi `Chunk`).
    chunks: Vec<Chunk>,
    /// Pezzi totali: la somma delle lunghezze dei blocchi.
    count: usize,
    /// Byte totali del documento: la somma delle `len` dei pezzi.
    total: u64,
    /// Massimo numero di pezzi per blocco (mai meno di 4).
    cap: usize,
}

impl PieceTable {
    /// Documento uguale al file originale: un solo pezzo `Original 0..original_len`
    /// (nessun pezzo se `original_len == 0`).
    pub fn new(original_len: u64) -> Self {
        PieceTable::with_chunk_capacity(original_len, DEFAULT_CHUNK_CAPACITY)
    }

    /// Come `new`, ma i pezzi stanno in blocchi di al piu' `cap` pezzi (`cap` minore di 4 vale 4).
    /// Serve ai test per avere molti blocchi su documenti piccoli; `new` usa 512.
    pub fn with_chunk_capacity(original_len: u64, cap: usize) -> Self {
        let cap = cap.max(4);
        let (chunks, count, total) = if original_len > 0 {
            (
                vec![Chunk {
                    pieces: vec![Piece {
                        buf: Buf::Original,
                        start: 0,
                        len: original_len,
                    }],
                    bytes: original_len,
                }],
                1,
                original_len,
            )
        } else {
            // Un documento senza pezzi non ha blocchi.
            (Vec::new(), 0, 0)
        };
        PieceTable {
            original_len,
            add: Vec::new(),
            chunks,
            count,
            total,
            cap,
        }
    }

    /// Quanti pezzi ci sono (O(1)).
    pub fn piece_count(&self) -> usize {
        self.count
    }

    /// Il pezzo numero `i` in ordine di documento; `None` se `i >= piece_count()`. I blocchi si
    /// saltano con la loro lunghezza in pezzi: O(blocchi + cap).
    pub fn piece(&self, i: usize) -> Option<Piece> {
        let mut base = 0usize; // indice globale del primo pezzo del blocco corrente
        for c in &self.chunks {
            if i < base + c.pieces.len() {
                return c.pieces.get(i - base).copied();
            }
            base += c.pieces.len();
        }
        None
    }

    /// I pezzi in ordine di documento, senza copiarli tutti.
    pub fn iter(&self) -> impl Iterator<Item = Piece> + '_ {
        self.chunks.iter().flat_map(|c| c.pieces.iter().copied())
    }

    /// Quanti pezzi ha ciascun blocco, in ordine.
    pub fn chunk_sizes(&self) -> Vec<usize> {
        self.chunks.iter().map(|c| c.pieces.len()).collect()
    }

    /// Lunghezza attuale del documento in byte (O(1): la somma sta in `total`).
    pub fn len(&self) -> u64 {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// I pezzi, in ordine di documento, in forma canonica, come vettore nuovo (O(pezzi)): resta
    /// per i test e per chi vuole tutti i pezzi in una volta; il codice interno usa `iter` e
    /// `piece`, che non copiano la lista.
    pub fn pieces(&self) -> Vec<Piece> {
        let mut v = Vec::with_capacity(self.count);
        for c in &self.chunks {
            v.extend_from_slice(&c.pieces);
        }
        v
    }

    /// Il massimo numero di pezzi per blocco di questa tabella: serve a rifare una tabella con
    /// lo stesso blocco, ad esempio dopo un salvataggio.
    pub(crate) fn chunk_capacity(&self) -> usize {
        self.cap
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
        self.insert_tracked(pos, data).map(|_| ())
    }

    /// Come `insert`, ma restituisce anche la modifica al vettore dei pezzi (`None` se
    /// l'inserimento non ha effetto): serve a `Document` per registrare la storia dell'annulla.
    ///
    /// Si riscrive solo la finestra dei pezzi `[i-1, i]`, dove `i` e' il pezzo che contiene
    /// `pos` (o che inizia a `pos`; `count` se `pos == len`): fuori dalla finestra i pezzi non
    /// cambiano. La finestra nuova ([pezzo `i-1`] + [parte sinistra di `i` fino a `pos`] + [pezzo
    /// nuovo `Add`] + [parte destra di `i`], resa canonica) si applica con `replace`, che tocca
    /// solo i blocchi coinvolti.
    pub(crate) fn insert_tracked(
        &mut self,
        pos: u64,
        data: &[u8],
    ) -> Result<Option<Splice>, EditError> {
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
            return Ok(None);
        }
        // I byte nuovi vanno in coda al buffer delle aggiunte...
        let start = self.add.len() as u64;
        self.add.extend_from_slice(data);
        // ...e diventano un pezzo nuovo dentro la finestra.
        let (i, inizio_i) = self.locate_byte(pos);
        let w0 = i.saturating_sub(1); // primo pezzo della finestra
        let vecchia = self.window(w0, i);
        let mut grezza: Vec<Piece> = Vec::new();
        if let Some(p) = i.checked_sub(1).and_then(|k| self.piece(k)) {
            grezza.push(p);
        }
        if let Some(p) = self.piece(i) {
            // Parte sinistra di i, fino a `pos` (vuota se `pos` e' l'inizio del pezzo).
            if pos > inizio_i {
                grezza.push(Piece {
                    buf: p.buf,
                    start: p.start,
                    len: pos - inizio_i,
                });
            }
            grezza.push(Piece {
                buf: Buf::Add,
                start,
                len: data.len() as u64,
            });
            // Parte destra di i: c'e' sempre, perche' il pezzo i finisce dopo `pos`.
            grezza.push(Piece {
                buf: p.buf,
                start: p.start + (pos - inizio_i),
                len: inizio_i + p.len - pos,
            });
        } else {
            // `pos == len`: il pezzo nuovo va in coda, dopo l'ultimo.
            grezza.push(Piece {
                buf: Buf::Add,
                start,
                len: data.len() as u64,
            });
        }
        let nuova = canonicalize(&grezza);
        // Lo splice e' la differenza minima fra finestra vecchia e nuova, con l'indice riportato
        // al vettore globale; `replace` la applica senza toccare i pezzi fuori dalla finestra.
        let mut sp = splice_between(&vecchia, &nuova);
        sp.index += w0;
        self.replace(sp.index, sp.removed.len(), &sp.inserted);
        Ok(Some(sp))
    }

    /// Cancella `len` byte a partire da `pos`. `len == 0` = nessun effetto (ma `pos <= len()` e' controllato).
    /// Errore `OutOfRange` se `pos + len > len()` (anche per overflow di u64); in caso di errore il documento resta identico.
    pub fn delete(&mut self, pos: u64, len: u64) -> Result<(), EditError> {
        self.delete_tracked(pos, len).map(|_| ())
    }

    /// Come `delete`, ma restituisce anche la modifica al vettore dei pezzi (`None` se la
    /// cancellazione non ha effetto): serve a `Document` per registrare la storia dell'annulla.
    ///
    /// Come l'inserimento, ma la finestra e' `[i-1 ..= j+1]`: `i` e' il pezzo che contiene
    /// `pos`, `j` l'ultimo che inizia prima della fine del tratto cancellato. La finestra nuova
    /// ([pezzo `i-1`] + [parte sinistra di `i` fino a `pos`] + [parte destra di `j` da `fine`] +
    /// [pezzo `j+1`], resa canonica: anche `i-1` e `j+1` si fondono se la cancellazione li rende
    /// contigui e dello stesso buffer) sostituisce la vecchia.
    pub(crate) fn delete_tracked(
        &mut self,
        pos: u64,
        len: u64,
    ) -> Result<Option<Splice>, EditError> {
        // Controllo PRIMA di toccare qualunque cosa: in caso di errore il documento resta identico.
        let doc_len = self.len();
        let fine = match pos.checked_add(len) {
            Some(fine) if fine <= doc_len => fine,
            _ => return Err(EditError::OutOfRange { pos, len, doc_len }),
        };
        if len == 0 {
            return Ok(None);
        }
        let (i, inizio_i) = self.locate_byte(pos);
        // `j` e' l'ultimo pezzo che inizia prima di `fine`: quello che contiene l'ultimo byte
        // cancellato (`fine - 1`, che esiste perche' `len > 0`).
        let (j, inizio_j) = self.locate_byte(fine - 1);
        let w0 = i.saturating_sub(1); // primo pezzo della finestra
        let vecchia = self.window(w0, j + 1);
        let mut grezza: Vec<Piece> = Vec::new();
        if let Some(p) = i.checked_sub(1).and_then(|k| self.piece(k)) {
            grezza.push(p);
        }
        if let Some(p) = self.piece(i) {
            // Parte sinistra di i, fino a `pos` (vuota se `pos` e' l'inizio del pezzo).
            if pos > inizio_i {
                grezza.push(Piece {
                    buf: p.buf,
                    start: p.start,
                    len: pos - inizio_i,
                });
            }
        }
        if let Some(p) = self.piece(j) {
            // Parte destra di j, da `fine` in poi (vuota se la cancellazione finisce col pezzo).
            if fine < inizio_j + p.len {
                grezza.push(Piece {
                    buf: p.buf,
                    start: p.start + (fine - inizio_j),
                    len: inizio_j + p.len - fine,
                });
            }
        }
        if let Some(p) = j.checked_add(1).and_then(|k| self.piece(k)) {
            grezza.push(p);
        }
        let nuova = canonicalize(&grezza);
        let mut sp = splice_between(&vecchia, &nuova);
        sp.index += w0;
        self.replace(sp.index, sp.removed.len(), &sp.inserted);
        Ok(Some(sp))
    }

    /// Applica `s` alla lista dei pezzi: i pezzi `s.removed` a partire da `s.index` diventano
    /// `s.inserted` (vettore com'era prima della modifica -> vettore com'e' adesso). Serve al ripeti.
    pub(crate) fn apply_splice(&mut self, s: &Splice) {
        // Difesa: uno splice che non combacia col vettore attuale non puo' arrivare con le
        // invarianti vere (ogni splice nasce da stati realmente attraversati, in ordine); lo
        // si ignora piuttosto che fare panic.
        match s.index.checked_add(s.removed.len()) {
            Some(fine) if fine <= self.count => self.replace(s.index, s.removed.len(), &s.inserted),
            _ => {}
        }
    }

    /// Inverte `s`: i pezzi `s.inserted` a partire da `s.index` tornano `s.removed` (vettore
    /// com'e' adesso -> vettore com'era prima della modifica). Serve all'annulla.
    pub(crate) fn revert_splice(&mut self, s: &Splice) {
        // Stessa difesa di `apply_splice`.
        match s.index.checked_add(s.inserted.len()) {
            Some(fine) if fine <= self.count => self.replace(s.index, s.inserted.len(), &s.removed),
            _ => {}
        }
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
        for c in &self.chunks {
            if copied >= want || offset >= read_end {
                break; // letto tutto quello che serviva, o la lettura e' gia' finita
            }
            let chunk_end = offset + c.bytes; // fine del blocco nel documento
            if chunk_end > pos {
                // Il blocco finisce oltre `pos`: qualche suo pezzo puo' interessare.
                for p in &c.pieces {
                    if copied >= want || offset >= read_end {
                        break; // letto tutto quello che serviva, o la lettura e' gia' finita
                    }
                    let piece_end = offset + p.len; // fine del pezzo nel documento
                    if piece_end > pos {
                        // Il pezzo interessa (tutto o in parte): `skip` byte saltati dentro il
                        // pezzo, `take` byte da copiare.
                        let skip = pos.max(offset) - offset;
                        let take = piece_end.min(read_end) - offset - skip;
                        if take > 0 {
                            // Conversioni u64 -> usize: falliscono solo su un documento piu'
                            // grande di `usize::MAX`; ci si ferma con i byte gia' raccolti
                            // (mai panic).
                            let n = match usize::try_from(take) {
                                Ok(n) => n,
                                Err(_) => break,
                            };
                            let base = p.start + skip;
                            let chunk = match p.buf {
                                // Il file originale si legge a fetta: `read_range` e' gia'
                                // limitato ai bordi e non da' mai errore.
                                Buf::Original => original.read_range(base, n),
                                Buf::Add => match usize::try_from(base) {
                                    Ok(b) => match b.checked_add(n) {
                                        Some(e) => self.add.get(b..e).unwrap_or(&[]),
                                        None => break,
                                    },
                                    Err(_) => break,
                                },
                            };
                            out.extend_from_slice(chunk);
                            copied += take;
                        }
                    }
                    offset = piece_end;
                }
            }
            // Il blocco e' stato scorso (o saltato): la prossima partenza e' la sua fine, che
            // resta esatta anche se il ciclo dei pezzi si e' interrotto prima.
            offset = chunk_end;
        }
        out
    }

    // ---------- lista a blocchi (primitive private) ----------

    /// Cerca la posizione di documento `pos` (dev'essere `<= len()`): restituisce l'indice
    /// globale del primo pezzo la cui fine viene dopo `pos` e la posizione di documento del suo
    /// inizio. Se `pos == len()` l'indice e' `count` (nessun pezzo finisce dopo la fine). Il
    /// pezzo trovato e' quello che contiene `pos`, oppure quello che inizia esattamente a `pos`.
    /// I blocchi prima si saltano con i loro `bytes`: O(blocchi + cap).
    fn locate_byte(&self, pos: u64) -> (usize, u64) {
        let mut base = 0usize; // indice globale del primo pezzo del blocco corrente
        let mut offset = 0u64; // posizione di documento dell'inizio del pezzo corrente
        for c in &self.chunks {
            if offset + c.bytes > pos {
                // Il pezzo cercato sta in questo blocco: lo si trova scorrendo i pezzi.
                for p in &c.pieces {
                    if offset + p.len > pos {
                        return (base, offset);
                    }
                    offset += p.len;
                    base += 1;
                }
            } else {
                // Il blocco finisce prima di `pos`: lo si salta con i byte in cache.
                offset += c.bytes;
                base += c.pieces.len();
            }
        }
        (self.count, self.total)
    }

    /// I pezzi con indice globale in `da..=a`, fermati all'ultimo che esiste: e' la finestra
    /// vecchia di una modifica (i pezzi fuori dalla finestra non cambiano e non si copiano).
    fn window(&self, da: usize, a: usize) -> Vec<Piece> {
        let mut v = Vec::new();
        let mut base = 0usize; // indice globale del primo pezzo del blocco corrente
        for c in &self.chunks {
            // Intersezione del blocco [base, base + n) con la finestra [da, a].
            let n = c.pieces.len();
            let inizio = base.max(da);
            let fine = (base + n).min(a.saturating_add(1));
            if inizio < fine {
                v.extend_from_slice(&c.pieces[inizio - base..fine - base]);
            }
            base += n;
            if base > a {
                break; // la finestra e' gia' tutta raccolta
            }
        }
        v
    }

    /// Il blocco che contiene il pezzo globale `index`, con l'indice globale del suo primo pezzo.
    /// Con `index == count` (aggiungere in coda) restituisce l'ultimo blocco; con la lista vuota
    /// (0, 0), che il chiamante tratta a parte. Presuppone `index <= count`.
    fn chunk_at(&self, index: usize) -> (usize, usize) {
        let mut base = 0usize; // indice globale del primo pezzo del blocco corrente
        let mut ultimo = (0usize, 0usize);
        for (k, c) in self.chunks.iter().enumerate() {
            let n = c.pieces.len();
            if index < base + n {
                return (k, base);
            }
            ultimo = (k, base);
            base += n;
        }
        ultimo
    }

    /// Sostituisce `remove` pezzi a partire dall'indice globale `index` con `nuovi`, toccando
    /// solo i blocchi coinvolti: i pezzi tolti possono stare in piu' blocchi (la coda del primo,
    /// teste dei successivi), i nuovi entrano tutti nel primo blocco toccato. Poi `fix_chunks`
    /// rimette a posto i blocchi. Se `index + remove > count` non fa nulla (difesa, come le
    /// difese di `apply_splice`).
    fn replace(&mut self, index: usize, remove: usize, nuovi: &[Piece]) {
        // Difesa: un intervallo che esce dai pezzi non si tocca.
        if index > self.count || remove > self.count - index {
            return;
        }
        if remove == 0 && nuovi.is_empty() {
            return;
        }
        let nuovi_bytes: u64 = nuovi.iter().map(|p| p.len).sum();
        // Documento senza blocchi (nessun pezzo): il primo blocco nasce qui e poi si sistema.
        if self.chunks.is_empty() {
            self.chunks.push(Chunk {
                pieces: nuovi.to_vec(),
                bytes: nuovi_bytes,
            });
            self.count = nuovi.len();
            self.total = nuovi_bytes;
            self.fix_chunks(0, 0);
            return;
        }
        let (ci, base) = self.chunk_at(index);
        // Posto del primo pezzo tolto dentro il blocco `ci`.
        let off = index - base;
        // 1. I pezzi tolti: prima la parte che sta nel blocco `ci`, con i byte che finiscono in
        //    `rimossi`...
        let mut rimossi = 0u64;
        let mut restanti = remove;
        let n_ci = match self.chunks.get(ci) {
            Some(c) => c.pieces.len(),
            None => return, // difesa: non puo' succedere (chunk_at torna un blocco esistente)
        };
        let prende = restanti.min(n_ci - off);
        if prende > 0 {
            rimossi += self.drain_chunk(ci, off, prende);
            restanti -= prende;
        }
        // ...poi le teste dei blocchi successivi, finche' ce ne sono da togliere.
        let mut k = ci + 1;
        while restanti > 0 {
            let n = match self.chunks.get(k) {
                Some(c) => c.pieces.len(),
                None => break, // difesa: non puo' succedere (i pezzi tolti stanno nella lista)
            };
            let prende = restanti.min(n);
            rimossi += self.drain_chunk(k, 0, prende);
            restanti -= prende;
            k += 1;
        }
        // L'ultimo blocco toccato.
        let ultimo = k - 1;
        // 2. I pezzi nuovi entrano nel primo blocco toccato, al posto dei tolti.
        if !nuovi.is_empty() {
            if let Some(c) = self.chunks.get_mut(ci) {
                c.pieces.splice(off..off, nuovi.iter().copied());
                c.bytes += nuovi_bytes;
            }
        }
        // 3. I contatori restano esatti.
        self.count = self.count - remove + nuovi.len();
        self.total = self.total - rimossi + nuovi_bytes;
        // 4. I blocchi coinvolti tornano a rispettare le invarianti.
        self.fix_chunks(ci, ultimo);
    }

    /// Toglie `n` pezzi dal blocco `k` a partire dal posto `off` (il chiamante garantisce che
    /// stanno dentro il blocco) e restituisce quanti byte avevano.
    fn drain_chunk(&mut self, k: usize, off: usize, n: usize) -> u64 {
        let Some(c) = self.chunks.get_mut(k) else {
            return 0;
        };
        let mut tolti = 0u64;
        for p in c.pieces.drain(off..off + n) {
            tolti += p.len;
        }
        c.bytes -= tolti;
        tolti
    }

    /// Rimette a posto i blocchi dopo che `replace` ha toccato i blocchi `da..=a` (estremi
    /// inclusi) e i loro vicini: butta i blocchi rimasti vuoti (possono stare solo nella regione
    /// toccata), spezza quello rifarcito se supera `cap`, fonde le coppie di vicini che insieme
    /// starebbero in mezzo blocco. Fuori dalla regione le coppie erano gia' larghe prima e non
    /// sono cambiate, quindi non si guardano.
    fn fix_chunks(&mut self, da: usize, a: usize) {
        // 1. Butta i blocchi vuoti: ogni blocco tolto sposta i successivi a sinistra, e `fine`
        //    segue il confine destro della regione.
        let mut fine = a;
        let mut i = da;
        while i <= fine && i < self.chunks.len() {
            if self.chunks[i].pieces.is_empty() {
                self.chunks.remove(i);
                fine = fine.saturating_sub(1);
            } else {
                i += 1;
            }
        }
        if self.chunks.is_empty() {
            return; // niente piu' pezzi: nessun blocco
        }
        // 2. Il blocco rifarcito (il primo toccato) puo' essere piu' lungo di `cap`: si divide
        //    in parti il piu' possibile uguali. Le parti in piu' allargano la regione a destra.
        let parti = self.split_chunk(da.min(self.chunks.len() - 1));
        fine = fine.saturating_add(parti - 1);
        // 3. Fonde i vicini che insieme stanno in mezzo blocco, dal blocco prima del primo
        //    toccato fino al confine della regione. Dopo una fusione si ricontrolla la stessa
        //    posizione: il blocco fuso potrebbe essere ancora piccolo rispetto al prossimo.
        let mut i = da.saturating_sub(1);
        while i + 1 < self.chunks.len() && i <= fine {
            if self.chunks[i].pieces.len() + self.chunks[i + 1].pieces.len() > self.cap / 2 {
                i += 1;
                continue;
            }
            let vicino = self.chunks.remove(i + 1);
            let c = &mut self.chunks[i];
            c.pieces.extend(vicino.pieces);
            c.bytes += vicino.bytes;
        }
    }

    /// Divide il blocco `k` in `m` parti il piu' possibile uguali, se e' piu' lungo di `cap`
    /// (altrimenti non fa nulla e restituisce 1). Con `m = ceil(n / cap)` parti uguali ogni
    /// parte sta fra circa `cap/2` e `cap` pezzi, quindi due parti vicine (o una parte e un
    /// blocco qualsiasi) sommano sempre piu' di `cap/2`: le invarianti reggono anche ai bordi.
    fn split_chunk(&mut self, k: usize) -> usize {
        let n = match self.chunks.get(k) {
            Some(c) if c.pieces.len() > self.cap => c.pieces.len(),
            _ => return 1,
        };
        let m = n.div_ceil(self.cap);
        let q = n / m; // pezzi per parte...
        let extra = n % m; // ...piu' uno per le prime `extra` parti
        let mut pezzi = std::mem::take(&mut self.chunks[k].pieces);
        for j in 0..m {
            let size = if j < extra { q + 1 } else { q };
            let parte: Vec<Piece> = pezzi.drain(..size).collect();
            let bytes = parte.iter().map(|p| p.len).sum();
            let blocco = Chunk {
                pieces: parte,
                bytes,
            };
            if j == 0 {
                self.chunks[k] = blocco;
            } else {
                self.chunks.insert(k + j, blocco);
            }
        }
        m
    }
}
