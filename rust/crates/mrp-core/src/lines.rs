//! Indice delle righe sparso: ricorda la posizione di un sottoinsieme di righe, non di tutte.
//!
//! Convenzione sulle righe (identica per tutto il progetto):
//! - il separatore e' il solo byte `\n` (in un file CRLF il `\r` resta dentro la riga);
//! - le righe sono numerate da 0;
//! - il numero di righe e' il numero di `\n` piu' 1 se il file e' non vuoto e non finisce con `\n`;
//!   un file vuoto ha 0 righe; `"a\n"` ha 1 riga; `"a\nb"` ha 2 righe; `"\n\n"` ha 2 righe.
//!
//! La costruzione legge il file con letture esplicite (`FileSource::read_at`) a segmenti di
//! `READ_BYTES`, non dalla mappatura: il working set del processo non cresce con la dimensione
//! del file. La mappatura si tocca solo nel ripiego di `scan_block`, per un blocco la cui
//! lettura e' fallita o finita prima del previsto.

use crate::source::FileSource;
use memchr::memchr_iter;
use rayon::prelude::*;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

/// Dimensione minima del file (64 MiB) sotto la quale il precaricamento sequenziale non
/// parte: i file piccoli restano in cache e i soli lavoratori leggono comunque in ordine.
const PREFETCH_MIN_FILE: usize = 64 * 1024 * 1024;
/// Passo del precaricamento sequenziale: byte di file toccati per ogni giro del thread.
const PREFETCH_STEP: usize = 8 * 1024 * 1024;
/// Massimo vantaggio in byte del thread di precaricamento sui lavoratori: oltre, aspetta.
const PREFETCH_LEAD: u64 = 256 * 1024 * 1024;

/// Quanti byte legge in una volta ogni lavoratore dell'indice, con `read_at` e un buffer
/// proprio riusato (E21): segmenti piccoli, cosi' il working set del processo non cresce
/// con la dimensione del file.
pub const READ_BYTES: usize = 256 * 1024;

/// La costruzione e' stata interrotta dal chiamante.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

/// Parametri della costruzione.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Dimensione in byte dei blocchi in cui il file viene diviso (>= 1). Predefinito: 4 MiB.
    pub chunk_bytes: usize,
    /// Distanza massima, in righe, fra due punti memorizzati nello stesso blocco (>= 1). Predefinito: 256.
    pub checkpoint_every: u32,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions {
            chunk_bytes: 4 * 1024 * 1024,
            checkpoint_every: 256,
        }
    }
}

/// Risultato dell'analisi di un blocco: quanti inizi di riga vi cadono e i punti locali
/// (indice locale dell'inizio, posizione in byte), solo per gli indici multipli di `every`.
struct ChunkResult {
    count: u64,
    points: Vec<(u64, u64)>,
}

/// Alla distruzione porta a `u64::MAX` il contatore dei byte precaricati, cioe' "tutto coperto":
/// sblocca i lavoratori in attesa anche se il thread del precaricamento termina male o va in panico.
struct ReleaseOnDrop<'a>(&'a AtomicU64);

impl Drop for ReleaseOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(u64::MAX, Ordering::Relaxed);
    }
}

/// Indice sparso delle righe di un file.
pub struct LineIndex {
    /// Numero totale di righe.
    total_lines: u64,
    /// Punti memorizzati: (numero di riga, posizione in byte), crescenti su entrambi.
    checkpoints: Vec<(u64, u64)>,
}

/// Converte una posizione in byte in un indice valido per la fetta `bytes`.
/// Con un indice coerente la posizione e' sempre dentro la fetta; il limite difensivo evita
/// un panic se l'indice viene interrogato su un file piu' corto di quello con cui e' stato
/// costruito. `usize::try_from` e non `as`, che su 32 bit troncherebbe.
fn slice_index(bytes: &[u8], pos: u64) -> usize {
    usize::try_from(pos).unwrap_or(bytes.len()).min(bytes.len())
}

/// Analisi del blocco `i` (da 0): quanti inizi di riga vi cadono e i punti locali (indice
/// locale dell'inizio, posizione in byte), solo per gli indici multipli di `every`. Il blocco
/// copre `[i * chunk, min((i + 1) * chunk, bytes.len()))`. Cancel e progress li gestisce il
/// chiamante, prima e dopo la chiamata.
///
/// Percorso di ripiego, non piu' quello normale (E21): i lavoratori leggono con
/// `scan_block_reads` e toccano la mappatura solo se una lettura esplicita fallisce o se il
/// file e' stato accorciato mentre si indicizzava; in quel caso il blocco si rifa' qui.
fn scan_block(bytes: &[u8], i: usize, chunk: usize, every: u64) -> ChunkResult {
    // Con i < n_blocchi resta a < file_len: il prodotto non va in overflow.
    let a = i * chunk;
    let b = a.saturating_add(chunk).min(bytes.len());
    let mut count: u64 = 0;
    let mut points: Vec<(u64, u64)> = Vec::new();
    // Il primo byte del blocco e' un inizio di riga se e' l'inizio del file
    // oppure se il byte precedente (del blocco prima, sola lettura) e' `\n`.
    if a == 0 || bytes[a - 1] == b'\n' {
        if count.is_multiple_of(every) {
            points.push((count, a as u64));
        }
        count += 1;
    }
    // Inizi interni: il byte dopo ogni `\n` del blocco. Se `p + 1 == b` l'inizio
    // appartiene al blocco dopo (lo trova con il controllo precedente);
    // se `p + 1 == len` non e' un inizio.
    for p in memchr_iter(b'\n', &bytes[a..b]) {
        let start = a + p + 1;
        if start < b {
            if count.is_multiple_of(every) {
                points.push((count, start as u64));
            }
            count += 1;
        }
    }
    ChunkResult { count, points }
}

/// Riempie tutto `buf` con letture esplicite da `src` a partire da `off`, ripetute perche'
/// una lettura puo' essere breve. Errore se una lettura fallisce o se il file finisce prima
/// del previsto (e' stato accorciato mentre si indicizza): il chiamante rifara' il blocco
/// con `scan_block` sulla mappatura. `off + buf.len()` non supera mai la fine del file,
/// quindi la posizione non va in overflow.
fn read_full(src: &FileSource, buf: &mut [u8], off: usize) -> io::Result<()> {
    let mut done: usize = 0;
    while done < buf.len() {
        let n = src.read_at(&mut buf[done..], (off + done) as u64)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "file accorciato mentre si indicizza",
            ));
        }
        done += n;
    }
    Ok(())
}

/// Analisi del blocco `[a, b)` con letture esplicite a segmenti di al piu' `buf.len()` byte
/// (E21): e' il percorso normale dei lavoratori, che non toccano la mappatura, cosi' il
/// working set del processo non cresce con la dimensione del file. `buf` e' il buffer del
/// lavoratore: non vuoto (il chiamante lo crea di `READ_BYTES.min(chunk)`, almeno 1) e riusato
/// per tutti i segmenti. Da un segmento all'altro si portano il conteggio locale degli inizi
/// di riga del blocco e se l'ultimo byte letto era `\n`. Le regole sono quelle di
/// `scan_block`: un inizio di riga a `p` in `[a, b)` e' `p == a` con il byte prima uguale a
/// `\n` (o `a == 0`, inizio del file), oppure `p > a` con `\n` in `p - 1`; non esiste un
/// inizio a `p == b` (appartiene al blocco dopo); il punto `(numero locale, p)` si memorizza
/// quando il numero locale e' multiplo di `every`. Errore se una lettura fallisce o il file
/// finisce prima di `b`: il chiamante rifara' il blocco con `scan_block` sulla mappatura.
fn scan_block_reads(
    src: &FileSource,
    a: usize,
    b: usize,
    every: u64,
    buf: &mut [u8],
) -> io::Result<ChunkResult> {
    let mut count: u64 = 0;
    let mut points: Vec<(u64, u64)> = Vec::new();
    // Se il byte appena prima di quello che sta per arrivare era `\n`: decide se il primo
    // byte del prossimo segmento e' un inizio di riga. Per il primo segmento del blocco il
    // "byte prima" e' quello in `a - 1`, letto qui con una lettura di un byte; per `a == 0`
    // conta come `\n` (inizio del file).
    let mut prev_nl = if a == 0 {
        true
    } else {
        let mut one = [0u8; 1];
        read_full(src, &mut one, a - 1)?;
        one[0] == b'\n'
    };
    let mut s = a;
    while s < b {
        // Prossimo segmento: al piu' quanto il buffer, mai oltre la fine del blocco
        // (`seg_len` resta quindi dentro `buf`, per costruzione).
        let seg_len = buf.len().min(b - s);
        let e = s + seg_len;
        // Riempie il segmento con letture esplicite ripetute (una puo' essere breve).
        read_full(src, &mut buf[..seg_len], s)?;
        let seg = &buf[..seg_len];
        // Il primo byte del segmento e' un inizio di riga se il "byte prima" era `\n`.
        if prev_nl {
            if count.is_multiple_of(every) {
                points.push((count, s as u64));
            }
            count += 1;
        }
        // Inizi interni: il byte dopo ogni `\n` del segmento. Quello sull'ultimo byte non si
        // registra qui: lo registra il segmento dopo tramite `prev_nl` (e se `e == b` non lo
        // registra nessuno, perche' un inizio a `b` appartiene al blocco dopo).
        for j in memchr_iter(b'\n', seg) {
            let start = s + j + 1;
            if start < e {
                if count.is_multiple_of(every) {
                    points.push((count, start as u64));
                }
                count += 1;
            }
        }
        // Da portare al segmento dopo: l'ultimo byte del segmento era `\n`?
        prev_nl = seg.last() == Some(&b'\n');
        s = e;
    }
    Ok(ChunkResult { count, points })
}

impl LineIndex {
    /// Costruisce l'indice con i parametri predefiniti.
    pub fn build(
        src: &FileSource,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<LineIndex, Cancelled> {
        Self::build_with(src, &BuildOptions::default(), cancel, progress)
    }

    /// Costruisce l'indice.
    /// - I blocchi vengono assegnati ai lavoratori in ordine crescente e letti una sola volta;
    ///   sui file grandi (>= `PREFETCH_MIN_FILE`) un thread di precaricamento legge il file in
    ///   avanti con letture esplicite e i lavoratori elaborano un blocco solo dopo che e' stato
    ///   letto: cosi' il disco lavora su letture contigue (vedi docs/BENCHMARKS.md).
    /// - Ogni lavoratore legge i suoi blocchi con letture esplicite (`read_at`) in un proprio
    ///   buffer di al piu' `READ_BYTES` byte, riusato: la mappatura non si tocca e il working
    ///   set del processo non cresce con la dimensione del file. Solo se una lettura fallisce
    ///   o il file finisce prima del previsto il blocco si rifa' sulla mappatura.
    /// - `cancel` e' controllato prima di ogni blocco: se e' vero restituisce `Err(Cancelled)`
    ///   (anche se e' gia' vero all'inizio).
    /// - `progress` viene incrementato del numero di byte elaborati; a costruzione riuscita vale `src.len()`
    ///   (si parte da zero: il chiamante passa un contatore a 0).
    pub fn build_with(
        src: &FileSource,
        opts: &BuildOptions,
        cancel: &AtomicBool,
        progress: &AtomicU64,
    ) -> Result<LineIndex, Cancelled> {
        // Annullato in partenza: vale per qualunque file, anche vuoto.
        if cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        let chunk = opts.chunk_bytes.max(1);
        let every = u64::from(opts.checkpoint_every.max(1));
        // La lunghezza arriva da `len()`, non dalla mappatura (E21): la costruzione non deve
        // toccarla. `open` garantisce len <= usize::MAX: la conversione non fallisce mai, ma
        // fuori dai test non si usa `unwrap` (regola del crate); il ripiego e' irraggiungibile.
        let file_len = usize::try_from(src.len()).unwrap_or(usize::MAX);
        if file_len == 0 {
            return Ok(LineIndex {
                total_lines: 0,
                checkpoints: Vec::new(),
            });
        }
        // Numero di blocchi: ceil(file_len / chunk), senza overflow.
        let n_blocks = file_len / chunk + usize::from(!file_len.is_multiple_of(chunk));
        // Contatori condivisi da lavoratori e precaricamento: `next` distribuisce i numeri di
        // blocco in ordine crescente, `done` conta i byte gia' elaborati, `finished` segnala
        // che i lavoratori sono usciti.
        let next = AtomicUsize::new(0);
        let done = AtomicU64::new(0);
        let finished = AtomicBool::new(false);
        // Quanti byte dall'inizio il precaricamento ha gia' letto. I lavoratori non elaborano un
        // blocco finche' non e' coperto: cosi' l'unico che legge dal disco e' il precaricamento, in
        // ordine (i page fault dei lavoratori si metterebbero in mezzo e causerebbero seek).
        // Senza precaricamento (file piccolo) o quando termina vale `u64::MAX`: nessuna attesa.
        let prefetched = AtomicU64::new(if file_len >= PREFETCH_MIN_FILE {
            0
        } else {
            u64::MAX
        });
        // Di quanto il precaricamento puo' stare davanti ai blocchi COMPLETATI (`done`). Deve essere
        // almeno un blocco piu' un passo: i blocchi completati coprono tutto cio' che e' stato letto
        // tranne al piu' il blocco in corso, quindi `touched - done <= chunk`; con un vantaggio
        // massimo minore il precaricamento si fermerebbe prima di coprire il blocco che i
        // lavoratori aspettano (attesa reciproca, corretta dopo la review del junior).
        let lead = PREFETCH_LEAD.max((chunk as u64).saturating_add(PREFETCH_STEP as u64));
        let r = std::thread::scope(|scope| {
            if file_len >= PREFETCH_MIN_FILE {
                // Precaricamento sequenziale: il thread scorre il file dall'inizio alla fine
                // a passi di PREFETCH_STEP con letture esplicite, cosi' le pagine entrano
                // in cache con letture contigue e i lavoratori, che procedono in ordine, non
                // costringono il disco a seek continui. Prima di ogni passo aspetta che il
                // vantaggio sui lavoratori scenda a PREFETCH_LEAD; esce subito su cancel o a
                // lavoratori finiti. Non alloca nulla di rilevante e non restituisce nulla.
                scope.spawn(|| {
                    let run = || {
                        let mut touched: u64 = 0;
                        let mut off: usize = 0;
                        let mut buf = vec![0u8; PREFETCH_STEP];
                        while off < file_len {
                            // Prima di ogni passo: esci subito su cancel o lavoratori finiti,
                            // altrimenti aspetta finche' il vantaggio supera PREFETCH_LEAD (i
                            // lavoratori possono anche essere davanti: sottrazione satura).
                            loop {
                                if cancel.load(Ordering::Relaxed)
                                    || finished.load(Ordering::Relaxed)
                                {
                                    return;
                                }
                                if touched.saturating_sub(done.load(Ordering::Relaxed)) <= lead {
                                    break;
                                }
                                std::thread::sleep(Duration::from_millis(1));
                            }
                            let end = off.saturating_add(PREFETCH_STEP).min(file_len);
                            // Lettura ESPLICITA (una richiesta grande e sequenziale), non un accesso
                            // alla mappatura: i page fault su disco meccanico sono 4-5 volte piu'
                            // lenti. Le pagine entrano nella cache di sistema e la mappatura le
                            // trova pronte. Il contenuto letto non serve: il buffer (8 MiB) si butta.
                            let want = end - off;
                            match src.read_at(&mut buf[..want], off as u64) {
                                Ok(n) if n > 0 => {
                                    touched += n as u64;
                                    off += n;
                                    prefetched.store(touched, Ordering::Relaxed);
                                }
                                // Errore o fine inattesa: il precaricamento e' solo un'ottimizzazione,
                                // i lavoratori leggono comunque con `read_at` (E21).
                                _ => return,
                            }
                        }
                    };
                    // Comunque finisca `run` (anche con un panico), libera i lavoratori in attesa.
                    let _libera = ReleaseOnDrop(&prefetched);
                    run();
                });
            }
            // Un lavoratore per thread del pool rayon: ciascuno prende da `next` il numero del
            // blocco successivo, cosi' i blocchi vengono letti in ordine crescente.
            let workers = (0..rayon::current_num_threads())
                .into_par_iter()
                .map(|_| {
                    let mut raccolti: Vec<(usize, ChunkResult)> = Vec::new();
                    // Un buffer per lavoratore, riusato per tutti i suoi blocchi (E21): le
                    // letture sono a segmenti di al piu' READ_BYTES, o del blocco se piu'
                    // corto, e la mappatura non si tocca.
                    let mut buf = vec![0u8; READ_BYTES.min(chunk)];
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= n_blocks {
                            break;
                        }
                        // Il cancel si controlla PRIMA di elaborare il blocco.
                        if cancel.load(Ordering::Relaxed) {
                            return Err(Cancelled);
                        }
                        // Stessi estremi del blocco che ricalcola scan_block.
                        let a = i * chunk;
                        let b = a.saturating_add(chunk).min(file_len);
                        // Aspetta che il precaricamento abbia coperto questo blocco.
                        while prefetched.load(Ordering::Relaxed) < b as u64 {
                            if cancel.load(Ordering::Relaxed) {
                                return Err(Cancelled);
                            }
                            std::thread::sleep(Duration::from_millis(1));
                        }
                        // Percorso normale (E21): letture esplicite a segmenti, senza
                        // mappatura. Se una lettura fallisce o il file finisce prima del
                        // previsto (accorciato mentre si indicizza), il blocco si rifa' con
                        // `scan_block` sulla mappatura: e' l'unico punto in cui i lavoratori
                        // la toccano.
                        let res = match scan_block_reads(src, a, b, every, &mut buf) {
                            Ok(res) => res,
                            Err(_) => scan_block(src.bytes(), i, chunk, every),
                        };
                        progress.fetch_add((b - a) as u64, Ordering::Relaxed);
                        done.fetch_add((b - a) as u64, Ordering::Relaxed);
                        raccolti.push((i, res));
                    }
                    Ok(raccolti)
                })
                .collect::<Result<Vec<_>, _>>();
            // I lavoratori sono usciti tutti (file finito o cancel): lo segnalo prima di uscire
            // dallo scope, che altrimenti resterebbe ad aspettare il precaricamento.
            finished.store(true, Ordering::Relaxed);
            workers
        });
        // Ogni lavoratore ha blocchi crescenti ma i suoi intervalli si alternano con quelli
        // degli altri: serve l'ordine globale prima della fusione.
        let mut results: Vec<(usize, ChunkResult)> = r?.into_iter().flatten().collect();
        results.sort_by_key(|&(i, _)| i);
        // Fusione sequenziale: la somma prefissa dei count da' il numero di riga globale del
        // primo inizio di ogni blocco; i punti globali sono (prima_riga_del_blocco + k, posizione).
        let mut checkpoints: Vec<(u64, u64)> = Vec::new();
        let mut first_row: u64 = 0;
        for (_, res) in results {
            for (k, pos) in res.points {
                checkpoints.push((first_row + k, pos));
            }
            first_row += res.count;
        }
        Ok(LineIndex {
            total_lines: first_row,
            checkpoints,
        })
    }

    /// Numero di righe (vedi la convenzione in testa al modulo).
    pub fn line_count(&self) -> u64 {
        self.total_lines
    }

    /// Quanti punti sono memorizzati. Serve a verificare che l'indice sia sparso.
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// Quanti `\n` ci sono nei byte `[0, offset)` del file. `offset == src.len()` e' ammesso (da' il totale dei `\n`);
    /// `None` se `offset > src.len()`. Parte dal punto memorizzato piu' vicino: il costo non dipende dalla dimensione del file.
    pub fn newlines_before(&self, src: &FileSource, offset: u64) -> Option<u64> {
        if offset > src.len() {
            return None;
        }
        let bytes = src.bytes();
        // Ultimo punto con posizione <= offset (le posizioni dei punti sono crescenti);
        // il fallback (0, 0) e' solo difensivo per un indice senza punti.
        let i = self.checkpoints.partition_point(|&(_, p)| p <= offset);
        let (riga_punto, pos_punto) = if i == 0 {
            (0, 0)
        } else {
            self.checkpoints[i - 1]
        };
        // Conta i `\n` in [pos_punto, offset), con `offset` escluso: la riga del punto e'
        // gia' il numero di `\n` che precedono la sua posizione.
        let inizio = slice_index(bytes, pos_punto);
        let fine = slice_index(bytes, offset);
        let conteggio = memchr_iter(b'\n', &bytes[inizio..fine]).count() as u64;
        Some(riga_punto + conteggio)
    }

    /// Posizione in byte dell'inizio della riga `line` (da 0). `None` se `line >= line_count()`.
    /// Parte dal punto memorizzato piu' vicino e cerca i `\n` nel tratto di file che segue.
    pub fn offset_of_line(&self, src: &FileSource, line: u64) -> Option<u64> {
        if line >= self.total_lines {
            return None;
        }
        // Ultimo punto con riga <= line (le righe dei punti sono crescenti). L'indice di un
        // file non vuoto ha sempre il punto (0, 0), quindi i >= 1: il fallback e' difensivo.
        let i = self.checkpoints.partition_point(|&(r, _)| r <= line);
        let (riga_punto, off_punto) = if i == 0 {
            (0, 0)
        } else {
            self.checkpoints[i - 1]
        };
        let da_saltare = line - riga_punto;
        if da_saltare == 0 {
            return Some(off_punto);
        }
        // Salta `da_saltare` caratteri `\n` a partire dal punto: il `\n` che chiude la riga
        // cercata e' il `da_saltare`-esimo della fetta (il primo ha indice 0). Se non c'e',
        // l'indice e' incoerente con questo `src`: None, senza panic (difensivo).
        let bytes = src.bytes();
        let inizio = slice_index(bytes, off_punto);
        let n = usize::try_from(da_saltare - 1).ok()?;
        let p = memchr_iter(b'\n', &bytes[inizio..]).nth(n)?;
        Some((inizio + p) as u64 + 1)
    }

    /// Numero della riga che contiene il byte in `offset`. `None` se `offset >= src.len()`.
    /// Il `\n` finale appartiene alla riga che chiude.
    pub fn line_at_offset(&self, src: &FileSource, offset: u64) -> Option<u64> {
        if offset >= src.len() {
            return None;
        }
        // La riga che contiene un byte e' il numero di `\n` che lo precedono: e' esattamente
        // `newlines_before` (che qui riceve `offset < src.len()`, quindi risponde sempre).
        self.newlines_before(src, offset)
    }
}
